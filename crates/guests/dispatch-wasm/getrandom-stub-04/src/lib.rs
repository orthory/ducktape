//! deterministic `getrandom` 0.4 stand-in for the dispatch GUEST build —
//! see Cargo.toml and ../getrandom-stub (the 0.2 twin) for the full why.
//! every entry point that would produce entropy fails DETERMINISTICALLY:
//! ambient randomness inside a consensus guest is a fork, not a feature.

use core::fmt;
use core::mem::MaybeUninit;

/// mirrors `getrandom::Error` 0.4: an opaque code. the stub only ever
/// produces [`Error::UNSUPPORTED`].
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Error(u32);

impl Error {
    /// the one error this stub ever produces: the guest has no entropy
    /// source, by design.
    pub const UNSUPPORTED: Error = Error(0);

    pub fn raw_os_error(self) -> Option<i32> {
        None
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Error").field("code", &self.0).finish()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "deterministic guest: no entropy source")
    }
}

impl std::error::Error for Error {}

/// the real crate's primary entry point: REFUSED, deterministically.
pub fn fill(_dest: &mut [u8]) -> Result<(), Error> {
    Err(Error::UNSUPPORTED)
}

/// uninit-buffer variant: refused identically.
pub fn fill_uninit(_dest: &mut [MaybeUninit<u8>]) -> Result<&mut [u8], Error> {
    Err(Error::UNSUPPORTED)
}

pub fn u32() -> Result<u32, Error> {
    Err(Error::UNSUPPORTED)
}

pub fn u64() -> Result<u64, Error> {
    Err(Error::UNSUPPORTED)
}

/// the real 0.4 crate re-exports its `rand_core` for dependents to name
/// trait items through (`getrandom::rand_core::UnwrapErr`).
pub use rand_core;

/// mirrors `getrandom::SysRng` 0.4: the system RNG handle. in this
/// deterministic guest every draw is REFUSED — ambient entropy inside a
/// consensus module is a fork, not a feature.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct SysRng;

impl rand_core::TryRng for SysRng {
    type Error = Error;

    fn try_next_u32(&mut self) -> Result<u32, Error> {
        Err(Error::UNSUPPORTED)
    }

    fn try_next_u64(&mut self) -> Result<u64, Error> {
        Err(Error::UNSUPPORTED)
    }

    fn try_fill_bytes(&mut self, _dst: &mut [u8]) -> Result<(), Error> {
        Err(Error::UNSUPPORTED)
    }
}

/// marker parity with the real crate: `SysRng` advertises the crypto-rng
/// contract (`UnwrapErr<SysRng>: CryptoRng` in dependents). every draw still
/// fails deterministically — the marker promises quality IF entropy flowed,
/// and none ever does here.
impl rand_core::TryCryptoRng for SysRng {}
