//! probed host capacity for the capability announce — total machine
//! resources, deliberately NOT free memory: capacity is a standing promise,
//! the ledger handles moment-to-moment load.

use std::collections::BTreeMap;

/// this host's total capacity, keyed like the scheduler's demand dimensions
/// (`cores`, `mem_gb`). only what a backend can enforce is announced; an
/// unprobeable dimension is simply absent (never a fabricated number).
pub(crate) fn probe() -> BTreeMap<String, u64> {
    let mut r = BTreeMap::new();
    if let Ok(n) = std::thread::available_parallelism() {
        r.insert("cores".into(), n.get() as u64);
    }
    if let Some(gb) = total_mem_gb() {
        r.insert("mem_gb".into(), gb);
    }
    r
}

#[cfg(target_os = "linux")]
fn total_mem_gb() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let kb: u64 = text
        .lines()
        .find(|l| l.starts_with("MemTotal:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;
    // floor at 1: a machine reporting <1 GiB still announces one whole unit
    // (the smallest promise the mem_gb dimension can express).
    Some((kb / (1024 * 1024)).max(1))
}

#[cfg(target_os = "macos")]
fn total_mem_gb() -> Option<u64> {
    // sysctlbyname("hw.memsize") reports total RAM in bytes.
    let mut bytes: u64 = 0;
    let mut len = std::mem::size_of::<u64>();
    // SAFETY: `hw.memsize` is a valid NUL-terminated name; `oldp` points at a
    // u64 sized by `oldlenp`; `newp` is null (a pure read).
    let rc = unsafe {
        libc::sysctlbyname(
            c"hw.memsize".as_ptr(),
            &mut bytes as *mut u64 as *mut libc::c_void,
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc != 0 {
        return None;
    }
    Some((bytes / (1024 * 1024 * 1024)).max(1))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn total_mem_gb() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_reports_nonzero_cores_and_mem_on_this_host() {
        let r = probe();
        assert!(r.get("cores").copied().unwrap_or(0) >= 1);
        assert!(r.get("mem_gb").copied().unwrap_or(0) >= 1);
    }
}
