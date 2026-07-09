use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rand::RngCore as _;
use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, IsCa,
    Issuer, KeyPair, KeyUsagePurpose,
};
use rustls::crypto::aws_lc_rs;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::sign::CertifiedKey;
use time::{Duration, OffsetDateTime};

pub const ROOT_CERT_FILE: &str = "root-ca.pem";
pub const ROOT_KEY_FILE: &str = "root-ca-key.der";
const ROOT_DER_FILE: &str = "root-ca.der";
const INSTALLATION_ID_FILE: &str = "installation.id";

#[derive(Clone)]
pub struct CaStore {
    inner: Arc<CaInner>,
}

struct CaInner {
    issuer: Issuer<'static, KeyPair>,
    root_der: CertificateDer<'static>,
    installation_id: String,
}

impl fmt::Debug for CaStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CaStore")
            .field("installation_id", &self.inner.installation_id)
            .finish_non_exhaustive()
    }
}

impl CaStore {
    pub fn load_or_create(state_dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(state_dir)
            .map_err(|error| format!("create DuckDNS state dir: {error}"))?;
        let paths = Paths::new(state_dir);
        let presence = [
            paths.key.exists(),
            paths.cert_der.exists(),
            paths.cert_pem.exists(),
            paths.installation_id.exists(),
        ];
        if presence.iter().all(|present| *present) {
            return Self::load(&paths);
        }
        if presence.iter().any(|present| *present) {
            return Err(
                "duckdnsd: incomplete CA state; repair or uninstall the helper transactionally"
                    .into(),
            );
        }
        let materials = Materials::generate()?;
        materials.write_new(&paths)?;
        Self::load(&paths)
    }

    pub fn rotate(state_dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(state_dir)
            .map_err(|error| format!("create DuckDNS state dir: {error}"))?;
        let paths = Paths::new(state_dir);
        let materials = Materials::generate()?;
        materials.replace(&paths)?;
        Self::load(&paths)
    }

    pub fn root_der(&self) -> CertificateDer<'static> {
        self.inner.root_der.clone()
    }

    pub fn installation_id(&self) -> &str {
        &self.inner.installation_id
    }

    pub fn mint(&self, hostname: &str) -> Result<Arc<CertifiedKey>, String> {
        let parsed = duckdns_core::parse_hostname(hostname)?;
        let hostname = parsed.hostname();
        let now = OffsetDateTime::now_utc();
        let mut params = CertificateParams::new(vec![hostname.clone()])
            .map_err(|error| format!("DuckDNS leaf parameters: {error}"))?;
        params.not_before = now - Duration::minutes(5);
        params.not_after = now + Duration::hours(24);
        params.distinguished_name = distinguished_name(&format!("DuckDNS {hostname}"), None);
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        let key = KeyPair::generate().map_err(|error| format!("DuckDNS leaf key: {error}"))?;
        let certificate = params
            .signed_by(&key, &self.inner.issuer)
            .map_err(|error| format!("sign DuckDNS leaf: {error}"))?;
        let private_key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der()));
        let chain = vec![certificate.der().clone(), self.root_der()];
        CertifiedKey::from_der(chain, private_key, &aws_lc_rs::default_provider())
            .map(Arc::new)
            .map_err(|error| format!("load DuckDNS leaf into rustls: {error}"))
    }

    fn load(paths: &Paths) -> Result<Self, String> {
        let key_bytes =
            std::fs::read(&paths.key).map_err(|error| format!("read DuckDNS CA key: {error}"))?;
        let key = KeyPair::try_from(key_bytes.as_slice())
            .map_err(|error| format!("decode DuckDNS CA key: {error}"))?;
        let root_der = CertificateDer::from(
            std::fs::read(&paths.cert_der)
                .map_err(|error| format!("read DuckDNS CA certificate: {error}"))?,
        );
        let installation_id = std::fs::read_to_string(&paths.installation_id)
            .map_err(|error| format!("read DuckDNS installation id: {error}"))?
            .trim()
            .to_owned();
        validate_installation_id(&installation_id)?;
        let issuer = Issuer::new(root_params(&installation_id), key);
        Ok(Self {
            inner: Arc::new(CaInner {
                issuer,
                root_der,
                installation_id,
            }),
        })
    }
}

struct Paths {
    key: PathBuf,
    cert_der: PathBuf,
    cert_pem: PathBuf,
    installation_id: PathBuf,
}

impl Paths {
    fn new(state_dir: &Path) -> Self {
        Self {
            key: state_dir.join(ROOT_KEY_FILE),
            cert_der: state_dir.join(ROOT_DER_FILE),
            cert_pem: state_dir.join(ROOT_CERT_FILE),
            installation_id: state_dir.join(INSTALLATION_ID_FILE),
        }
    }
}

struct Materials {
    key: Vec<u8>,
    cert_der: Vec<u8>,
    cert_pem: Vec<u8>,
    installation_id: Vec<u8>,
}

impl Materials {
    fn generate() -> Result<Self, String> {
        let installation_id = random_hex(16);
        let params = root_params(&installation_id);
        let key =
            KeyPair::generate().map_err(|error| format!("generate DuckDNS CA key: {error}"))?;
        let certificate = params
            .self_signed(&key)
            .map_err(|error| format!("generate DuckDNS CA certificate: {error}"))?;
        Ok(Self {
            key: key.serialize_der(),
            cert_der: certificate.der().to_vec(),
            cert_pem: certificate.pem().into_bytes(),
            installation_id: format!("{installation_id}\n").into_bytes(),
        })
    }

    fn write_new(&self, paths: &Paths) -> Result<(), String> {
        write_new(&paths.key, &self.key, true)?;
        if let Err(error) = write_new(&paths.cert_der, &self.cert_der, false)
            .and_then(|_| write_new(&paths.cert_pem, &self.cert_pem, false))
            .and_then(|_| write_new(&paths.installation_id, &self.installation_id, false))
        {
            for path in [
                &paths.key,
                &paths.cert_der,
                &paths.cert_pem,
                &paths.installation_id,
            ] {
                let _ = std::fs::remove_file(path);
            }
            return Err(error);
        }
        Ok(())
    }

    fn replace(&self, paths: &Paths) -> Result<(), String> {
        let nonce = random_hex(8);
        let temporary = Paths {
            key: temporary_path(&paths.key, &nonce),
            cert_der: temporary_path(&paths.cert_der, &nonce),
            cert_pem: temporary_path(&paths.cert_pem, &nonce),
            installation_id: temporary_path(&paths.installation_id, &nonce),
        };
        self.write_new(&temporary)?;
        for (from, to) in [
            (&temporary.key, &paths.key),
            (&temporary.cert_der, &paths.cert_der),
            (&temporary.cert_pem, &paths.cert_pem),
            (&temporary.installation_id, &paths.installation_id),
        ] {
            if to.exists() {
                std::fs::remove_file(to)
                    .map_err(|error| format!("replace {}: {error}", to.display()))?;
            }
            std::fs::rename(from, to)
                .map_err(|error| format!("install {}: {error}", to.display()))?;
        }
        Ok(())
    }
}

fn temporary_path(path: &Path, nonce: &str) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .expect("DuckDNS CA paths have UTF-8 file names");
    path.with_file_name(format!("{name}.tmp-{nonce}"))
}

fn root_params(installation_id: &str) -> CertificateParams {
    let now = OffsetDateTime::now_utc();
    let mut params = CertificateParams::new(Vec::<String>::new()).expect("empty SAN list");
    params.not_before = now - Duration::days(1);
    params.not_after = now + Duration::days(3650);
    params.distinguished_name = distinguished_name(
        "Ducktape DuckDNS Device Root",
        Some(&format!("duckdnsd:{installation_id}")),
    );
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
    ];
    params
}

fn distinguished_name(common_name: &str, unit: Option<&str>) -> DistinguishedName {
    let mut name = DistinguishedName::new();
    name.push(DnType::OrganizationName, "Ducktape");
    if let Some(unit) = unit {
        name.push(DnType::OrganizationalUnitName, unit);
    }
    name.push(DnType::CommonName, common_name);
    name
}

fn validate_installation_id(value: &str) -> Result<(), String> {
    if value.len() != 32
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("duckdnsd: installation id is corrupt".into());
    }
    Ok(())
}

fn random_hex(bytes: usize) -> String {
    let mut random = vec![0u8; bytes];
    rand::rngs::OsRng.fill_bytes(&mut random);
    random.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn write_new(path: &Path, bytes: &[u8], private: bool) -> Result<(), String> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(if private { 0o600 } else { 0o644 });
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("create {}: {error}", path.display()))?;
    use std::io::Write as _;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("write {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ca_persists_and_rotates_with_protected_key() {
        let directory = tempfile::tempdir().unwrap();
        let first = CaStore::load_or_create(directory.path()).unwrap();
        let same = CaStore::load_or_create(directory.path()).unwrap();
        assert_eq!(first.root_der(), same.root_der());
        assert_eq!(first.installation_id(), same.installation_id());
        first.mint("docs.team-a1b2c3d4.net.ducktape.quack").unwrap();

        let rotated = CaStore::rotate(directory.path()).unwrap();
        assert_ne!(first.root_der(), rotated.root_der());
        assert_ne!(first.installation_id(), rotated.installation_id());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(directory.path().join(ROOT_KEY_FILE))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }
    }
}
