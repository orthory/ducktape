# Local blst 0.3.16 wasm32 no-C patch

This directory mirrors supranational/blst 0.3.16 as published on crates.io.

The only change is in `build.rs`: on any `wasm32*` target it emits the
`no-threads` cfg the upstream script would emit and returns before compiling
the bundled C/assembly. Guest workspaces (`crates/examples/*-wasm`) apply it
via `[patch.crates-io]`; the host workspace keeps the real registry crate.

Why this is sound: blst reaches every guest unconditionally through
commonware-cryptography, but no module calls bls12381, so wasm-ld dead-strips
every blst symbol from the shipped modules (verified against a control module
that does use blst). The C objects were only ever compiled to be discarded —
and compiling C for wasm32 requires a wasm-capable clang, which stock linux
boxes often lack and Apple's Xcode clang cannot provide at all (no wasm
backend). With this patch, guest builds are pure Rust on every platform.

Fail-safe: the Rust bindings still declare the extern C symbols, so if a
guest ever genuinely calls into blst, wasm-ld fails loudly on the missing
definitions. That is the moment to decide between a host-function import on
the module world and compiling real blst to wasm.

Native (non-wasm) targets are untouched: the full C/assembly tree is kept so
a host-target `cargo check` of a guest workspace still builds real blst.
