//! Fixed privileged installation operations for the device helper.
//!
//! The desktop may select only these verbs. Every mutable OS artifact carries
//! the CA installation id, and removal refuses to touch a fixed-name artifact
//! that does not carry that id.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde::Serialize;

use crate::ca::INSTALLATION_ID_FILE;
use crate::{
    CaStore, ROOT_CERT_FILE, control_token_path, default_state_dir, install_token, read_token,
};

const ARTIFACT_MARKER: &str = "Ducktape-DuckDNS-Installation";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InstallationStatus {
    pub installed: bool,
    pub healthy: bool,
    pub installation_id: Option<String>,
    pub root_certificate: Option<String>,
    pub problems: Vec<String>,
}

/// Install or repair the helper from the currently running, elevated binary.
/// `client_token` is created by the unprivileged desktop under its own app-data
/// directory; the helper only reads it and installs a private system copy.
pub fn install(client_token: &Path, repair: bool) -> Result<InstallationStatus, String> {
    let token = read_token(client_token)?;
    let state_dir = default_state_dir();
    let existed = state_dir.join(INSTALLATION_ID_FILE).exists();
    if existed && !repair {
        let status = installation_status();
        if status.healthy {
            return Ok(status);
        }
        return Err("duckdnsd: an incomplete installation exists; run repair".into());
    }

    #[cfg(unix)]
    if repair {
        use std::os::unix::fs::PermissionsExt as _;
        for path in [
            state_dir.join(crate::ROOT_KEY_FILE),
            control_token_path(&state_dir),
        ] {
            if path.exists() {
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).map_err(
                    |error| format!("repair permissions on {}: {error}", path.display()),
                )?;
            }
        }
    }

    let ca = CaStore::load_or_create(&state_dir)?;
    let id = ca.installation_id().to_owned();
    let result = (|| {
        platform::stop_owned(&id)?;
        install_token(&state_dir, &token)
            .map_err(|error| format!("install DuckDNS control token: {error}"))?;
        platform::protect_state(&state_dir)?;
        install_binary(&id)?;
        platform::apply(&id, &state_dir.join(ROOT_CERT_FILE))
    })();
    if let Err(error) = result {
        if !existed {
            let _ = platform::remove(&id);
            let _ = remove_binary(&id);
            let _ = std::fs::remove_dir_all(&state_dir);
        }
        return Err(error);
    }
    let status = installation_status();
    if status.healthy {
        Ok(status)
    } else {
        Err(format!(
            "duckdnsd: installation did not verify: {}",
            status.problems.join("; ")
        ))
    }
}

pub fn uninstall() -> Result<InstallationStatus, String> {
    let state_dir = default_state_dir();
    let Some(id) = read_installation_id(&state_dir)? else {
        return Ok(InstallationStatus {
            installed: false,
            healthy: true,
            installation_id: None,
            root_certificate: None,
            problems: Vec::new(),
        });
    };
    platform::remove(&id)?;
    remove_binary(&id)?;
    // The id was read from this exact directory and validated above. Never
    // recursively remove an unmarked path.
    std::fs::remove_dir_all(&state_dir)
        .map_err(|error| format!("remove DuckDNS state {}: {error}", state_dir.display()))?;
    Ok(installation_status())
}

pub fn installation_status() -> InstallationStatus {
    let state_dir = default_state_dir();
    let id = match read_installation_id(&state_dir) {
        Ok(Some(id)) => id,
        Ok(None) => {
            return InstallationStatus {
                installed: false,
                healthy: true,
                installation_id: None,
                root_certificate: None,
                problems: Vec::new(),
            };
        }
        Err(error) => {
            return InstallationStatus {
                installed: true,
                healthy: false,
                installation_id: None,
                root_certificate: None,
                problems: vec![error],
            };
        }
    };
    let mut problems = Vec::new();
    for path in [
        state_dir.join(ROOT_CERT_FILE),
        state_dir.join(crate::ROOT_KEY_FILE),
        control_token_path(&state_dir),
    ] {
        if !path.is_file() {
            problems.push(format!("missing {}", path.display()));
        }
    }
    #[cfg(unix)]
    for path in [
        state_dir.join(crate::ROOT_KEY_FILE),
        control_token_path(&state_dir),
    ] {
        use std::os::unix::fs::PermissionsExt as _;
        if let Ok(metadata) = std::fs::metadata(&path) {
            let mode = metadata.permissions().mode() & 0o777;
            if mode & 0o077 != 0 {
                problems.push(format!("{} has unsafe mode {mode:o}", path.display()));
            }
        }
    }
    verify_binary(&id, &mut problems);
    platform::inspect(&id, &mut problems);
    InstallationStatus {
        installed: true,
        healthy: problems.is_empty(),
        installation_id: Some(id.clone()),
        root_certificate: platform::public_certificate(&id).map(|path| path.display().to_string()),
        problems,
    }
}

fn read_installation_id(state_dir: &Path) -> Result<Option<String>, String> {
    let path = state_dir.join(INSTALLATION_ID_FILE);
    let value = match std::fs::read_to_string(&path) {
        Ok(value) => value.trim().to_owned(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "read DuckDNS installation id {}: {error}",
                path.display()
            ));
        }
    };
    if value.len() != 32
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("duckdnsd: installation id is corrupt".into());
    }
    Ok(Some(value))
}

fn marker(id: &str) -> String {
    format!("{ARTIFACT_MARKER}={id}")
}

fn binary_marker_path() -> PathBuf {
    let binary = platform::binary_path();
    let name = binary
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("duckdnsd");
    binary.with_file_name(format!("{name}.installation-id"))
}

fn install_binary(id: &str) -> Result<(), String> {
    let target = platform::binary_path();
    let marker_path = binary_marker_path();
    ensure_owned_or_absent(&marker_path, id)?;
    write_owned(&marker_path, format!("{}\n", marker(id)).as_bytes(), id)?;
    let source = std::env::current_exe().map_err(|error| format!("locate duckdnsd: {error}"))?;
    if source.canonicalize().ok() != target.canonicalize().ok() {
        let parent = target
            .parent()
            .ok_or_else(|| "duckdnsd: install path has no parent".to_string())?;
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
        let temporary = target.with_extension(format!("new-{}", std::process::id()));
        let _ = std::fs::remove_file(&temporary);
        std::fs::copy(&source, &temporary).map_err(|error| {
            format!(
                "copy DuckDNS helper {} to {}: {error}",
                source.display(),
                temporary.display()
            )
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o755))
                .map_err(|error| format!("chmod {}: {error}", temporary.display()))?;
        }
        #[cfg(not(unix))]
        if target.exists() {
            ensure_owned_or_absent(&marker_path, id)?;
            std::fs::remove_file(&target)
                .map_err(|error| format!("replace {}: {error}", target.display()))?;
        }
        std::fs::rename(&temporary, &target)
            .map_err(|error| format!("install {}: {error}", target.display()))?;
    }
    Ok(())
}

fn verify_binary(id: &str, problems: &mut Vec<String>) {
    let binary = platform::binary_path();
    if !binary.is_file() {
        problems.push(format!("missing installed helper {}", binary.display()));
    }
    let marker_path = binary_marker_path();
    match artifact_owned(&marker_path, id) {
        Ok(true) => {}
        Ok(false) => problems.push(format!("unowned helper marker {}", marker_path.display())),
        Err(error) => problems.push(error),
    }
}

fn remove_binary(id: &str) -> Result<(), String> {
    let marker_path = binary_marker_path();
    if !marker_path.exists() {
        if platform::binary_path().exists() {
            return Err(format!(
                "duckdnsd: refusing to remove unmarked helper {}",
                platform::binary_path().display()
            ));
        }
        return Ok(());
    }
    ensure_owned(&marker_path, id)?;
    remove_file_if_present(&platform::binary_path())?;
    remove_file_if_present(&marker_path)
}

fn write_owned(path: &Path, bytes: &[u8], id: &str) -> Result<(), String> {
    ensure_owned_or_absent(path, id)?;
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create {}: {error}", parent.display()))?;
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    let _ = std::fs::remove_file(&temporary);
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("write {}: {error}", temporary.display()))?;
    #[cfg(unix)]
    {
        std::fs::rename(&temporary, path)
            .map_err(|error| format!("install {}: {error}", path.display()))
    }
    #[cfg(not(unix))]
    {
        if path.exists() {
            std::fs::remove_file(path)
                .map_err(|error| format!("replace {}: {error}", path.display()))?;
        }
        std::fs::rename(&temporary, path)
            .map_err(|error| format!("install {}: {error}", path.display()))
    }
}

fn ensure_owned_or_absent(path: &Path, id: &str) -> Result<(), String> {
    if !path.exists() || artifact_owned(path, id)? {
        Ok(())
    } else {
        Err(format!(
            "duckdnsd: refusing to replace unowned artifact {}",
            path.display()
        ))
    }
}

fn ensure_owned(path: &Path, id: &str) -> Result<(), String> {
    if artifact_owned(path, id)? {
        Ok(())
    } else {
        Err(format!(
            "duckdnsd: refusing to remove unowned artifact {}",
            path.display()
        ))
    }
}

fn artifact_owned(path: &Path, id: &str) -> Result<bool, String> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(bytes
            .windows(marker(id).len())
            .any(|window| window == marker(id).as_bytes())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("read {}: {error}", path.display())),
    }
}

fn remove_owned(path: &Path, id: &str) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    ensure_owned(path, id)?;
    remove_file_if_present(path)
}

fn remove_file_if_present(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("remove {}: {error}", path.display())),
    }
}

fn run(program: &str, arguments: &[&str]) -> Result<(), String> {
    let output = Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| format!("run {program}: {error}"))?;
    check_output(program, output)
}

fn run_allow_failure(program: &str, arguments: &[&str]) {
    let _ = Command::new(program).args(arguments).status();
}

fn check_output(program: &str, output: Output) -> Result<(), String> {
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr);
    Err(format!(
        "{program} exited {}: {}",
        output.status,
        detail.trim()
    ))
}

#[cfg(target_os = "linux")]
#[path = "install/linux.rs"]
mod platform;

#[cfg(target_os = "macos")]
mod platform {
    use super::*;

    const BINARY: &str = "/Library/PrivilegedHelperTools/com.ducktape.duckdnsd";
    const PLIST: &str = "/Library/LaunchDaemons/com.ducktape.duckdnsd.plist";
    const RESOLVER: &str = "/etc/resolver/ducktape.quack";

    pub(super) fn binary_path() -> PathBuf {
        BINARY.into()
    }

    pub(super) fn public_certificate(_id: &str) -> Option<PathBuf> {
        Some(default_state_dir().join(ROOT_CERT_FILE))
    }

    pub(super) fn stop_owned(id: &str) -> Result<(), String> {
        if Path::new(PLIST).exists() {
            ensure_owned(Path::new(PLIST), id)?;
            run_allow_failure("launchctl", &["bootout", "system", PLIST]);
        }
        Ok(())
    }

    pub(super) fn protect_state(state_dir: &Path) -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(state_dir, std::fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("chmod {}: {error}", state_dir.display()))
    }

    pub(super) fn apply(id: &str, root_cert: &Path) -> Result<(), String> {
        let plist = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<!-- {} -->\n<plist version=\"1.0\"><dict><key>Label</key><string>com.ducktape.duckdnsd</string><key>ProgramArguments</key><array><string>{BINARY}</string><string>serve</string></array><key>RunAtLoad</key><true/><key>KeepAlive</key><true/><key>DucktapeInstallationID</key><string>{id}</string></dict></plist>\n",
            marker(id)
        );
        write_owned(Path::new(PLIST), plist.as_bytes(), id)?;
        let resolver = format!(
            "# {}\nnameserver 127.77.0.1\nport 53\nsearch_order 1\n",
            marker(id)
        );
        write_owned(Path::new(RESOLVER), resolver.as_bytes(), id)?;
        let fingerprint = certificate_fingerprint(root_cert)?;
        run_allow_failure(
            "security",
            &[
                "delete-certificate",
                "-Z",
                &fingerprint,
                "/Library/Keychains/System.keychain",
            ],
        );
        run(
            "security",
            &[
                "add-trusted-cert",
                "-d",
                "-r",
                "trustRoot",
                "-k",
                "/Library/Keychains/System.keychain",
                root_cert
                    .to_str()
                    .ok_or("duckdnsd: root CA path is not UTF-8")?,
            ],
        )?;
        run("launchctl", &["bootstrap", "system", PLIST])
    }

    pub(super) fn inspect(id: &str, problems: &mut Vec<String>) {
        inspect_owned(Path::new(PLIST), id, problems);
        inspect_owned(Path::new(RESOLVER), id, problems);
        if !matches!(
            Command::new("launchctl")
                .args(["print", "system/com.ducktape.duckdnsd"])
                .output(),
            Ok(output) if output.status.success()
        ) {
            problems.push("DuckDNS launch daemon is not active".into());
        }
    }

    pub(super) fn remove(id: &str) -> Result<(), String> {
        stop_owned(id)?;
        remove_owned(Path::new(PLIST), id)?;
        remove_owned(Path::new(RESOLVER), id)?;
        let root = default_state_dir().join(ROOT_CERT_FILE);
        if root.exists()
            && let Ok(fingerprint) = certificate_fingerprint(&root)
        {
            run_allow_failure(
                "security",
                &[
                    "delete-certificate",
                    "-Z",
                    &fingerprint,
                    "/Library/Keychains/System.keychain",
                ],
            );
        }
        Ok(())
    }

    fn certificate_fingerprint(certificate: &Path) -> Result<String, String> {
        let output = Command::new("/usr/bin/openssl")
            .args(["x509", "-in"])
            .arg(certificate)
            .args(["-noout", "-fingerprint", "-sha1"])
            .output()
            .map_err(|error| format!("run openssl for DuckDNS root fingerprint: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "openssl exited {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let text = String::from_utf8(output.stdout)
            .map_err(|_| "duckdnsd: openssl fingerprint output is not UTF-8")?;
        let fingerprint = text
            .trim()
            .rsplit_once('=')
            .map(|(_, value)| value.replace(':', ""))
            .ok_or("duckdnsd: could not parse root certificate fingerprint")?;
        if fingerprint.len() != 40 || !fingerprint.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("duckdnsd: invalid root certificate SHA-1 fingerprint".into());
        }
        Ok(fingerprint)
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::*;

    const SERVICE: &str = "DucktapeDuckDNS";

    pub(super) fn binary_path() -> PathBuf {
        std::env::var_os("ProgramFiles")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"))
            .join("Ducktape")
            .join("duckdnsd.exe")
    }

    pub(super) fn public_certificate(_id: &str) -> Option<PathBuf> {
        Some(default_state_dir().join(ROOT_CERT_FILE))
    }

    pub(super) fn stop_owned(id: &str) -> Result<(), String> {
        ensure_owned_or_absent(&binary_marker_path(), id)?;
        run_allow_failure("sc.exe", &["stop", SERVICE]);
        Ok(())
    }

    pub(super) fn protect_state(state_dir: &Path) -> Result<(), String> {
        let path = state_dir
            .to_str()
            .ok_or("duckdnsd: state path is not UTF-8")?;
        run(
            "icacls.exe",
            &[
                path,
                "/inheritance:r",
                "/grant:r",
                "SYSTEM:(OI)(CI)F",
                "Administrators:(OI)(CI)F",
            ],
        )
    }

    pub(super) fn apply(id: &str, root_cert: &Path) -> Result<(), String> {
        let binary = binary_path().display().to_string();
        run_allow_failure("sc.exe", &["delete", SERVICE]);
        let bin_path = format!("\"{binary}\" serve");
        run(
            "sc.exe",
            &[
                "create",
                SERVICE,
                "start=",
                "auto",
                "binPath=",
                &bin_path,
                "DisplayName=",
                "Ducktape DuckDNS",
            ],
        )?;
        let root = ps_quote(&root_cert.display().to_string());
        let comment = ps_quote(&marker(id));
        let id = ps_quote(id);
        let script = format!(
            "$owned={comment}; $foreign=Get-DnsClientNrptRule -ErrorAction SilentlyContinue | Where-Object {{$_.Namespace -contains '.ducktape.quack' -and $_.Comment -ne $owned}}; if($foreign){{throw 'unowned NRPT rule for ducktape.quack'}}; Get-DnsClientNrptRule -ErrorAction SilentlyContinue | Where-Object {{$_.Comment -eq $owned}} | Remove-DnsClientNrptRule -Force; Add-DnsClientNrptRule -Namespace '.ducktape.quack' -NameServers '127.77.0.1' -Comment $owned; Import-Certificate -FilePath {root} -CertStoreLocation 'Cert:\\LocalMachine\\Root' | Out-Null; $cert=Get-ChildItem 'Cert:\\LocalMachine\\Root' | Where-Object {{$_.Subject -like ('*OU=duckdnsd:' + {id} + '*')}}; if(-not $cert){{throw 'DuckDNS root certificate did not install'}}"
        );
        run(
            "powershell.exe",
            &["-NoProfile", "-NonInteractive", "-Command", &script],
        )?;
        run("sc.exe", &["start", SERVICE])
    }

    pub(super) fn inspect(id: &str, problems: &mut Vec<String>) {
        inspect_owned(&binary_marker_path(), id, problems);
        if !matches!(
            Command::new("sc.exe").args(["query", SERVICE]).output(),
            Ok(output) if output.status.success()
        ) {
            problems.push("DuckDNS Windows service is not installed".into());
        }
        let comment = ps_quote(&marker(id));
        let script = format!(
            "if(-not (Get-DnsClientNrptRule -ErrorAction SilentlyContinue | Where-Object {{$_.Comment -eq {comment}}})){{exit 1}}"
        );
        if !matches!(
            Command::new("powershell.exe")
                .args(["-NoProfile", "-NonInteractive", "-Command", &script])
                .output(),
            Ok(output) if output.status.success()
        ) {
            problems.push("missing owned Windows NRPT rule".into());
        }
    }

    pub(super) fn remove(id: &str) -> Result<(), String> {
        stop_owned(id)?;
        run_allow_failure("sc.exe", &["delete", SERVICE]);
        let comment = ps_quote(&marker(id));
        let id = ps_quote(id);
        let script = format!(
            "Get-DnsClientNrptRule -ErrorAction SilentlyContinue | Where-Object {{$_.Comment -eq {comment}}} | Remove-DnsClientNrptRule -Force; Get-ChildItem 'Cert:\\LocalMachine\\Root' | Where-Object {{$_.Subject -like ('*OU=duckdnsd:' + {id} + '*')}} | Remove-Item -Force"
        );
        run(
            "powershell.exe",
            &["-NoProfile", "-NonInteractive", "-Command", &script],
        )
    }

    fn ps_quote(value: &str) -> String {
        format!("'{}'", value.replace('\'', "''"))
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
mod platform {
    use super::*;

    pub(super) fn binary_path() -> PathBuf {
        PathBuf::from("duckdnsd")
    }
    pub(super) fn public_certificate(_id: &str) -> Option<PathBuf> {
        None
    }
    pub(super) fn stop_owned(_id: &str) -> Result<(), String> {
        Err("duckdnsd: installation is unsupported on this OS".into())
    }
    pub(super) fn protect_state(_state_dir: &Path) -> Result<(), String> {
        Err("duckdnsd: installation is unsupported on this OS".into())
    }
    pub(super) fn apply(_id: &str, _root_cert: &Path) -> Result<(), String> {
        Err("duckdnsd: installation is unsupported on this OS".into())
    }
    pub(super) fn inspect(_id: &str, problems: &mut Vec<String>) {
        problems.push("installation is unsupported on this OS".into());
    }
    pub(super) fn remove(_id: &str) -> Result<(), String> {
        Err("duckdnsd: installation is unsupported on this OS".into())
    }
}

fn inspect_owned(path: &Path, id: &str, problems: &mut Vec<String>) {
    match artifact_owned(path, id) {
        Ok(true) => {}
        Ok(false) if !path.exists() => problems.push(format!("missing {}", path.display())),
        Ok(false) => problems.push(format!("unowned artifact {}", path.display())),
        Err(error) => problems.push(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ownership_marker_never_matches_another_installation() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("artifact");
        let first = "ab".repeat(16);
        let second = "cd".repeat(16);
        std::fs::write(&path, format!("# {}\n", marker(&first))).unwrap();
        assert!(artifact_owned(&path, &first).unwrap());
        assert!(!artifact_owned(&path, &second).unwrap());
        assert!(ensure_owned(&path, &second).is_err());
    }
}
