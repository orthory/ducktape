fn main() {
    // CEF is ABI-pinned and staged beside the executable. Prefer that exact
    // runtime even when the launcher has an ambient LD_LIBRARY_PATH.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-arg=-Wl,--disable-new-dtags");
        println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN");
    }
}
