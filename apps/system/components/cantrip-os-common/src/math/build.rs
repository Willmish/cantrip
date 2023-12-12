use std::env;
use std::fmt::Write;

fn main() {
    // Find the toolchain version for conditionally setting features.
    let rust_toolchain = env::var("RUSTUP_TOOLCHAIN").unwrap();
    println!("RUST_TOOLCHAIN {rust_toolchain}");
    let bits: Vec<&str> = rust_toolchain.split('-').collect();
    let mut version = String::new();
    let _ = write!(&mut version, "{}-{}-{}", bits[1], bits[2], bits[3]);
    println!("RUST_TOOLCHAIN {version}");

    // Some point between 1.56 and 1.74.1 fmax & co were added automatically
    // so we need to suppress our compat implementations.
    // TODO: need a better bound on when fp symbols were added
    if version < "2023-12-04".to_string() {
        println!("cargo:rustc-cfg=feature=\"fp_compat\"");
    }
}
