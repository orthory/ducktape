//! Terminal helpers for the interactive CLI verbs: raw mode for the full-screen
//! wraps, and the yes/no confirmation the consent-bearing verbs ask first.
//!
//! Raw mode: both the pty attach (`agent pty`) and the vendor-login wrap
//! (`cred add`) drive a full-screen TUI over a pty; that only works when THIS
//! terminal is in raw mode, so keystrokes (arrows, Enter as `\r`, a pasted
//! code) reach the child verbatim instead of being line-buffered, echoed, and
//! `\n`-translated by the local tty.

/// True when this process's stdin is an interactive terminal.
pub(crate) fn stdin_is_tty() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stdin())
}

/// Ask a yes/no question, defaulting to yes, and report whether to proceed.
///
/// This is the consent seam: a verb that is about to grant something asks
/// first, so an interactive user sees what they are authorizing. Automation
/// has nobody to ask — a pipe, a systemd unit and CI have no stdin worth
/// reading — so a NON-terminal proceeds instead of hanging forever, and
/// `--yes` states that intent explicitly.
///
/// `is_tty` is a parameter rather than a probe so both non-interactive paths
/// are unit-testable without a controlling terminal (the discipline
/// `userkey_cli::with_prompt` already uses). The prompt goes to stderr so a
/// verb's stdout stays a clean machine-readable value.
pub(crate) fn confirm(question: &str, is_tty: bool, assume_yes: bool) -> Result<bool, String> {
    if assume_yes || !is_tty {
        return Ok(true);
    }
    eprint!("{question} [Y/n] ");
    let _ = std::io::Write::flush(&mut std::io::stderr());
    let mut answer = String::new();
    std::io::BufRead::read_line(&mut std::io::stdin().lock(), &mut answer)
        .map_err(|error| format!("read confirmation: {error}"))?;
    // an empty line takes the capitalized default (yes); only an explicit no
    // declines, so a typo never silently grants anything.
    Ok(!matches!(answer.trim().to_ascii_lowercase().as_str(), "n" | "no"))
}

/// RAII: restore the saved `termios` on drop (covers normal return AND a panic
/// unwinding through the caller), so the tty is never left raw.
pub(crate) struct RawGuard {
    fd: i32,
    saved: libc::termios,
}

impl RawGuard {
    /// Put stdin into raw mode, returning the restore guard. `None` when stdin
    /// is not a tty (piped) — nothing to restore, and the stream still runs.
    pub(crate) fn enter() -> Option<Self> {
        let fd = libc::STDIN_FILENO;
        // SAFETY: fd is a valid descriptor; termios is fully written by tcgetattr
        // before any read, and cfmakeraw/tcsetattr take valid pointers.
        unsafe {
            if libc::isatty(fd) != 1 {
                return None;
            }
            let mut saved: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(fd, &mut saved) != 0 {
                return None;
            }
            let mut raw = saved;
            libc::cfmakeraw(&mut raw);
            if libc::tcsetattr(fd, libc::TCSANOW, &raw) != 0 {
                return None;
            }
            Some(Self { fd, saved })
        }
    }
}

impl Drop for RawGuard {
    fn drop(&mut self) {
        // SAFETY: fd stayed valid for the guard's life; `saved` is the termios
        // tcgetattr filled in enter().
        unsafe {
            libc::tcsetattr(self.fd, libc::TCSANOW, &self.saved);
        }
    }
}
