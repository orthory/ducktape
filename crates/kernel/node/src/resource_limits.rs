//! the open-file soft limit every ducktape process needs. the module stores
//! (one value log + one index per module, plus the block spine and every live
//! socket) blow past a 256-fd shell default long before anything else notices,
//! and the failure surfaces as a bare `EMFILE` from whatever opened last. the
//! desktop app needs it exactly as much as the node does — macOS launches GUIs
//! with the 256 default — so the raise lives here, next to `log_file`, and both
//! binaries call it at startup.
//!
//! The raise runs before a subscriber exists (it must precede anything that
//! opens a descriptor), so it RECORDS what it did and the binary reports the
//! record once its sink is up — see [`startup_outcome`]. A raise nothing can
//! observe is how a process spent a whole run on 256 descriptors while its log
//! said nothing at all.

use std::sync::OnceLock;

#[cfg(unix)]
const TARGET_OPEN_FILES: libc::rlim_t = 65_536;

/// the soft limit below which a node is running on borrowed time: it holds
/// ~300 descriptors at rest (one value log + one index per module, the block
/// spine, `daemon.log`, every peer socket) and a catch-up stage or a burst of
/// gateway connections multiplies that. Anything under this is reported, loudly
/// — the node still boots, and the EMFILE that follows is minutes away rather
/// than obviously connected to it.
pub const MINIMUM_OPEN_FILES: u64 = 4_096;

/// The open-file limits in force after the startup raise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenFileLimit {
    /// the soft limit — the one that produces `EMFILE`.
    pub soft: u64,
    /// the ceiling this process may raise the soft limit to, unasked.
    pub hard: u64,
    /// whether this process moved the soft limit itself. `false` means the
    /// inherited limit was already at or above what we would have asked for.
    pub raised: bool,
}

impl OpenFileLimit {
    /// whether the limit in force covers what a node needs.
    pub fn is_sufficient(&self) -> bool {
        self.soft >= MINIMUM_OPEN_FILES
    }
}

/// What [`raise_open_file_limit`] concluded, kept for the report that can only
/// happen later. `None` before the raise has run; the error is flattened to a
/// string because `std::io::Error` is not `Clone` and this is read many times.
static STARTUP_OUTCOME: OnceLock<Result<OpenFileLimit, String>> = OnceLock::new();

/// The recorded outcome of this process's startup raise, for a binary whose
/// subscriber did not exist when it ran. `None` only if nothing called
/// [`raise_open_file_limit`].
pub fn startup_outcome() -> Option<Result<OpenFileLimit, String>> {
    STARTUP_OUTCOME.get().cloned()
}

/// Raise the inherited open-file soft limit toward [`TARGET_OPEN_FILES`],
/// returning the limits in force afterwards. An `Err` carries the OS error from
/// the `getrlimit`/`setrlimit` that refused, and is non-fatal: the caller keeps
/// running and the underlying open-file error still surfaces on its own. The
/// outcome is also recorded in [`startup_outcome`], because the node raises
/// before it has a sink to report through.
pub fn raise_open_file_limit() -> std::io::Result<OpenFileLimit> {
    let outcome = raise_now();
    let record = match &outcome {
        Ok(limit) => Ok(*limit),
        Err(error) => Err(error.to_string()),
    };
    let _ = STARTUP_OUTCOME.set(record);
    outcome
}

fn raise_now() -> std::io::Result<OpenFileLimit> {
    #[cfg(unix)]
    {
        raise_open_file_limit_unix()
    }
    #[cfg(not(unix))]
    {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "no rlimit on this platform",
        ))
    }
}

#[cfg(unix)]
fn raise_open_file_limit_unix() -> std::io::Result<OpenFileLimit> {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };

    // SAFETY: `limit` is a valid writable rlimit and RLIMIT_NOFILE selects
    // exactly that structure in both calls.
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let inherited = limit.rlim_cur;

    let desired = desired_soft_limit(inherited, limit.rlim_max, per_process_ceiling());
    if desired == inherited {
        return Ok(OpenFileLimit {
            soft: inherited,
            hard: limit.rlim_max,
            raised: false,
        });
    }
    limit.rlim_cur = desired;

    // SAFETY: the hard limit is unchanged and `desired` never exceeds it.
    if unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &limit) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(OpenFileLimit {
        soft: desired,
        hard: limit.rlim_max,
        raised: true,
    })
}

/// macOS caps how many descriptors ONE process may hold at `kern.maxfilesperproc`,
/// independently of `RLIMIT_NOFILE` — `setrlimit` happily accepts a soft limit
/// above it and every `open` past the sysctl still fails. Clamping here is what
/// keeps the number we log equal to the number the kernel will honor.
///
/// It is NOT `sysconf(_SC_OPEN_MAX)`: on Darwin that call returns the CURRENT
/// SOFT LIMIT, so clamping to it made the target equal the 256 a launchd-started
/// process inherits, and the raise then "succeeded" without raising anything.
#[cfg(all(unix, target_os = "macos"))]
fn per_process_ceiling() -> Option<libc::rlim_t> {
    let mut per_proc: libc::c_int = 0;
    let mut size = std::mem::size_of::<libc::c_int>();
    let name = c"kern.maxfilesperproc";
    // SAFETY: the name is a NUL-terminated C string, `per_proc`/`size` are a
    // valid writable int and its length, and the new-value pointer is null.
    let read = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            std::ptr::from_mut(&mut per_proc).cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    (read == 0 && per_proc > 0).then_some(per_proc as libc::rlim_t)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn per_process_ceiling() -> Option<libc::rlim_t> {
    None
}

#[cfg(unix)]
fn desired_soft_limit(
    current: libc::rlim_t,
    hard: libc::rlim_t,
    ceiling: Option<libc::rlim_t>,
) -> libc::rlim_t {
    let reachable = ceiling.map_or(hard, |ceiling| hard.min(ceiling));
    let already_high_enough = current >= TARGET_OPEN_FILES || current >= reachable;
    if already_high_enough {
        current
    } else {
        TARGET_OPEN_FILES.min(reachable)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn raises_to_target_when_allowed() {
        assert_eq!(desired_soft_limit(256, libc::RLIM_INFINITY, None), 65_536);
    }

    #[test]
    fn respects_the_hard_limit() {
        assert_eq!(desired_soft_limit(256, 4_096, None), 4_096);
    }

    #[test]
    fn never_lowers_an_existing_limit() {
        assert_eq!(
            desired_soft_limit(1_048_576, libc::RLIM_INFINITY, None),
            1_048_576
        );
        assert_eq!(desired_soft_limit(4_096, 4_096, None), 4_096);
    }

    /// THE launchd shape, and the regression this module exists for:
    /// `launchctl limit maxfiles` is `256 unlimited`, so a node started from a
    /// GUI-launched shell inherits soft 256 with an infinite hard limit. The
    /// target must survive that untouched — a ceiling read from the CURRENT
    /// SOFT LIMIT (what `sysconf(_SC_OPEN_MAX)` returns on Darwin) collapsed
    /// this to 256 and reported success.
    #[test]
    fn the_launchd_default_raises_to_the_target() {
        assert_eq!(
            desired_soft_limit(256, libc::RLIM_INFINITY, Some(245_760)),
            65_536
        );
    }

    /// a real per-process ceiling below the target still binds.
    #[test]
    fn clamps_to_the_per_process_ceiling() {
        assert_eq!(
            desired_soft_limit(256, libc::RLIM_INFINITY, Some(10_240)),
            10_240
        );
    }

    #[test]
    fn a_ceiling_never_lowers_an_existing_limit() {
        assert_eq!(
            desired_soft_limit(20_000, libc::RLIM_INFINITY, Some(10_240)),
            20_000
        );
    }

    #[test]
    fn the_lower_of_ceiling_and_hard_limit_wins() {
        assert_eq!(desired_soft_limit(256, 4_096, Some(10_240)), 4_096);
        assert_eq!(desired_soft_limit(256, 100_000, Some(10_240)), 10_240);
    }

    /// the marker that tells a re-exec of this test binary it is the child arm.
    const CHILD_ARM: &str = "DUCKTAPE_RLIMIT_CHILD";

    /// THE field failure, end to end: a process holding the launchd shape
    /// (`launchctl limit maxfiles` is `256 unlimited`, and every macOS terminal
    /// inherits it) must come back with a soft limit a node can live on.
    ///
    /// It runs in a CHILD, because the raise is a process-wide mutation and
    /// every other test in this binary shares the process — so the test re-execs
    /// itself with a marker in the environment and asserts on the child's
    /// verdict. No sleeps and no clock: the wait is the child's own exit.
    #[test]
    fn a_launchd_shaped_process_reaches_the_target() {
        if std::env::var_os(CHILD_ARM).is_some() {
            return launchd_shaped_child();
        }
        let child = std::process::Command::new(
            std::env::current_exe().expect("the test binary knows its own path"),
        )
        .args([
            "--exact",
            "--nocapture",
            "resource_limits::tests::a_launchd_shaped_process_reaches_the_target",
        ])
        .env(CHILD_ARM, "1")
        .output()
        .expect("the test binary re-execs");
        assert!(
            child.status.success(),
            "the child refused the launchd shape:\n{}\n{}",
            String::from_utf8_lossy(&child.stdout),
            String::from_utf8_lossy(&child.stderr)
        );
    }

    /// the child arm: adopt the launchd shape, then run the SHIPPED raise.
    fn launchd_shaped_child() {
        const INHERITED: libc::rlim_t = 256;
        let mut limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        // SAFETY: `limit` is a valid writable rlimit for RLIMIT_NOFILE.
        assert_eq!(
            unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) },
            0
        );
        let hard = limit.rlim_max;
        // a build host whose HARD limit is already the default has nothing to
        // raise to, and that is a property of the host, not of this code.
        if hard <= INHERITED {
            println!("skipped: this host's hard limit is {hard}");
            return;
        }
        limit.rlim_cur = INHERITED;
        // SAFETY: lowering our own soft limit; the hard limit is unchanged.
        assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &limit) }, 0);

        let raised = raise_open_file_limit().expect("the raise must not refuse the launchd shape");
        assert!(
            raised.raised && raised.soft > INHERITED,
            "the inherited default was left in force: {raised:?}"
        );
        assert!(
            raised.is_sufficient(),
            "a node cannot run on {} descriptors",
            raised.soft
        );
        // and the process really holds what the record claims.
        let mut after = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        // SAFETY: as above.
        assert_eq!(
            unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut after) },
            0
        );
        assert_eq!(
            after.rlim_cur, raised.soft,
            "the reported soft limit is not the one in force"
        );
        assert_eq!(
            startup_outcome(),
            Some(Ok(raised)),
            "the outcome must be recorded for the boot report"
        );
        println!("raised to {} (hard {})", raised.soft, raised.hard);
    }

    /// the sufficiency verdict the boot report warns on.
    #[test]
    fn the_inherited_default_is_not_sufficient() {
        let inherited = OpenFileLimit {
            soft: 256,
            hard: u64::MAX,
            raised: false,
        };
        assert!(!inherited.is_sufficient());
        assert!(
            OpenFileLimit {
                soft: 65_536,
                ..inherited
            }
            .is_sufficient()
        );
    }

    /// WHY the ceiling is a sysctl and not `sysconf`: on Darwin
    /// `sysconf(_SC_OPEN_MAX)` answers with this process's CURRENT SOFT LIMIT.
    /// Clamping the target to it makes the target equal the limit we are trying
    /// to leave, so the raise turns into a no-op that reports success — the
    /// exact way a node spent a run on 256 descriptors. This pins the platform
    /// fact so the shortcut cannot come back looking reasonable.
    #[cfg(target_os = "macos")]
    #[test]
    fn sysconf_open_max_is_the_soft_limit_not_a_ceiling() {
        let mut limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        // SAFETY: `limit` is a valid writable rlimit; sysconf takes an int name.
        let (read, open_max) = unsafe {
            (
                libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit),
                libc::sysconf(libc::_SC_OPEN_MAX),
            )
        };
        assert_eq!(read, 0, "RLIMIT_NOFILE is always readable");
        assert_eq!(
            open_max as libc::rlim_t, limit.rlim_cur,
            "_SC_OPEN_MAX tracks the soft limit on Darwin; it is not a ceiling"
        );
        let ceiling = per_process_ceiling().expect("kern.maxfilesperproc is always readable");
        assert!(
            ceiling >= MINIMUM_OPEN_FILES,
            "a per-process ceiling of {ceiling} cannot host a node"
        );
    }
}
