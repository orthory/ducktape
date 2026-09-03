//! the open-file soft limit every ducktape process needs. the module stores
//! (one value log + one index per module, plus the block spine and every live
//! socket) blow past a 256-fd shell default long before anything else notices,
//! and the failure surfaces as a bare `EMFILE` from whatever opened last. the
//! desktop app needs it exactly as much as the node does — macOS launches GUIs
//! with the 256 default — so the raise lives here, next to `log_file`, and both
//! binaries call it at startup.

#[cfg(unix)]
const TARGET_OPEN_FILES: libc::rlim_t = 65_536;

/// Raise the inherited open-file soft limit toward [`TARGET_OPEN_FILES`],
/// returning the soft limit in force afterwards. An `Err` carries the OS error
/// from the `getrlimit`/`setrlimit` that refused, and is non-fatal: the caller
/// keeps running and the underlying open-file error still surfaces on its own.
/// The caller reports the outcome, because a daemon that has no subscriber yet
/// and a GUI that does need different sinks.
pub fn raise_open_file_limit() -> std::io::Result<u64> {
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
fn raise_open_file_limit_unix() -> std::io::Result<u64> {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };

    // SAFETY: `limit` is a valid writable rlimit and RLIMIT_NOFILE selects
    // exactly that structure in both calls.
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } != 0 {
        return Err(std::io::Error::last_os_error());
    }

    let desired = desired_soft_limit(limit.rlim_cur, limit.rlim_max, open_max_clamp());
    if desired == limit.rlim_cur {
        return Ok(limit.rlim_cur);
    }
    limit.rlim_cur = desired;

    // SAFETY: the hard limit is unchanged and `desired` never exceeds it.
    if unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &limit) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(desired)
}

/// macOS refuses `setrlimit(RLIMIT_NOFILE)` with EINVAL above `OPEN_MAX`, so a
/// 65_536 request fails outright and leaves the process on the 256 default —
/// the exact failure this module exists to prevent. Every other unix takes the
/// hard limit at face value.
#[cfg(all(unix, target_os = "macos"))]
fn open_max_clamp() -> Option<libc::rlim_t> {
    // SAFETY: sysconf takes an int name and has no memory preconditions.
    let open_max = unsafe { libc::sysconf(libc::_SC_OPEN_MAX) };
    (open_max > 0).then_some(open_max as libc::rlim_t)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_max_clamp() -> Option<libc::rlim_t> {
    None
}

#[cfg(unix)]
fn desired_soft_limit(
    current: libc::rlim_t,
    hard: libc::rlim_t,
    clamp: Option<libc::rlim_t>,
) -> libc::rlim_t {
    let ceiling = clamp.map_or(hard, |clamp| hard.min(clamp));
    let already_high_enough = current >= TARGET_OPEN_FILES || current >= ceiling;
    if already_high_enough {
        current
    } else {
        TARGET_OPEN_FILES.min(ceiling)
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

    /// the macOS shape: an infinite hard limit but an `OPEN_MAX` of 10_240, so
    /// the request must settle at the clamp instead of failing with EINVAL.
    #[test]
    fn clamps_to_open_max() {
        assert_eq!(
            desired_soft_limit(256, libc::RLIM_INFINITY, Some(10_240)),
            10_240
        );
    }

    #[test]
    fn a_clamp_never_lowers_an_existing_limit() {
        assert_eq!(
            desired_soft_limit(20_000, libc::RLIM_INFINITY, Some(10_240)),
            20_000
        );
    }

    #[test]
    fn the_lower_of_clamp_and_hard_limit_wins() {
        assert_eq!(desired_soft_limit(256, 4_096, Some(10_240)), 4_096);
        assert_eq!(desired_soft_limit(256, 100_000, Some(10_240)), 10_240);
    }
}
