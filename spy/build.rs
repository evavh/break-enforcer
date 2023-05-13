// use std::{env, error::Error, fs::File, io::Write, path::PathBuf};
//
// use cc::Build;

fn main() {
    // // build directory for this crate
    // let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    //
    // println!("cargo:rustc-link-search={}", out_dir.display());

    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
}
