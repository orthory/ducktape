//! Process resource limits required by the production node.

#[cfg(unix)]
const TARGET_OPEN_FILES: libc::rlim_t = 65_536;

/// Raise the inherited open-file soft limit when the launcher provides less
/// than the node's module stores require. Failure remains non-fatal so
/// restricted environments still get the underlying open-file error.
pub(crate) fn raise_open_file_limit() {
    #[cfg(unix)]
    if let Err(err) = raise_open_file_limit_unix() {
        eprintln!("[node] warning: could not raise open-file limit: {err}");
    }
}

#[cfg(unix)]
fn raise_open_file_limit_unix() -> std::io::Result<()> {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };

    // SAFETY: `limit` is a valid writable rlimit and RLIMIT_NOFILE selects
    // exactly that structure in both calls.
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } != 0 {
        return Err(std::io::Error::last_os_error());
    }

    let desired = desired_soft_limit(limit.rlim_cur, limit.rlim_max);
    if desired == limit.rlim_cur {
        return Ok(());
    }
    limit.rlim_cur = desired;

    // SAFETY: the hard limit is unchanged and `desired` never exceeds it.
    if unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &limit) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(unix)]
fn desired_soft_limit(current: libc::rlim_t, hard: libc::rlim_t) -> libc::rlim_t {
    if current >= TARGET_OPEN_FILES || current >= hard {
        current
    } else {
        TARGET_OPEN_FILES.min(hard)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn raises_to_target_when_allowed() {
        assert_eq!(desired_soft_limit(256, libc::RLIM_INFINITY), 65_536);
    }

    #[test]
    fn respects_the_hard_limit() {
        assert_eq!(desired_soft_limit(256, 4_096), 4_096);
    }

    #[test]
    fn never_lowers_an_existing_limit() {
        assert_eq!(
            desired_soft_limit(1_048_576, libc::RLIM_INFINITY),
            1_048_576
        );
        assert_eq!(desired_soft_limit(4_096, 4_096), 4_096);
    }
}
