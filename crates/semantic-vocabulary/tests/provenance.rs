mod common;
use semantic_vocabulary::load;

#[test]
fn every_term_has_required_annotations() {
    let v = load(&common::repo_vocab_dir()).unwrap();
    let viol: Vec<_> = v.check().into_iter().filter(|x| x.rule == "V005").collect();
    assert!(viol.is_empty(), "{viol:?}");
    for t in v.terms() {
        assert_eq!(t.values("sourceRevision").len(), 1, "{}", t.iri);
        assert!(!t.values("source").is_empty(), "{}", t.iri);
    }
}

#[test]
fn deprecated_have_replacement_and_planned_start_with_planned() {
    let v = load(&common::repo_vocab_dir()).unwrap();
    let r = common::rules(&v.check());
    assert!(!r.contains(&"V007".to_string()), "{:?}", v.check());
    assert!(!r.contains(&"V008".to_string()), "{:?}", v.check());
    let look = v
        .term(&format!("{}hasLook", semantic_vocabulary::NS))
        .unwrap();
    assert_eq!(
        look.values("replacedBy"),
        vec![format!("{}hasPerformerVariation", semantic_vocabulary::NS)]
    );
}

#[test]
fn a_dangling_replacement_is_reported() {
    let dir = common::scratch_copy();
    let core = common::read(&dir, "core.ttl");
    let damaged = core.replacen(
        "sem:replacedBy sem:Archetype",
        "sem:replacedBy sem:NoSuchTerm",
        1,
    );
    assert_ne!(core, damaged);
    common::write(&dir, "core.ttl", &damaged);
    let v = load(&dir).unwrap();
    assert!(common::rules(&v.check()).contains(&"V007".to_string()));
}
