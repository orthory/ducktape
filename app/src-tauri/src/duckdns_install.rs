//! Desktop-facing opt-in installation of the privileged `duckdnsd` sidecar.
//!
//! The elevated process accepts only fixed install/repair/uninstall verbs.
//! The app creates the control credential as the current user before elevation;
//! the system helper reads it and stores its own protected copy.

use std::path::Path;
#[cfg(target_os = "linux")]
use std::path::PathBuf;
use std::process::Command;

use serde::Serialize;
#[cfg(target_os = "linux")]
use tauri::Manager as _;

use crate::daemon::require_main_window;
use crate::duckdns::{Registration, client_token_path, deactivate_async};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    pub installation: serde_json::Value,
    pub warnings: Vec<String>,
}

#[tauri::command]
pub async fn duckdns_install(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<InstallResult, String> {
    require_main_window(&window)?;
    install_or_repair(&app, "install").await
}

#[tauri::command]
pub async fn duckdns_repair(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<InstallResult, String> {
    require_main_window(&window)?;
    install_or_repair(&app, "repair").await
}

#[tauri::command]
pub async fn duckdns_remove(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    registration: tauri::State<'_, Registration>,
) -> Result<InstallResult, String> {
    require_main_window(&window)?;
    let clear_warning = deactivate_async(&registration).await.err();
    let before = helper_installation_status().ok();
    let helper = crate::daemon::resolve_duckdnsd_bin()?;
    tauri::async_runtime::spawn_blocking(move || run_privileged(&helper, &["uninstall"]))
        .await
        .map_err(|error| format!("join DuckDNS uninstall: {error}"))??;
    let mut warnings = remove_nss_trust(&app, before.as_ref());
    if let Some(error) = clear_warning {
        warnings.push(format!(
            "clear active DuckDNS lease before removal: {error}"
        ));
    }
    if std::env::var_os("DUCKTAPE_DUCKDNS_STATE").is_none()
        && let Ok(path) = client_token_path(&app)
        && let Err(error) = std::fs::remove_file(&path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        warnings.push(format!("remove {}: {error}", path.display()));
    }
    Ok(InstallResult {
        installation: helper_installation_status()?,
        warnings,
    })
}

async fn install_or_repair(
    app: &tauri::AppHandle,
    operation: &'static str,
) -> Result<InstallResult, String> {
    let token = client_token_path(app)?;
    let client_state = token
        .parent()
        .ok_or("DuckDNS client token path has no parent")?;
    duckdnsd::load_or_create_token(client_state)
        .map_err(|error| format!("create DuckDNS app control token: {error}"))?;
    let helper = crate::daemon::resolve_duckdnsd_bin()?;
    let token_arg = token
        .to_str()
        .ok_or("DuckDNS client token path is not UTF-8")?
        .to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        run_privileged(&helper, &[operation, "--client-token", &token_arg])
    })
    .await
    .map_err(|error| format!("join DuckDNS {operation}: {error}"))??;
    let installation = helper_installation_status()?;
    let warnings = install_nss_trust(app, &installation);
    Ok(InstallResult {
        installation,
        warnings,
    })
}

pub(crate) fn helper_installation_status() -> Result<serde_json::Value, String> {
    let helper = crate::daemon::resolve_duckdnsd_bin()?;
    let output = Command::new(&helper)
        .arg("install-status")
        .output()
        .map_err(|error| format!("run {} install-status: {error}", helper.display()))?;
    if !output.status.success() {
        return Err(format!(
            "DuckDNS install-status exited {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("decode DuckDNS install-status: {error}"))
}

fn run_privileged(helper: &Path, arguments: &[&str]) -> Result<(), String> {
    if direct_privileged_allowed() {
        return run_command(Command::new(helper), arguments, "DuckDNS privileged helper");
    }

    #[cfg(target_os = "linux")]
    {
        let mut command = Command::new("pkexec");
        command.arg(helper);
        return run_command(command, arguments, "pkexec DuckDNS helper");
    }
    #[cfg(target_os = "macos")]
    {
        let mut words = Vec::with_capacity(arguments.len() + 1);
        words.push(shell_quote(&helper.display().to_string()));
        words.extend(arguments.iter().map(|argument| shell_quote(argument)));
        let shell = words.join(" ");
        let script = format!(
            "do shell script \"{}\" with administrator privileges",
            shell.replace('\\', "\\\\").replace('"', "\\\"")
        );
        let mut command = Command::new("osascript");
        command.arg("-e").arg(script);
        return run_command(command, &[], "macOS DuckDNS authorization");
    }
    #[cfg(target_os = "windows")]
    {
        let file = ps_quote(&helper.display().to_string());
        let args = arguments
            .iter()
            .map(|argument| ps_quote(argument))
            .collect::<Vec<_>>()
            .join(",");
        let script = format!(
            "$p=Start-Process -Verb RunAs -Wait -PassThru -FilePath {file} -ArgumentList @({args}); exit $p.ExitCode"
        );
        let mut command = Command::new("powershell.exe");
        command
            .arg("-NoProfile")
            .arg("-NonInteractive")
            .arg("-Command")
            .arg(script);
        return run_command(command, &[], "Windows DuckDNS authorization");
    }
    #[allow(unreachable_code)]
    Err("DuckDNS privileged installation is unsupported on this OS".into())
}

fn run_command(mut command: Command, arguments: &[&str], label: &str) -> Result<(), String> {
    let output = command
        .args(arguments)
        .output()
        .map_err(|error| format!("run {label}: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{label} exited {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn direct_privileged_allowed() -> bool {
    if std::env::var_os("DUCKTAPE_DUCKDNS_NO_ELEVATE").is_some() {
        return true;
    }
    #[cfg(unix)]
    {
        Command::new("id")
            .arg("-u")
            .output()
            .is_ok_and(|output| output.status.success() && output.stdout == b"0\n")
    }
    #[cfg(not(unix))]
    {
        false
    }
}

#[cfg(target_os = "macos")]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(target_os = "windows")]
fn ps_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(target_os = "linux")]
fn install_nss_trust(app: &tauri::AppHandle, installation: &serde_json::Value) -> Vec<String> {
    let Some(id) = installation
        .get("installation_id")
        .and_then(serde_json::Value::as_str)
    else {
        return vec!["DuckDNS installation did not report an installation id".into()];
    };
    let Some(certificate) = installation
        .get("root_certificate")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
    else {
        return vec!["DuckDNS installation did not report its public root certificate".into()];
    };
    let nickname = format!("Ducktape DuckDNS {id}");
    nss_databases(app)
        .into_iter()
        .filter_map(|database| {
            if let Err(error) = initialize_nss_database(&database) {
                return Some(error);
            }
            let argument = format!("sql:{}", database.display());
            // Repair is intentionally idempotent. NSS refuses to add a
            // certificate under an existing nickname, so discard only our
            // installation-scoped entry before replacing it.
            let _ = Command::new("certutil")
                .args(["-D", "-d", &argument, "-n", &nickname])
                .output();
            let output = Command::new("certutil")
                .args(["-A", "-d", &argument, "-n", &nickname, "-t", "C,,", "-i"])
                .arg(&certificate)
                .output();
            match output {
                Ok(output) if output.status.success() => None,
                Ok(output) => Some(format!(
                    "NSS trust for {} failed: {}",
                    database.display(),
                    String::from_utf8_lossy(&output.stderr).trim()
                )),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(
                    "certutil is unavailable; Chromium/Firefox NSS trust was not installed".into(),
                ),
                Err(error) => Some(format!("run certutil: {error}")),
            }
        })
        .collect()
}

#[cfg(not(target_os = "linux"))]
fn install_nss_trust(_app: &tauri::AppHandle, _installation: &serde_json::Value) -> Vec<String> {
    Vec::new()
}

#[cfg(target_os = "linux")]
fn remove_nss_trust(
    app: &tauri::AppHandle,
    installation: Option<&serde_json::Value>,
) -> Vec<String> {
    let Some(id) = installation
        .and_then(|value| value.get("installation_id"))
        .and_then(serde_json::Value::as_str)
    else {
        return Vec::new();
    };
    let nickname = format!("Ducktape DuckDNS {id}");
    nss_databases(app)
        .into_iter()
        .filter(|database| database.join("cert9.db").exists())
        .filter_map(|database| {
            let argument = format!("sql:{}", database.display());
            match Command::new("certutil")
                .args(["-D", "-d", &argument, "-n", &nickname])
                .output()
            {
                Ok(output) if output.status.success() => None,
                // A browser database may have been removed or repaired since
                // installation. Missing Ducktape trust is already the desired
                // uninstall state.
                Ok(output)
                    if String::from_utf8_lossy(&output.stderr)
                        .contains("SEC_ERROR_BAD_DATABASE")
                        || String::from_utf8_lossy(&output.stderr)
                            .contains("SEC_ERROR_UNKNOWN_CERT") =>
                {
                    None
                }
                Ok(output) => Some(format!(
                    "remove NSS trust for {} failed: {}",
                    database.display(),
                    String::from_utf8_lossy(&output.stderr).trim()
                )),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => Some(format!("run certutil: {error}")),
            }
        })
        .collect()
}

#[cfg(not(target_os = "linux"))]
fn remove_nss_trust(
    _app: &tauri::AppHandle,
    _installation: Option<&serde_json::Value>,
) -> Vec<String> {
    Vec::new()
}

#[cfg(target_os = "linux")]
fn nss_databases(app: &tauri::AppHandle) -> Vec<PathBuf> {
    let Ok(home) = app.path().home_dir() else {
        return Vec::new();
    };
    nss_databases_in(&home, std::env::var_os("XDG_DATA_HOME").as_deref())
}

#[cfg(target_os = "linux")]
fn nss_databases_in(home: &Path, xdg_data_home: Option<&std::ffi::OsStr>) -> Vec<PathBuf> {
    // Chromium M146+ moved the shared NSS database under XDG data. Preserve
    // Chromium's legacy selection rule when ~/.pki/nssdb already exists.
    let legacy_chromium = home.join(".pki/nssdb");
    let chromium = if legacy_chromium.exists() {
        legacy_chromium
    } else {
        xdg_data_home
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share"))
            .join("pki/nssdb")
    };
    let mut databases = vec![chromium];
    let root = home.join(".mozilla/firefox");
    if let Ok(entries) = std::fs::read_dir(root) {
        databases.extend(
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.is_dir() && path.join("cert9.db").exists()),
        );
    }
    databases.sort();
    databases.dedup();
    databases
}

#[cfg(target_os = "linux")]
fn initialize_nss_database(database: &Path) -> Result<(), String> {
    if database.join("cert9.db").exists() {
        return Ok(());
    }
    std::fs::create_dir_all(database)
        .map_err(|error| format!("create NSS database {}: {error}", database.display()))?;
    let argument = format!("sql:{}", database.display());
    let output = Command::new("certutil")
        .args(["-N", "--empty-password", "-d", &argument])
        .output()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                "certutil is unavailable; Chromium/Firefox NSS trust was not installed".into()
            } else {
                format!("run certutil: {error}")
            }
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "initialize NSS database {} failed: {}",
            database.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[cfg(all(test, target_os = "linux"))]
mod linux_nss_tests {
    use super::*;

    #[test]
    fn discovers_chromium_and_existing_firefox_databases() {
        let directory = tempfile::tempdir().unwrap();
        let home = directory.path().join("home");
        let xdg = directory.path().join("xdg");
        let firefox = home.join(".mozilla/firefox/member.default");
        std::fs::create_dir_all(&firefox).unwrap();
        std::fs::write(firefox.join("cert9.db"), []).unwrap();

        assert_eq!(
            nss_databases_in(&home, Some(xdg.as_os_str())),
            vec![firefox, xdg.join("pki/nssdb")]
        );
    }

    #[test]
    fn preserves_chromiums_existing_legacy_database() {
        let directory = tempfile::tempdir().unwrap();
        let home = directory.path();
        let legacy = home.join(".pki/nssdb");
        std::fs::create_dir_all(&legacy).unwrap();

        assert_eq!(nss_databases_in(home, None), vec![legacy]);
    }
}
