fn semver(s: &str) -> bool {
    let p: Vec<&str> = s.split('.').collect();
    p.len() == 3
        && p.iter()
            .all(|x| !x.is_empty() && x.chars().all(|c| c.is_ascii_digit()))
}

#[test]
fn versions_are_complete() {
    let v = engine::versions();
    assert!(!v.engine_version.is_empty());
    assert_eq!(v.contract_version, "1.0.0");
    assert_eq!(v.stash_minimum_version, "v0.31.1");
    let vocab = semantic_vocabulary::embedded().unwrap().version().unwrap();
    assert_eq!(v.vocabulary_version, vocab);
    assert!(semver(&v.oxigraph_version), "{}", v.oxigraph_version);
    assert!(semver(&v.rocksdb_version), "{}", v.rocksdb_version);
}

#[test]
fn versions_serialize_camel_case() {
    let j = serde_json::to_value(engine::versions()).unwrap();
    for k in [
        "engineVersion",
        "contractVersion",
        "vocabularyVersion",
        "oxigraphVersion",
        "rocksdbVersion",
        "stashMinimumVersion",
        "build",
    ] {
        assert!(j.get(k).is_some(), "missing {k}");
    }
}
