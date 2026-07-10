//! the `mount` verb seam. the FUSE filesystem itself lives in `crate::fuse`
//! behind the `fuse` cargo feature; this thin shim keeps the verb name and its
//! help stable in EVERY build. without the feature the deps (`fuser`/`libc`) are
//! never compiled and `mount` is a clear rebuild error — that is how the default
//! workspace build stays free of libfuse.

use crate::args::CliError;

/// with the `fuse` feature: parse the mount args and run the mount (blocks until
/// SIGINT/SIGTERM). see [`crate::fuse::run`].
#[cfg(feature = "fuse")]
pub fn mount(args: &[String]) -> Result<(), CliError> {
    crate::fuse::run(args)
}

/// without the `fuse` feature: the verb is recognized but this binary can't mount
/// — name the exact rebuild rather than pretend to work.
#[cfg(not(feature = "fuse"))]
pub fn mount(_args: &[String]) -> Result<(), CliError> {
    Err(CliError::failed(
        "mount needs a build with the `fuse` feature (unprivileged, via \
         fusermount3 — no libfuse); rebuild: cargo build -p fs-bin --features fuse",
    ))
}
