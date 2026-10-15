mod common;
use semantic_vocabulary::load;
use std::fs;

fn expected() -> Vec<(String, String, String, Option<String>)> {
    let text = fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/expected-terms.txt"),
    )
    .unwrap();
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            let p: Vec<&str> = l.split_whitespace().collect();
            (
                p[0].to_string(),
                p[1].to_string(),
                p[2].to_string(),
                p.get(3).map(|s| s.to_string()),
            )
        })
        .collect()
}

#[test]
fn every_expected_term_is_defined_in_the_named_file() {
    let v = load(&common::repo_vocab_dir()).unwrap();
    for (kind, name, file, _) in expected() {
        let iri = format!("{}{}", semantic_vocabulary::NS, name);
        let term = v
            .term(&iri)
            .unwrap_or_else(|| panic!("sem:{name} not defined"));
        assert_eq!(term.file, file, "sem:{name} is in the wrong file");
        assert_eq!(
            term.kind.as_str(),
            if kind == "class" { "class" } else { "property" }
        );
    }
}

#[test]
fn there_are_no_unexpected_terms() {
    let v = load(&common::repo_vocab_dir()).unwrap();
    let names: Vec<String> = expected().into_iter().map(|e| e.1).collect();
    for t in v.terms() {
        let local = t.iri.strip_prefix(semantic_vocabulary::NS).unwrap();
        assert!(
            names.contains(&local.to_string()),
            "unexpected term sem:{local}"
        );
    }
}

#[test]
fn defined_term_and_single_file_rules_pass() {
    let v = load(&common::repo_vocab_dir()).unwrap();
    let r = common::rules(&v.check());
    for rule in ["V003", "V012"] {
        assert!(
            !r.contains(&rule.to_string()),
            "{rule} violated: {:?}",
            v.check()
        );
    }
}

#[test]
fn stash_entities_are_referenced_not_redefined() {
    let dir = common::scratch_copy();
    let core = common::read(&dir, "core.ttl");
    // Drop the references annotation from Scene: V013 must report it.
    let damaged = core.replacen("    sem:references \"Scene\" ;\n", "", 1);
    assert_ne!(core, damaged);
    common::write(&dir, "core.ttl", &damaged);
    let v = load(&dir).unwrap();
    assert!(common::rules(&v.check()).contains(&"V013".to_string()));
    // And a class named like a Stash entity is reported.
    let dir2 = common::scratch_copy();
    let l = common::read(&dir2, "labels.ttl");
    common::write(
        &dir2,
        "labels.ttl",
        &format!("{l}\nsem:Gallery a rdfs:Class ;\n    rdfs:label \"Gallery\"@en ;\n    rdfs:comment \"x\" ;\n    sem:source \"x\" ;\n    sem:sourceRevision \"x\" ;\n    sem:status sem:Active ;\n    sem:since \"1.0.0\" .\n"),
    );
    let v2 = load(&dir2).unwrap();
    assert!(common::rules(&v2.check()).contains(&"V013".to_string()));
}
