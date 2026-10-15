mod common;
use semantic_vocabulary::load;

fn set_version(dir: &std::path::Path, ver: &str) {
    let core = common::read(dir, "core.ttl");
    let start = core.find("sem:version \"").unwrap() + "sem:version \"".len();
    let end = core[start..].find('"').unwrap() + start;
    let core = format!("{}{}{}", &core[..start], ver, &core[end..]);
    common::write(dir, "core.ttl", &core);
    let cl = common::read(dir, "CHANGELOG.md");
    let h = cl.find("\n## ").unwrap() + 4;
    let he = cl[h..].find(' ').unwrap() + h;
    common::write(
        dir,
        "CHANGELOG.md",
        &format!("{}{}{}", &cl[..h], ver, &cl[he..]),
    );
    // Keep TERMS.md out of the way: regenerate through the library.
    let v = load(dir).unwrap();
    common::write(dir, "TERMS.md", &v.generate_terms_md());
}

#[test]
fn unchanged_release_passes() {
    let dir = common::scratch_copy();
    let v = load(&dir).unwrap();
    assert!(v.check().is_empty(), "{:?}", v.check());
}

#[test]
fn no_snapshot_passes_with_notice() {
    let dir = common::scratch_copy();
    for e in std::fs::read_dir(dir.join("released")).unwrap().flatten() {
        std::fs::remove_file(e.path()).unwrap();
    }
    let v = load(&dir).unwrap();
    assert!(!common::rules(&v.check()).contains(&"V014".to_string()));
    assert!(v.release_notice().is_some());
}

#[test]
fn removed_term_without_major_bump_fails_naming_the_term() {
    let dir = common::scratch_copy();
    let l = common::read(&dir, "labels.ttl");
    // drop hasValue2 entirely
    let start = l.find("sem:hasValue2 a rdf:Property").unwrap();
    let end = l[start..].find(" .\n").unwrap() + start + 3;
    common::write(&dir, "labels.ttl", &format!("{}{}", &l[..start], &l[end..]));
    set_version(&dir, "1.2.0");
    let v = load(&dir).unwrap();
    let viol = v.check();
    let hit = viol
        .iter()
        .find(|x| x.rule == "V014")
        .expect("V014 expected");
    assert!(hit.message.contains("hasValue2"), "{hit:?}");
}

#[test]
fn removed_term_with_major_bump_passes() {
    let dir = common::scratch_copy();
    let l = common::read(&dir, "labels.ttl");
    let start = l.find("sem:hasValue2 a rdf:Property").unwrap();
    let end = l[start..].find(" .\n").unwrap() + start + 3;
    common::write(&dir, "labels.ttl", &format!("{}{}", &l[..start], &l[end..]));
    set_version(&dir, "2.0.0");
    let v = load(&dir).unwrap();
    assert!(
        !common::rules(&v.check()).contains(&"V014".to_string()),
        "{:?}",
        v.check()
    );
}

#[test]
fn added_term_without_any_bump_fails() {
    let dir = common::scratch_copy();
    let l = common::read(&dir, "labels.ttl");
    let extra = r#"
sem:extraProp a rdf:Property ;
    rdfs:domain sem:Label ;
    rdfs:range xsd:string ;
    rdfs:label "extra"@en ;
    rdfs:comment "test" ;
    sem:source "test" ;
    sem:sourceRevision "test" ;
    sem:scope sem:Global ;
    sem:status sem:Active ;
    sem:since "1.0.0" .
"#;
    common::write(&dir, "labels.ttl", &format!("{l}{extra}"));
    let v = load(&dir).unwrap();
    assert!(common::rules(&v.check()).contains(&"V014".to_string()));
}
