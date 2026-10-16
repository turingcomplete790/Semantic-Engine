//! Embeds build facts the `status` operation reports: the Oxigraph version and the version of
//! the RocksDB that Oxigraph bundles (constitution v2.0.0, research R6), plus target and profile.
//!
//! Versions come from the workspace `Cargo.lock`; the RocksDB version is read from
//! `rocksdb/include/rocksdb/version.h` inside the `oxrocksdb-sys` source in the Cargo registry
//! (or a `vendor/` directory). No `cargo metadata` call: running cargo inside a build script can
//! deadlock on Cargo's package lock.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn locked_version(lock: &str, name: &str) -> Option<String> {
    let mut lines = lock.lines();
    while let Some(l) = lines.next() {
        if l.trim() == format!("name = \"{name}\"") {
            let v = lines.next()?.trim();
            return v
                .strip_prefix("version = \"")?
                .strip_suffix('"')
                .map(str::to_string);
        }
    }
    None
}

fn cargo_home() -> Option<PathBuf> {
    if let Ok(h) = env::var("CARGO_HOME") {
        return Some(PathBuf::from(h));
    }
    let home = env::var("HOME").or_else(|_| env::var("USERPROFILE")).ok()?;
    Some(Path::new(&home).join(".cargo"))
}

fn find_sys_source(workspace: &Path, version: &str) -> Option<PathBuf> {
    let dir_name = format!("oxrocksdb-sys-{version}");
    let vendored = workspace.join("vendor").join("oxrocksdb-sys");
    if vendored.exists() {
        return Some(vendored);
    }
    let src = cargo_home()?.join("registry").join("src");
    for index in fs::read_dir(src).ok()?.flatten() {
        let candidate = index.path().join(&dir_name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn rocksdb_version(header: &str) -> Option<String> {
    let get = |key: &str| {
        header.lines().find_map(|l| {
            let rest = l.trim().strip_prefix("#define ")?.strip_prefix(key)?;
            rest.trim().parse::<u32>().ok()
        })
    };
    Some(format!(
        "{}.{}.{}",
        get("ROCKSDB_MAJOR")?,
        get("ROCKSDB_MINOR")?,
        get("ROCKSDB_PATCH")?
    ))
}

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace = manifest.join("../..");
    let lock_path = workspace.join("Cargo.lock");
    println!("cargo:rerun-if-changed={}", lock_path.display());
    let lock = fs::read_to_string(&lock_path).expect("workspace Cargo.lock must exist");

    let oxigraph = locked_version(&lock, "oxigraph").expect("oxigraph must be in Cargo.lock");
    let sys = locked_version(&lock, "oxrocksdb-sys").expect("oxrocksdb-sys must be in Cargo.lock");
    let src = find_sys_source(&workspace, &sys).unwrap_or_else(|| {
        panic!("cannot find the oxrocksdb-sys {sys} source to read the bundled RocksDB version; set CARGO_HOME or vendor it under vendor/oxrocksdb-sys")
    });
    let header_path = src.join("rocksdb/include/rocksdb/version.h");
    let header = fs::read_to_string(&header_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", header_path.display()));
    let rocksdb = rocksdb_version(&header)
        .unwrap_or_else(|| panic!("no ROCKSDB_MAJOR/MINOR/PATCH in {}", header_path.display()));

    println!("cargo:rustc-env=SE_OXIGRAPH_VERSION={oxigraph}");
    println!("cargo:rustc-env=SE_ROCKSDB_VERSION={rocksdb}");
    println!(
        "cargo:rustc-env=SE_TARGET={}",
        env::var("TARGET").unwrap_or_default()
    );
    println!(
        "cargo:rustc-env=SE_PROFILE={}",
        env::var("PROFILE").unwrap_or_default()
    );
}
