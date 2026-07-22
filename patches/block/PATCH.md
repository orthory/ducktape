# Local block 0.1.6 compatibility patch

This directory mirrors SSheldon/rust-block 0.1.6 at upstream commit
`642ea4a4a5853a21b55b05c34832a5f1bb1af61c`.

The only runtime-source change replaces the uninhabited `enum Class {}` used
as an extern static type with an inhabited opaque `#[repr(C)] struct Class;`.
Rust reports the original declaration as future-incompatible (rust-lang/rust
issue 74840). The public API and Objective-C Blocks ABI are unchanged. The
test helper also uses the maintained `cc` crate and an explicit C ABI so its
own verification stays warning-free on current Rust.
