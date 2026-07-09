//! Linux split-DNS, trust-store, and service adapter for `duckdnsd`.
//!
//! The shared installer owns transaction markers and fixed verbs; this file
//! keeps Linux-specific systemd, systemd-resolved, p11-kit, NSS/Firefox, and
//! ownership-preserving enterprise-policy details out of the cross-platform
//! installation coordinator.

use super::*;

const BINARY: &str = "/usr/local/libexec/ducktape/duckdnsd";
const SERVICE: &str = "/etc/systemd/system/ducktape-duckdnsd.service";
const FIREFOX_POLICY: &str = "/etc/firefox/policies/policies.json";

pub(super) fn binary_path() -> PathBuf {
    BINARY.into()
}

pub(super) fn public_certificate(id: &str) -> Option<PathBuf> {
    trust_anchor(id).ok()
}

fn resolver(id: &str) -> PathBuf {
    PathBuf::from(format!(
        "/etc/systemd/resolved.conf.d/ducktape-duckdns-{id}.conf"
    ))
}

fn trust_anchor(id: &str) -> Result<PathBuf, String> {
    if Path::new("/usr/local/share/ca-certificates").is_dir() {
        Ok(format!("/usr/local/share/ca-certificates/ducktape-duckdns-{id}.crt").into())
    } else if Path::new("/etc/pki/ca-trust/source/anchors").is_dir() {
        Ok(format!("/etc/pki/ca-trust/source/anchors/ducktape-duckdns-{id}.pem").into())
    } else {
        Err("duckdnsd: no supported system CA anchor directory found".into())
    }
}

fn firefox_policy_marker(id: &str) -> PathBuf {
    PathBuf::from(format!(
        "/etc/firefox/policies/ducktape-duckdns-{id}.installation-id"
    ))
}

fn firefox_certificate(id: &str) -> PathBuf {
    PathBuf::from(format!(
        "/usr/lib/mozilla/certificates/ducktape-duckdns-{id}.pem"
    ))
}

fn firefox_certificate_marker(id: &str) -> PathBuf {
    firefox_certificate(id).with_extension("pem.installation-id")
}

pub(super) fn stop_owned(id: &str) -> Result<(), String> {
    let service = Path::new(SERVICE);
    if service.exists() {
        ensure_owned(service, id)?;
        run_allow_failure("systemctl", &["stop", "ducktape-duckdnsd.service"]);
    }
    Ok(())
}

pub(super) fn protect_state(state_dir: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(state_dir, std::fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("chmod {}: {error}", state_dir.display()))
}

pub(super) fn apply(id: &str, root_cert: &Path) -> Result<(), String> {
    let service = format!(
        "# {}\n[Unit]\nDescription=Ducktape DuckDNS device helper\nAfter=network.target systemd-resolved.service\nWants=systemd-resolved.service\n\n[Service]\nType=simple\nExecStart={BINARY} serve\nRestart=on-failure\nRestartSec=2\nNoNewPrivileges=true\nPrivateTmp=true\nPrivateDevices=true\nProtectSystem=strict\nProtectHome=true\nReadWritePaths={}\nRestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX\nCapabilityBoundingSet=CAP_NET_BIND_SERVICE\nAmbientCapabilities=CAP_NET_BIND_SERVICE\n\n[Install]\nWantedBy=multi-user.target\n",
        marker(id),
        default_state_dir().display(),
    );
    write_owned(Path::new(SERVICE), service.as_bytes(), id)?;
    let resolver = format!(
        "# {}\n[Resolve]\nDNS=127.77.0.1\nDomains=~ducktape.quack\n",
        marker(id)
    );
    write_owned(&resolver_path(id), resolver.as_bytes(), id)?;

    let anchor = trust_anchor(id)?;
    let mut certificate = format!("# {}\n", marker(id)).into_bytes();
    certificate.extend(
        std::fs::read(root_cert)
            .map_err(|error| format!("read {}: {error}", root_cert.display()))?,
    );
    write_owned(&anchor, &certificate, id)?;
    refresh_trust(&anchor)?;
    let firefox_certificate = install_firefox_certificate(id, root_cert)?;
    install_firefox_policy(id, &firefox_certificate)?;
    run("systemctl", &["daemon-reload"])?;
    run("systemctl", &["restart", "systemd-resolved.service"])?;
    run(
        "systemctl",
        &["enable", "--now", "ducktape-duckdnsd.service"],
    )
}

fn resolver_path(id: &str) -> PathBuf {
    resolver(id)
}

pub(super) fn inspect(id: &str, problems: &mut Vec<String>) {
    inspect_owned(Path::new(SERVICE), id, problems);
    inspect_owned(&resolver_path(id), id, problems);
    let anchor = match trust_anchor(id) {
        Ok(path) => {
            inspect_owned(&path, id, problems);
            Some(path)
        }
        Err(error) => {
            problems.push(error);
            None
        }
    };
    inspect_firefox_certificate(id, problems);
    inspect_firefox_policy(id, problems);
    inspect_certificate_copies(anchor.as_deref(), &firefox_certificate(id), problems);
    if !matches!(
        Command::new("systemctl")
            .args(["is-active", "--quiet", "ducktape-duckdnsd.service"])
            .output(),
        Ok(output) if output.status.success()
    ) {
        problems.push("DuckDNS systemd service is not active".into());
    }
}

fn inspect_certificate_copies(anchor: Option<&Path>, firefox: &Path, problems: &mut Vec<String>) {
    let root_path = default_state_dir().join(ROOT_CERT_FILE);
    let root = match std::fs::read(&root_path) {
        Ok(root) => root,
        Err(error) => {
            problems.push(format!("read {}: {error}", root_path.display()));
            return;
        }
    };
    if let Some(anchor) = anchor
        && std::fs::read(anchor).is_ok_and(|installed| !installed.ends_with(&root))
    {
        problems.push("system trust store has a stale DuckDNS root certificate".into());
    }
    if std::fs::read(firefox).is_ok_and(|installed| installed != root) {
        problems.push("Firefox policy has a stale DuckDNS root certificate".into());
    }
}

pub(super) fn remove(id: &str) -> Result<(), String> {
    stop_owned(id)?;
    run_allow_failure("systemctl", &["disable", "ducktape-duckdnsd.service"]);
    remove_owned(Path::new(SERVICE), id)?;
    remove_owned(&resolver_path(id), id)?;
    remove_firefox_policy(id)?;
    remove_firefox_certificate_file(id)?;
    if let Ok(anchor) = trust_anchor(id) {
        remove_owned(&anchor, id)?;
        refresh_trust(&anchor)?;
    }
    run("systemctl", &["daemon-reload"])?;
    run("systemctl", &["restart", "systemd-resolved.service"])
}

fn refresh_trust(anchor: &Path) -> Result<(), String> {
    if anchor.starts_with("/usr/local/share/ca-certificates") {
        run("update-ca-certificates", &[])
    } else {
        run("update-ca-trust", &["extract"])
    }
}

fn install_firefox_certificate(id: &str, root_cert: &Path) -> Result<PathBuf, String> {
    let certificate = firefox_certificate(id);
    let marker_path = firefox_certificate_marker(id);
    if certificate.exists() && !marker_path.exists() {
        return Err(format!(
            "duckdnsd: refusing to replace unmarked Firefox certificate {}",
            certificate.display()
        ));
    }
    ensure_owned_or_absent(&marker_path, id)?;
    write_owned(&marker_path, format!("{}\n", marker(id)).as_bytes(), id)?;
    let bytes = std::fs::read(root_cert)
        .map_err(|error| format!("read {}: {error}", root_cert.display()))?;
    write_unmarked_owned(&certificate, &bytes, &marker_path, id)?;
    Ok(certificate)
}

fn inspect_firefox_certificate(id: &str, problems: &mut Vec<String>) {
    let certificate = firefox_certificate(id);
    if !certificate.is_file() {
        problems.push(format!(
            "missing Firefox certificate {}",
            certificate.display()
        ));
    }
    inspect_owned(&firefox_certificate_marker(id), id, problems);
}

fn remove_firefox_certificate_file(id: &str) -> Result<(), String> {
    let certificate = firefox_certificate(id);
    let marker_path = firefox_certificate_marker(id);
    if !marker_path.exists() {
        if certificate.exists() {
            return Err(format!(
                "duckdnsd: refusing to remove unmarked Firefox certificate {}",
                certificate.display()
            ));
        }
        return Ok(());
    }
    ensure_owned(&marker_path, id)?;
    remove_file_if_present(&certificate)?;
    remove_owned(&marker_path, id)
}

fn write_unmarked_owned(
    path: &Path,
    bytes: &[u8],
    marker_path: &Path,
    id: &str,
) -> Result<(), String> {
    ensure_owned(marker_path, id)?;
    if std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(format!(
            "duckdnsd: refusing to replace symlinked artifact {}",
            path.display()
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create {}: {error}", parent.display()))?;
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    let _ = std::fs::remove_file(&temporary);
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("write {}: {error}", temporary.display()))?;
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o644))
        .map_err(|error| format!("chmod {}: {error}", temporary.display()))?;
    std::fs::rename(&temporary, path)
        .map_err(|error| format!("install {}: {error}", path.display()))
}

#[derive(Clone, Copy, Debug, Default)]
struct FirefoxPolicyOwnership {
    created_policy: bool,
    created_certificates: bool,
    created_certificate_install: bool,
    created_dns_over_https: bool,
    created_excluded_domains: bool,
    added_dns_exclusion: bool,
}

impl FirefoxPolicyOwnership {
    fn before_install(policy_path: &Path, policy: &serde_json::Value) -> Self {
        let policies = policy
            .get("policies")
            .and_then(serde_json::Value::as_object);
        let certificates = policies.and_then(|value| value.get("Certificates"));
        let dns = policies.and_then(|value| value.get("DNSOverHTTPS"));
        Self {
            created_policy: !policy_path.exists(),
            created_certificates: certificates.is_none(),
            created_certificate_install: certificates
                .and_then(|value| value.get("Install"))
                .is_none(),
            created_dns_over_https: dns.is_none(),
            created_excluded_domains: dns.and_then(|value| value.get("ExcludedDomains")).is_none(),
            added_dns_exclusion: false,
        }
    }

    fn from_marker(contents: &str) -> Self {
        Self {
            created_policy: marker_flag(contents, "CreatedPolicy"),
            created_certificates: marker_flag(contents, "CreatedCertificates"),
            created_certificate_install: marker_flag(contents, "CreatedCertificateInstall"),
            created_dns_over_https: marker_flag(contents, "CreatedDnsOverHttps"),
            created_excluded_domains: marker_flag(contents, "CreatedExcludedDomains"),
            added_dns_exclusion: marker_flag(contents, "AddedDnsExclusion"),
        }
    }

    fn encode(self, id: &str) -> String {
        format!(
            "{}\nCreatedPolicy={}\nCreatedCertificates={}\nCreatedCertificateInstall={}\nCreatedDnsOverHttps={}\nCreatedExcludedDomains={}\nAddedDnsExclusion={}\n",
            marker(id),
            self.created_policy,
            self.created_certificates,
            self.created_certificate_install,
            self.created_dns_over_https,
            self.created_excluded_domains,
            self.added_dns_exclusion,
        )
    }
}

fn marker_flag(contents: &str, name: &str) -> bool {
    contents.lines().any(|line| line == format!("{name}=true"))
}

fn install_firefox_policy(id: &str, certificate: &Path) -> Result<(), String> {
    let policy_path = Path::new(FIREFOX_POLICY);
    let marker_path = firefox_policy_marker(id);
    let marker_contents = std::fs::read_to_string(&marker_path).ok();
    let mut policy = read_firefox_policy(policy_path)?;
    let mut ownership = marker_contents
        .as_deref()
        .map(FirefoxPolicyOwnership::from_marker)
        .unwrap_or_else(|| FirefoxPolicyOwnership::before_install(policy_path, &policy));
    if marker_contents.is_none() {
        write_owned(&marker_path, ownership.encode(id).as_bytes(), id)?;
    } else {
        ensure_owned(&marker_path, id)?;
    }

    add_firefox_certificate(&mut policy, certificate)?;
    let legacy_certificate = trust_anchor(id)?;
    if legacy_certificate != certificate {
        remove_firefox_certificate(&mut policy, &legacy_certificate)?;
    }
    let added_exclusion = add_firefox_dns_exclusion(&mut policy)?;
    ownership.added_dns_exclusion |= added_exclusion;
    // Record exactly what this installation added before replacing the shared
    // policy. If the policy write fails, first-install rollback can still
    // remove only our intended fields; if it succeeds, uninstall has the same
    // ownership record even across a crash between these two writes.
    write_owned(&marker_path, ownership.encode(id).as_bytes(), id)?;
    write_shared_json(policy_path, &policy)
}

fn inspect_firefox_policy(id: &str, problems: &mut Vec<String>) {
    let marker_path = firefox_policy_marker(id);
    inspect_owned(&marker_path, id, problems);
    let certificate = firefox_certificate(id);
    match read_firefox_policy(Path::new(FIREFOX_POLICY)) {
        Ok(policy)
            if firefox_certificate_is_installed(&policy, &certificate)
                && firefox_dns_exclusion_is_installed(&policy) => {}
        Ok(_) => problems
            .push("Firefox policy is missing DuckDNS certificate or split-DNS exclusion".into()),
        Err(error) => problems.push(error),
    }
}

fn remove_firefox_policy(id: &str) -> Result<(), String> {
    let marker_path = firefox_policy_marker(id);
    if !marker_path.exists() {
        return Ok(());
    }
    ensure_owned(&marker_path, id)?;
    let marker_contents = std::fs::read_to_string(&marker_path)
        .map_err(|error| format!("read {}: {error}", marker_path.display()))?;
    let ownership = FirefoxPolicyOwnership::from_marker(&marker_contents);
    let policy_path = Path::new(FIREFOX_POLICY);
    if policy_path.exists() {
        let mut policy = read_firefox_policy(policy_path)?;
        let certificate = firefox_certificate(id);
        let removed_certificate = remove_firefox_certificate(&mut policy, &certificate)?;
        let removed_legacy_certificate =
            remove_firefox_certificate(&mut policy, &trust_anchor(id)?)?;
        let removed_exclusion =
            ownership.added_dns_exclusion && remove_firefox_dns_exclusion(&mut policy)?;
        let removed_created_fields = remove_created_firefox_fields(&mut policy, ownership)?;
        if removed_certificate
            || removed_legacy_certificate
            || removed_exclusion
            || removed_created_fields
        {
            if ownership.created_policy && firefox_policy_is_empty(&policy) {
                remove_file_if_present(policy_path)?;
            } else {
                write_shared_json(policy_path, &policy)?;
            }
        }
    }
    remove_owned(&marker_path, id)
}

fn read_firefox_policy(path: &Path) -> Result<serde_json::Value, String> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|error| format!("parse Firefox policy {}: {error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(serde_json::json!({ "policies": {} }))
        }
        Err(error) => Err(format!("read Firefox policy {}: {error}", path.display())),
    }
}

fn add_firefox_certificate(
    policy: &mut serde_json::Value,
    certificate: &Path,
) -> Result<(), String> {
    let certificates = firefox_certificates(policy)?;
    let install = certificates
        .entry("Install")
        .or_insert_with(|| serde_json::Value::Array(Vec::new()));
    let install = install
        .as_array_mut()
        .ok_or("duckdnsd: Firefox Certificates.Install policy is not an array")?;
    let certificate = serde_json::Value::String(certificate.display().to_string());
    if !install.contains(&certificate) {
        install.push(certificate);
    }
    Ok(())
}

fn remove_firefox_certificate(
    policy: &mut serde_json::Value,
    certificate: &Path,
) -> Result<bool, String> {
    let policies = policy
        .get_mut("policies")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or("duckdnsd: Firefox policy must contain a policies object")?;
    let Some(certificates) = policies
        .get_mut("Certificates")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return Ok(false);
    };
    let Some(install) = certificates
        .get_mut("Install")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return Ok(false);
    };
    let path = certificate.display().to_string();
    let before = install.len();
    install.retain(|entry| entry.as_str() != Some(path.as_str()));
    Ok(install.len() != before)
}

fn add_firefox_dns_exclusion(policy: &mut serde_json::Value) -> Result<bool, String> {
    let dns = firefox_policies(policy)?
        .entry("DNSOverHTTPS")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
        .as_object_mut()
        .ok_or("duckdnsd: Firefox DNSOverHTTPS policy is not an object")?;
    let excluded = dns
        .entry("ExcludedDomains")
        .or_insert_with(|| serde_json::Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or("duckdnsd: Firefox DNSOverHTTPS.ExcludedDomains policy is not an array")?;
    let zone = serde_json::Value::String(duckdns_core::DUCKDNS_ZONE.into());
    if excluded.contains(&zone) {
        Ok(false)
    } else {
        excluded.push(zone);
        Ok(true)
    }
}

fn remove_firefox_dns_exclusion(policy: &mut serde_json::Value) -> Result<bool, String> {
    let policies = policy
        .get_mut("policies")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or("duckdnsd: Firefox policy must contain a policies object")?;
    let Some(excluded) = policies
        .get_mut("DNSOverHTTPS")
        .and_then(serde_json::Value::as_object_mut)
        .and_then(|dns| dns.get_mut("ExcludedDomains"))
        .and_then(serde_json::Value::as_array_mut)
    else {
        return Ok(false);
    };
    let before = excluded.len();
    excluded.retain(|entry| entry.as_str() != Some(duckdns_core::DUCKDNS_ZONE));
    Ok(excluded.len() != before)
}

fn remove_created_firefox_fields(
    policy: &mut serde_json::Value,
    ownership: FirefoxPolicyOwnership,
) -> Result<bool, String> {
    let policies = policy
        .get_mut("policies")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or("duckdnsd: Firefox policy must contain a policies object")?;
    let mut changed = false;

    if ownership.created_certificate_install
        && policies
            .get("Certificates")
            .and_then(serde_json::Value::as_object)
            .and_then(|certificates| certificates.get("Install"))
            .and_then(serde_json::Value::as_array)
            .is_some_and(Vec::is_empty)
        && let Some(certificates) = policies
            .get_mut("Certificates")
            .and_then(serde_json::Value::as_object_mut)
    {
        certificates.remove("Install");
        changed = true;
    }
    if ownership.created_certificates
        && policies
            .get("Certificates")
            .and_then(serde_json::Value::as_object)
            .is_some_and(serde_json::Map::is_empty)
    {
        policies.remove("Certificates");
        changed = true;
    }

    if ownership.created_excluded_domains
        && policies
            .get("DNSOverHTTPS")
            .and_then(serde_json::Value::as_object)
            .and_then(|dns| dns.get("ExcludedDomains"))
            .and_then(serde_json::Value::as_array)
            .is_some_and(Vec::is_empty)
        && let Some(dns) = policies
            .get_mut("DNSOverHTTPS")
            .and_then(serde_json::Value::as_object_mut)
    {
        dns.remove("ExcludedDomains");
        changed = true;
    }
    if ownership.created_dns_over_https
        && policies
            .get("DNSOverHTTPS")
            .and_then(serde_json::Value::as_object)
            .is_some_and(serde_json::Map::is_empty)
    {
        policies.remove("DNSOverHTTPS");
        changed = true;
    }
    Ok(changed)
}

fn firefox_dns_exclusion_is_installed(policy: &serde_json::Value) -> bool {
    policy
        .get("policies")
        .and_then(|policies| policies.get("DNSOverHTTPS"))
        .and_then(|dns| dns.get("ExcludedDomains"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|entries| {
            entries
                .iter()
                .any(|entry| entry.as_str() == Some(duckdns_core::DUCKDNS_ZONE))
        })
}

fn firefox_certificates(
    policy: &mut serde_json::Value,
) -> Result<&mut serde_json::Map<String, serde_json::Value>, String> {
    firefox_policies(policy)?
        .entry("Certificates")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
        .as_object_mut()
        .ok_or_else(|| "duckdnsd: Firefox Certificates policy is not an object".into())
}

fn firefox_policies(
    policy: &mut serde_json::Value,
) -> Result<&mut serde_json::Map<String, serde_json::Value>, String> {
    policy
        .as_object_mut()
        .ok_or("duckdnsd: Firefox policy root is not an object")?
        .entry("policies")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
        .as_object_mut()
        .ok_or_else(|| "duckdnsd: Firefox policies value is not an object".into())
}

fn firefox_certificate_is_installed(policy: &serde_json::Value, certificate: &Path) -> bool {
    let path = certificate.display().to_string();
    policy
        .get("policies")
        .and_then(|policies| policies.get("Certificates"))
        .and_then(|certificates| certificates.get("Install"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|entries| entries.iter().any(|entry| entry.as_str() == Some(&path)))
}

fn firefox_policy_is_empty(policy: &serde_json::Value) -> bool {
    let Some(root) = policy.as_object() else {
        return false;
    };
    if root.len() != 1 || !root.contains_key("policies") {
        return false;
    }
    let Some(policies) = policy
        .get("policies")
        .and_then(serde_json::Value::as_object)
    else {
        return false;
    };
    policies.iter().all(|(name, value)| {
        (name == "Certificates"
            && value.as_object().is_some_and(|certificates| {
                certificates.iter().all(|(name, value)| {
                    name == "Install" && value.as_array().is_some_and(|install| install.is_empty())
                })
            }))
            || (name == "DNSOverHTTPS"
                && value.as_object().is_some_and(|dns| {
                    dns.iter().all(|(name, value)| {
                        name == "ExcludedDomains"
                            && value.as_array().is_some_and(|domains| domains.is_empty())
                    })
                }))
    })
}

fn write_shared_json(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    if std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(format!(
            "duckdnsd: refusing to replace symlinked Firefox policy {}",
            path.display()
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create {}: {error}", parent.display()))?;
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("encode Firefox policy: {error}"))?;
    bytes.push(b'\n');
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    let _ = std::fs::remove_file(&temporary);
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("write {}: {error}", temporary.display()))?;
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o644))
        .map_err(|error| format!("chmod {}: {error}", temporary.display()))?;
    std::fs::rename(&temporary, path)
        .map_err(|error| format!("install {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn firefox_policy_merge_preserves_foreign_settings() {
        let certificate = Path::new("/tmp/ducktape-root.pem");
        let mut policy = serde_json::json!({
            "policies": {
                "DisableTelemetry": true,
                "Certificates": { "Install": ["company.pem"] },
                "DNSOverHTTPS": { "ExcludedDomains": ["corp.example"] }
            }
        });

        add_firefox_certificate(&mut policy, certificate).unwrap();
        add_firefox_certificate(&mut policy, certificate).unwrap();
        assert!(add_firefox_dns_exclusion(&mut policy).unwrap());
        assert!(!add_firefox_dns_exclusion(&mut policy).unwrap());
        assert!(firefox_certificate_is_installed(&policy, certificate));
        assert!(firefox_dns_exclusion_is_installed(&policy));
        assert_eq!(
            policy["policies"]["Certificates"]["Install"],
            serde_json::json!(["company.pem", "/tmp/ducktape-root.pem"])
        );
        assert_eq!(
            policy["policies"]["DNSOverHTTPS"]["ExcludedDomains"],
            serde_json::json!(["corp.example", "ducktape.quack"])
        );
        assert_eq!(policy["policies"]["DisableTelemetry"], true);

        assert!(remove_firefox_certificate(&mut policy, certificate).unwrap());
        assert!(remove_firefox_dns_exclusion(&mut policy).unwrap());
        assert_eq!(
            policy["policies"]["Certificates"]["Install"],
            serde_json::json!(["company.pem"])
        );
        assert_eq!(
            policy["policies"]["DNSOverHTTPS"]["ExcludedDomains"],
            serde_json::json!(["corp.example"])
        );
        assert_eq!(policy["policies"]["DisableTelemetry"], true);
        assert!(!firefox_policy_is_empty(&policy));
    }

    #[test]
    fn helper_created_empty_firefox_policy_can_be_removed() {
        let certificate = Path::new("/tmp/ducktape-root.pem");
        let mut policy = serde_json::json!({ "policies": {} });
        let ownership = FirefoxPolicyOwnership::before_install(
            Path::new("/definitely-missing-duckdns-policy"),
            &policy,
        );
        add_firefox_certificate(&mut policy, certificate).unwrap();
        add_firefox_dns_exclusion(&mut policy).unwrap();
        remove_firefox_certificate(&mut policy, certificate).unwrap();
        remove_firefox_dns_exclusion(&mut policy).unwrap();
        remove_created_firefox_fields(&mut policy, ownership).unwrap();
        assert!(firefox_policy_is_empty(&policy));
    }

    #[test]
    fn foreign_firefox_policy_shape_is_restored() {
        let certificate = Path::new("/tmp/ducktape-root.pem");
        let original = serde_json::json!({
            "policies": {
                "DisableTelemetry": true,
                "DNSOverHTTPS": { "ExcludedDomains": ["corp.example"] }
            }
        });
        let mut policy = original.clone();
        let mut ownership = FirefoxPolicyOwnership::before_install(Path::new("/"), &policy);

        add_firefox_certificate(&mut policy, certificate).unwrap();
        ownership.added_dns_exclusion = add_firefox_dns_exclusion(&mut policy).unwrap();
        remove_firefox_certificate(&mut policy, certificate).unwrap();
        remove_firefox_dns_exclusion(&mut policy).unwrap();
        remove_created_firefox_fields(&mut policy, ownership).unwrap();

        assert_eq!(policy, original);
    }
}
