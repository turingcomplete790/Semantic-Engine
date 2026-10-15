mod common;
use semantic_vocabulary::load;
use std::process::Command;

#[test]
fn rapper_parses_each_file_with_the_same_triple_count() {
    let probe = Command::new("rapper").arg("--version").output();
    if probe.is_err() {
        eprintln!("notice: rapper not installed; second-parser test skipped");
        return;
    }
    let v = load(&common::repo_vocab_dir()).unwrap();
    for f in &v.files {
        let out = Command::new("rapper")
            .args(["-i", "turtle", "-c"])
            .arg(common::repo_vocab_dir().join(&f.name))
            .output()
            .unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(out.status.success(), "rapper failed on {}: {err}", f.name);
        assert!(
            !err.contains("Warning") && !err.contains("Error"),
            "{}: {err}",
            f.name
        );
        let n: usize = err
            .split_whitespace()
            .skip_while(|w| *w != "returned")
            .nth(1)
            .and_then(|w| w.parse().ok())
            .unwrap_or_else(|| panic!("could not read triple count from: {err}"));
        assert_eq!(n, f.triples.len(), "triple count differs in {}", f.name);
    }
}
