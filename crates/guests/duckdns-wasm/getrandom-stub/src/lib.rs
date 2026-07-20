//! deterministic `getrandom` 0.2 stand-in for the duckdns GUEST build —
//! see Cargo.toml for the full why. the API mirrors the subset the dependency
//! tree actually links against (`rand_core`'s `os`/`error` modules); every
//! entry point that would produce entropy instead fails DETERMINISTICALLY,
//! because ambient randomness inside a consensus guest is a fork, not a
//! feature. no consensus code path ever calls this at runtime.

use core::fmt;
use core::mem::MaybeUninit;
use core::num::NonZeroU32;

/// mirrors `getrandom::Error`: a NonZeroU32 code, with the same reserved
/// ranges the real crate (and `rand_core`) document.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Error(NonZeroU32);

impl Error {
    /// codes below this point represent OS errors (identical to the real crate).
    pub const INTERNAL_START: u32 = 1 << 31;
    /// codes at or above this point may be used by users (identical to the
    /// real crate).
    pub const CUSTOM_START: u32 = (1 << 31) + (1 << 30);
    /// the one error this stub ever produces: the guest has no entropy source,
    /// by design.
    pub const UNSUPPORTED: Error =
        Error(match NonZeroU32::new(Error::CUSTOM_START) {
            Some(code) => code,
            None => unreachable!(),
        });

    pub fn code(self) -> NonZeroU32 {
        self.0
    }

    pub fn raw_os_error(self) -> Option<i32> {
        None
    }
}

impl From<NonZeroU32> for Error {
    fn from(code: NonZeroU32) -> Self {
        Error(code)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Error").field("code", &self.0).finish()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "deterministic guest: no entropy source (code {})", self.0)
    }
}

impl std::error::Error for Error {}

/// the real crate's primary entry point: REFUSED, deterministically — a guest
/// module may never observe entropy.
pub fn getrandom(_dest: &mut [u8]) -> Result<(), Error> {
    Err(Error::UNSUPPORTED)
}

/// the real crate's uninit-buffer variant: refused identically.
pub fn getrandom_uninit(_dest: &mut [MaybeUninit<u8>]) -> Result<&mut [u8], Error> {
    Err(Error::UNSUPPORTED)
}
