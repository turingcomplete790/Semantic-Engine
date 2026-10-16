mod common;
use std::process::Command;

fn run(dir: &std::path::Path) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_vocab-check"))
        .arg("--dir")
        .arg(dir)
        .output()
        .unwrap();
    (
        out.status.code().unwrap(),
        String::from_utf8_lossy(&out.stdout).to_string(),
    )
}

#[test]
fn clean_directory_exits_zero() {
    let (code, out) = run(&common::repo_vocab_dir());
    assert_eq!(code, 0, "{out}");
}

#[test]
fn missing_annotation_exits_one_naming_the_rule() {
    let dir = common::scratch_copy();
    let core = common::read(&dir, "core.ttl");
    let damaged = core.replacen("    sem:sourceRevision ", "    sem:removedRevision ", 1);
    common::write(&dir, "core.ttl", &damaged);
    let (code, out) = run(&dir);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("V005") || out.contains("V003"), "{out}");
}

#[test]
fn syntax_error_exits_two() {
    let dir = common::scratch_copy();
    common::write(&dir, "timeline.ttl", "this is not turtle");
    let (code, _) = run(&dir);
    assert_eq!(code, 2);
}

#[test]
fn json_format_has_contract_shape() {
    let out = Command::new(env!("CARGO_BIN_EXE_vocab-check"))
        .args(["--format", "json", "--dir"])
        .arg(common::repo_vocab_dir())
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["contractVersion"], "1.0.0");
    assert_eq!(v["ok"], true);
    assert!(v["violations"].as_array().unwrap().is_empty());
    assert!(v["vocabularyVersion"].is_string());
}

#[test]
fn crlf_terms_md_is_still_current() {
    // A Windows checkout can turn LF into CRLF; that must not fail V011.
    let dir = common::scratch_copy();
    let terms = common::read(&dir, "TERMS.md").replace('\n', "\r\n");
    common::write(&dir, "TERMS.md", &terms);
    let (code, out) = run(&dir);
    assert_eq!(code, 0, "{out}");
}
