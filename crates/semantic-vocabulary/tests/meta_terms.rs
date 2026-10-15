mod common;
use semantic_vocabulary::load;

const META: &[&str] = &[
    "source",
    "sourceRevision",
    "scope",
    "status",
    "since",
    "replacedBy",
    "retiredNote",
    "references",
    "version",
    "fromEntity",
    "fromField",
    "allowedValues",
    "produces",
    "Mapping",
    "RetiredField",
    "Scope",
    "Status",
    "SceneScoped",
    "Global",
    "Active",
    "Deprecated",
    "Planned",
    "Derived",
    "serverIdentifier",
];

#[test]
fn every_meta_term_is_defined_with_a_type() {
    let v = load(&common::repo_vocab_dir()).unwrap();
    for name in META {
        let iri = format!("{}{}", semantic_vocabulary::NS, name);
        assert!(v.is_defined(&iri), "meta term sem:{name} is not defined");
    }
}

#[test]
fn meta_properties_and_classes_carry_required_annotations() {
    let v = load(&common::repo_vocab_dir()).unwrap();
    let violations = v.check();
    let meta_iris: Vec<String> = META
        .iter()
        .map(|n| format!("{}{}", semantic_vocabulary::NS, n))
        .collect();
    for viol in violations {
        if let Some(t) = &viol.term {
            assert!(
                !meta_iris.contains(t),
                "meta term violates {}: {}",
                viol.rule,
                viol.message
            );
        }
    }
}
