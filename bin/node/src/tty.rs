//! Terminal raw-mode for the interactive CLI verbs. Both the pty attach
//! (`agent pty`) and the vendor-login wrap (`cred add`) drive a full-screen TUI
//! over a pty; that only works when THIS terminal is in raw mode, so keystrokes
//! (arrows, Enter as `\r`, a pasted code) reach the child verbatim instead of
//! being line-buffered, echoed, and `\n`-translated by the local tty.

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
