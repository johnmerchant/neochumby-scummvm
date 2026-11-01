//! Builds ScummVM (third_party/scummvm, unchanged) and our backend for the
//! Chumby, by tools/build-scummvm, and links them. Only for the Chumby: on any
//! other target just the tests of the parts that need no device are built.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=backend-gpl");
    println!("cargo:rerun-if-changed=tools/build-scummvm");
    if !std::env::var("TARGET").unwrap().starts_with("armv5te") {
        return;
    }
    let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    if !root.join("third_party/scummvm/configure").exists() {
        panic!("third_party/scummvm is empty: run `git submodule update --init`");
    }
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("scummvm");
    let status = Command::new(root.join("tools/build-scummvm")).arg(&out).status().unwrap();
    assert!(status.success(), "tools/build-scummvm failed");

    // Its objects and archives refer to each other, and to the chum_* functions
    // of this crate.
    println!("cargo:rustc-link-arg=-Wl,--start-group");
    println!("cargo:rustc-link-arg={}", out.join("chumby.o").display());
    let list = std::fs::read_to_string(out.join("chumby-objs.txt")).unwrap();
    for file in list.split_whitespace() {
        println!("cargo:rustc-link-arg={}", out.join(file).display());
    }
    println!("cargo:rustc-link-arg=-Wl,--end-group");
    println!("cargo:rustc-link-arg=-lc++");
}
