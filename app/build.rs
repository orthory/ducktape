// Ice sources compile from here, not from the `include_app!` macro's own reader.
//
// Two things fall out of that, and both were real costs before it:
//   * the generated Rust lands under OUT_DIR instead of a shared /tmp path, so
//     it is out of clippy's way and `cargo clean` takes it with everything else.
//   * `compile_dir` emits `cargo::rerun-if-changed` for the directory and for
//     every source and imported fragment it reads, so editing a `.ice` file
//     invalidates the build. It used not to, and `touch app/src/main.rs` before
//     every build was the house workaround.
fn main() {
    ui_lang_build::compile_dir("src/ui").expect("compile Ice sources");
}
