//! `ducktape agent install` — the agent CLIs a guest image lends to runs.
//!
//! A run executes inside a Linux microVM on EVERY host, so the CLI baked into
//! the image must be a Linux build for the GUEST's architecture. On Linux the
//! operator's own CLI happens to be exactly that, which is why reading the host
//! `PATH` worked for as long as nobody built an image on a Mac — where the
//! vendor's installer produces a Mach-O binary the guest cannot exec at all.
//!
//! So the binary is acquired deliberately, into `~/.ducktape/executors`, and
//! the node derives the guest's copy from whatever is there
//! (`sandbox_host::executor_image`). This verb owns that directory, the pinned
//! versions, and the one approved way to fill it.
//!
//! TWO RULES IT EXISTS TO KEEP:
//!
//! 1. NOTHING IS FETCHED WITHOUT THE OPERATOR ASKING FOR IT. Bare
//!    `agent install` shows what is missing and what installing it would
//!    download — the checklist IS the approval, and unchecking everything is a
//!    complete answer. Nothing else in the tree fetches an executable.
//! 2. WHAT IS FETCHED IS VERIFIED AGAINST A VALUE IN THIS FILE. A checksum
//!    published beside an artifact proves only that the download was not
//!    corrupted in flight — it comes from the same place the artifact does.
//!    The expected hash lives here, where changing it is a reviewed diff.
//!
//! WHY THE HASH MATTERS MORE THAN USUAL: this executable runs inside the
//! sandbox that holds the operator's provider credential. The sandbox is what
//! protects the HOST from the CLI; it is not what protects the CREDENTIAL from
//! it.
//!
//! The operator does not have to use this verb at all — dropping their own
//! Linux build into the directory is equally valid, and the image builder's
//! ELF check stays for exactly that case. This is a convenience with a
//! receipt, not a gate.

use std::io::IsTerminal as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};

use crate::cred_cli::ProviderArg;

type InstallResult = Result<(), Box<dyn std::error::Error>>;

/// Every provider that has a guest CLI. Adding one here is what makes it
/// offerable; the download metadata below is what makes it installable.
const ALL: [ProviderArg; 2] = [ProviderArg::Claude, ProviderArg::Codex];

// ---- the pins ---------------------------------------------------------------
// A bump is a reviewed diff: new version, new hashes, both arches.
const CODEX_RELEASE: &str = "rust-v0.150.1";
const CLAUDE_VERSION: &str = "2.1.231";

#[derive(Debug, clap::Args)]
pub(crate) struct InstallArgs {
    /// which CLIs to install (omitted = a checklist of what is missing)
    #[arg(value_name = "NAME")]
    providers: Vec<ProviderArg>,
}

/// The guest's architecture — the HOST's, because there is no cross-hypervisor:
/// a Mac runs an aarch64 guest, an x86 box an x86_64 one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GuestArch {
    Aarch64,
    X86_64,
}

impl GuestArch {
    fn host() -> Result<Self, String> {
        match std::env::consts::ARCH {
            "aarch64" => Ok(Self::Aarch64),
            "x86_64" => Ok(Self::X86_64),
            other => Err(format!("no pinned agent CLIs for guest arch {other}")),
        }
    }

    /// the arch as the vendors' asset names spell it.
    fn rust_triple_arch(self) -> &'static str {
        match self {
            Self::Aarch64 => "aarch64",
            Self::X86_64 => "x86_64",
        }
    }

    fn claude_platform(self) -> &'static str {
        match self {
            Self::Aarch64 => "linux-arm64",
            Self::X86_64 => "linux-x64",
        }
    }
}

/// What the artifact IS, which is what decides how it is unpacked.
enum Payload {
    /// the download is the executable itself, installed under this name.
    Binary(&'static str),
    /// members to lift out of a gzipped tar, by their path inside the archive.
    TarGz(&'static [&'static str]),
}

/// One vendor download — everything the operator is being asked to approve.
struct Download {
    version: &'static str,
    url: String,
    sha256: &'static str,
    payload: Payload,
}

impl Download {
    /// the file names this download installs into the executors directory.
    fn files(&self) -> Vec<&'static str> {
        match &self.payload {
            Payload::Binary(name) => vec![name],
            Payload::TarGz(members) => members.iter().copied().map(base_name).collect(),
        }
    }
}

/// the last path segment — a tar member's file name, a url's artifact name.
fn base_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

impl ProviderArg {
    /// The pinned Linux build of this provider's CLI for `arch`.
    fn download(self, arch: GuestArch) -> Download {
        match self {
            // `codex-package-<triple>` carries `bin/codex` AND the
            // `bin/codex-code-mode-host` companion codex requires, in ONE
            // artifact the release publishes a sha256 for. The bare
            // `codex-<triple>` asset ships neither.
            Self::Codex => Download {
                version: CODEX_RELEASE.trim_start_matches("rust-v"),
                url: format!(
                    "https://github.com/openai/codex/releases/download/{CODEX_RELEASE}/codex-package-{}-unknown-linux-musl.tar.gz",
                    arch.rust_triple_arch()
                ),
                sha256: match arch {
                    GuestArch::Aarch64 => {
                        "1ecac3f87823efb98153233b076ea3d6e34a7a8cebe43c5285dc5f79e1514639"
                    }
                    GuestArch::X86_64 => {
                        "00aba704f029f6dc0d948be407a756e0c97cc840132fd691353b2c6b0a505b17"
                    }
                },
                payload: Payload::TarGz(&["bin/codex", "bin/codex-code-mode-host"]),
            },
            // glibc, not musl: the guest base is a full Ubuntu userland.
            Self::Claude => Download {
                version: CLAUDE_VERSION,
                url: format!(
                    "https://downloads.claude.ai/claude-code-releases/{CLAUDE_VERSION}/{}/claude",
                    arch.claude_platform()
                ),
                sha256: match arch {
                    GuestArch::Aarch64 => {
                        "4ee7c484b11dece6521aa2173a19ea913428c1c78599186d62559d2d2aef4e32"
                    }
                    GuestArch::X86_64 => {
                        "47a01daebf794f6c86c13d1875ad6e5be0627029ad8600731161f24018ecde5b"
                    }
                },
                payload: Payload::Binary("claude"),
            },
        }
    }
}

/// Staging for a download in flight, BESIDE the executors directory rather than
/// inside it: the guest's copy is an image built from that directory's whole
/// contents, so a half-written 200 MB `.part` left there by an interrupted
/// install would be baked into it.
fn download_dir(executors: &Path) -> PathBuf {
    executors.with_extension("download")
}

pub(crate) fn run(args: InstallArgs) -> InstallResult {
    let arch = GuestArch::host()?;
    let dir = workspace_config::executor_dir()?;
    print_status(&dir, arch);

    // Named providers are the operator's explicit ask — already the approval a
    // checklist would collect, so it is not collected twice.
    if !args.providers.is_empty() {
        return install_all(&args.providers, &dir, arch);
    }

    let missing: Vec<ProviderArg> = ALL
        .into_iter()
        .filter(|p| !is_installed(*p, &dir, arch))
        .collect();
    if missing.is_empty() {
        println!("\nnothing to install. `ducktape agent install <name>` reinstalls one.");
        return Ok(());
    }
    let chosen = choose(&missing, arch)?;
    if chosen.is_empty() {
        return Ok(());
    }
    install_all(&chosen, &dir, arch)
}

/// Every file a provider's download installs is present and executable.
/// A partial install reports as missing rather than as present: codex without
/// its Code Mode companion is a codex that dies at startup inside the guest.
fn is_installed(provider: ProviderArg, dir: &Path, arch: GuestArch) -> bool {
    provider
        .download(arch)
        .files()
        .iter()
        .all(|f| is_executable(&dir.join(f)))
}

fn is_executable(path: &Path) -> bool {
    match std::fs::metadata(path) {
        Ok(meta) => meta.is_file() && meta.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

/// What is here, and — for what is not — exactly what installing it would
/// download. This IS the proposal the checklist below then asks approval for,
/// so it names the vendor url and the sha256 this build expects, in full.
fn print_status(dir: &Path, arch: GuestArch) {
    println!(
        "guest executors ({}, guest arch {})",
        dir.display(),
        arch.rust_triple_arch()
    );
    for provider in ALL {
        let name = provider.token();
        let download = provider.download(arch);
        if !is_installed(provider, dir, arch) {
            println!("  MISS    {name:<8} {}", download.version);
            println!("          {}", download.url);
            println!("          sha256 {}", download.sha256);
            continue;
        }
        // the hash of what is actually installed, so the image's contents stay
        // attributable to a download without unpacking the image.
        let installed = sha256_file(&dir.join(name)).unwrap_or_else(|_| "?".into());
        println!(
            "  ok      {name:<8} sha256:{}…",
            &installed[..16.min(installed.len())]
        );
    }
}

/// The checklist — the approval step for the downloads [`print_status`] just
/// proposed. Off a terminal there is nobody to approve, so it prints the
/// commands and installs nothing.
fn choose(missing: &[ProviderArg], arch: GuestArch) -> Result<Vec<ProviderArg>, String> {
    if !std::io::stdin().is_terminal() {
        println!("\nnot a terminal — install what you want with:");
        for provider in missing {
            println!("  ducktape agent install {}", provider.token());
        }
        return Ok(Vec::new());
    }

    println!();
    let items: Vec<String> = missing
        .iter()
        .map(|p| format!("{:<8} {}", p.token(), p.download(arch).version))
        .collect();
    let picked = dialoguer::MultiSelect::new()
        .with_prompt(
            "agent CLIs to install into this host's guest image (space toggles, enter confirms)",
        )
        .items(&items)
        .interact_opt()
        .map_err(|e| format!("checklist: {e}"))?;
    let Some(picked) = picked else {
        return Ok(Vec::new());
    };
    Ok(picked.into_iter().map(|i| missing[i]).collect())
}

fn install_all(providers: &[ProviderArg], dir: &Path, arch: GuestArch) -> InstallResult {
    std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    for provider in providers {
        install_one(*provider, dir, arch)?;
    }
    // Nothing else to do: the node derives the guest's copy from this directory
    // and rebuilds it whenever the directory has moved on, so the next run
    // picks this up on its own.
    Ok(())
}

fn install_one(provider: ProviderArg, dir: &Path, arch: GuestArch) -> InstallResult {
    let download = provider.download(arch);
    println!(
        "\n{} {} <- {}",
        provider.token(),
        download.version,
        download.url
    );

    let work = download_dir(dir);
    std::fs::create_dir_all(&work).map_err(|e| format!("create {}: {e}", work.display()))?;
    let artifact = work.join(base_name(&download.url));
    fetch(&download.url, &artifact, download.sha256)?;
    println!("  sha256 ok");

    match &download.payload {
        Payload::Binary(name) => install_file(&artifact, &dir.join(name))?,
        Payload::TarGz(members) => unpack_into(&artifact, members, dir)?,
    }
    // the artifact is the image's input, not its store: what was verified now
    // lives in the executors directory, and 200+ MB of tarball does not.
    let _ = std::fs::remove_file(&artifact);
    println!("  installed {} -> {}", provider.token(), dir.display());
    Ok(())
}

/// Download to `dest` unless it is already there, then verify — a cached file
/// is RE-verified rather than trusted, so a half-written or tampered cache
/// entry cannot survive into an image. A mismatch deletes the file and stops:
/// there is no "carry on without it" for an executable that runs beside a
/// credential.
fn fetch(url: &str, dest: &Path, want: &str) -> Result<(), String> {
    if !dest.exists() {
        download_to(url, dest)?;
    }
    let got = sha256_file(dest)?;
    if got != want {
        let _ = std::fs::remove_file(dest);
        return Err(format!(
            "checksum mismatch for {url}\n  expected {want}\n  got      {got}\n\
             refusing to install an unverified executable"
        ));
    }
    Ok(())
}

/// Stream to `<dest>.part` and rename on success: an interrupted download must
/// never be picked up as a cache hit on the next run.
fn download_to(url: &str, dest: &Path) -> Result<(), String> {
    let part = dest.with_extension("part");
    // no read timeout: these artifacts are hundreds of megabytes, and reqwest's
    // blocking default (30s) would abort every one of them.
    let client = reqwest::blocking::Client::builder()
        .timeout(None)
        .build()
        .map_err(|e| format!("http client: {e}"))?;
    let mut response = client
        .get(url)
        .send()
        .map_err(|e| format!("fetch {url}: {e}"))?
        .error_for_status()
        .map_err(|e| format!("fetch {url}: {e}"))?;
    let mut file =
        std::fs::File::create(&part).map_err(|e| format!("create {}: {e}", part.display()))?;
    let copied = std::io::copy(&mut response, &mut file);
    if let Err(e) = copied {
        let _ = std::fs::remove_file(&part);
        return Err(format!("download {url}: {e}"));
    }
    std::fs::rename(&part, dest).map_err(|e| format!("rename into {}: {e}", dest.display()))
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file =
        std::fs::File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|e| format!("read {}: {e}", path.display()))?;
    Ok(hex::encode(hasher.finalize()))
}

/// `tar` rather than a crate: it is on every macOS and Linux host, both
/// flavours extract named members the same way, and a tar reader is a parser
/// this verb does not need to own.
fn unpack_into(archive: &Path, members: &[&str], dir: &Path) -> Result<(), String> {
    let unpack = download_dir(dir).join("unpack");
    let _ = std::fs::remove_dir_all(&unpack);
    std::fs::create_dir_all(&unpack).map_err(|e| format!("create {}: {e}", unpack.display()))?;
    let status = std::process::Command::new("tar")
        .arg("xzf")
        .arg(archive)
        .arg("-C")
        .arg(&unpack)
        .args(members)
        .status()
        .map_err(|e| format!("tar: {e}"))?;
    if !status.success() {
        return Err(format!("tar xzf {} failed", archive.display()));
    }
    for member in members {
        install_file(&unpack.join(member), &dir.join(base_name(member)))?;
    }
    let _ = std::fs::remove_dir_all(&unpack);
    Ok(())
}

fn install_file(src: &Path, dest: &Path) -> Result<(), String> {
    std::fs::copy(src, dest)
        .map_err(|e| format!("install {} -> {}: {e}", src.display(), dest.display()))?;
    std::fs::set_permissions(dest, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("chmod {}: {e}", dest.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every pin is a full sha256 and every url is the vendor's, on both
    /// arches — the two things a bump can get wrong without failing to compile.
    #[test]
    fn every_pin_is_a_full_hash_from_a_vendor_url() {
        for arch in [GuestArch::Aarch64, GuestArch::X86_64] {
            for provider in ALL {
                let download = provider.download(arch);
                assert_eq!(download.sha256.len(), 64, "{:?} {arch:?}", provider.token());
                assert!(
                    download.sha256.chars().all(|c| c.is_ascii_hexdigit()),
                    "{} {arch:?} sha is not hex",
                    provider.token()
                );
                assert!(
                    download.url.starts_with("https://"),
                    "{} {arch:?} url is not https",
                    provider.token()
                );
                assert!(!download.files().is_empty());
            }
        }
    }

    /// codex is useless in the guest without its Code Mode companion, so a
    /// directory holding only `codex` must report as missing, not installed.
    #[test]
    fn a_partial_install_reports_as_missing() {
        let dir = std::env::temp_dir().join(format!("dt-exec-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let arch = GuestArch::Aarch64;

        install_file(&std::env::current_exe().unwrap(), &dir.join("codex")).unwrap();
        assert!(!is_installed(ProviderArg::Codex, &dir, arch));
        install_file(
            &std::env::current_exe().unwrap(),
            &dir.join("codex-code-mode-host"),
        )
        .unwrap();
        assert!(is_installed(ProviderArg::Codex, &dir, arch));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The verify gate: an unexpected hash deletes the file and refuses. This
    /// is the whole point of the pins, so it is checked without a network — a
    /// present `dest` skips the download.
    #[test]
    fn a_mismatched_download_is_deleted_and_refused() {
        let dir = std::env::temp_dir().join(format!("dt-exec-fetch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("artifact");
        std::fs::write(&dest, b"not what the pin says").unwrap();

        let err = fetch("https://example.invalid/x", &dest, &"0".repeat(64)).unwrap_err();
        assert!(
            err.contains("refusing to install an unverified executable"),
            "{err}"
        );
        assert!(!dest.exists(), "a mismatched artifact must not survive");

        // and the matching case installs: sha256("") is the empty-file hash.
        std::fs::write(&dest, b"").unwrap();
        fetch(
            "https://example.invalid/x",
            &dest,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        )
        .unwrap();
        assert!(dest.exists());

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
