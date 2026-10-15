mod common;
use semantic_vocabulary::{load, Error};

#[test]
fn loads_all_four_files() {
    let v = load(&common::repo_vocab_dir()).expect("vocabulary must parse");
    let names: Vec<&str> = v.files.iter().map(|f| f.name.as_str()).collect();
    for f in common::FILES {
        assert!(names.contains(&f), "missing {f}");
    }
    assert!(v.files.iter().all(|f| !f.triples.is_empty()));
}

#[test]
fn syntax_error_is_a_parse_error_naming_the_file() {
    let dir = common::scratch_copy();
    common::write(
        &dir,
        "labels.ttl",
        "@prefix sem: <urn:semantic-stash:vocab:> .\nsem:X a .\n",
    );
    match load(&dir) {
        Err(Error::Parse { file, .. }) => assert_eq!(file, "labels.ttl"),
        other => panic!("expected parse error, got {:?}", other.map(|_| ())),
    }
}
