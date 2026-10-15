mod common;
use semantic_vocabulary::{load, TermKind};

#[test]
fn every_property_has_exactly_one_domain_and_range_and_no_owl() {
    let v = load(&common::repo_vocab_dir()).unwrap();
    let r = common::rules(&v.check());
    assert!(!r.contains(&"V004".to_string()), "{:?}", v.check());
    assert!(!r.contains(&"V002".to_string()), "{:?}", v.check());
    assert!(v.terms().any(|t| t.kind == TermKind::Property));
}

#[test]
fn scopes_match_the_expected_terms() {
    let v = load(&common::repo_vocab_dir()).unwrap();
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/expected-terms.txt"),
    )
    .unwrap();
    for l in text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
    {
        let p: Vec<&str> = l.split_whitespace().collect();
        let Some(scope) = p.get(3) else { continue };
        let iri = format!("{}{}", semantic_vocabulary::NS, p[1]);
        let t = v.term(&iri).unwrap();
        let got = t.values("scope");
        assert_eq!(
            got,
            vec![format!("{}{}", semantic_vocabulary::NS, scope)],
            "scope of sem:{}",
            p[1]
        );
    }
    let r = common::rules(&v.check());
    assert!(!r.contains(&"V006".to_string()), "{:?}", v.check());
}
