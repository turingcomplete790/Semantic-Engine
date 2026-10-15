mod common;
use semantic_vocabulary::load;

#[test]
fn version_is_declared_once_and_matches_changelog() {
    let v = load(&common::repo_vocab_dir()).unwrap();
    assert_eq!(v.declared_versions().len(), 1);
    let r = common::rules(&v.check());
    assert!(!r.contains(&"V009".to_string()), "{:?}", v.check());
}

#[test]
fn mismatch_with_changelog_is_reported() {
    let dir = common::scratch_copy();
    let cl = common::read(&dir, "CHANGELOG.md");
    common::write(&dir, "CHANGELOG.md", &cl.replace("## 1.0.0", "## 9.9.9"));
    let v = load(&dir).unwrap();
    assert!(common::rules(&v.check()).contains(&"V009".to_string()));
}

#[test]
fn two_declarations_are_reported() {
    let dir = common::scratch_copy();
    let l = common::read(&dir, "labels.ttl");
    common::write(
        &dir,
        "labels.ttl",
        &format!("{l}\n<urn:semantic-stash:vocab:> sem:version \"1.0.0\" .\n"),
    );
    let v = load(&dir).unwrap();
    assert!(common::rules(&v.check()).contains(&"V009".to_string()));
}
