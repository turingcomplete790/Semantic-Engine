//! Every active mapping entry of `stash-mapping.ttl` is exercised by the fixture (spec 007 T033;
//! FR-020, SC-002).

mod common;

use common::build::{fixture_library, iris};
use engine::build::map_library;
use engine::graph::Term;
use std::collections::{BTreeMap, BTreeSet};

const SEM: &str = "urn:semantic-stash:vocab:";
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// The `sem:` terms named after "also produces" in a comment, up to the end of that sentence.
fn also_produces(comment: &str) -> Vec<String> {
    let lower = comment.to_lowercase();
    let Some(at) = lower.find("also produces") else {
        return Vec::new();
    };
    let rest = &comment[at..];
    let sentence = rest.split(". ").next().unwrap_or(rest);
    sentence
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == ':'))
        .filter_map(|w| w.strip_prefix("sem:"))
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// Each active mapping entry, with every term it produces.
fn active_mappings() -> BTreeMap<String, Vec<String>> {
    let v = semantic_vocabulary::embedded().unwrap();
    let file = v
        .files
        .iter()
        .find(|f| f.name == "stash-mapping.ttl")
        .unwrap();
    let mut by_subject: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for t in &file.triples {
        let s = t.subject.to_string();
        let s = s.trim_matches(['<', '>']).to_string();
        let p = t.predicate.as_str().to_string();
        // IRIs print as <iri>; literals as "value" with a datatype or language after it.
        let text = t.object.to_string();
        let o = if text.starts_with('<') {
            text.trim_matches(['<', '>']).to_string()
        } else {
            let end = text.rfind('"').unwrap_or(text.len());
            text[1..end].replace("\\\"", "\"")
        };
        by_subject.entry(s).or_default().push((p, o));
    }
    let mut out = BTreeMap::new();
    for (s, props) in by_subject {
        let has = |p: &str, o: &str| props.iter().any(|(pp, oo)| pp == p && oo == o);
        if !has(RDF_TYPE, &format!("{SEM}Mapping"))
            || !has(&format!("{SEM}status"), &format!("{SEM}Active"))
        {
            continue;
        }
        let mut terms: Vec<String> = props
            .iter()
            .filter(|(p, _)| p == &format!("{SEM}produces"))
            .map(|(_, o)| o.strip_prefix(SEM).unwrap().to_string())
            .collect();
        for (p, o) in &props {
            if p == "http://www.w3.org/2000/01/rdf-schema#comment" {
                terms.extend(also_produces(o));
            }
        }
        out.insert(s.strip_prefix(SEM).unwrap().to_string(), terms);
    }
    out
}

#[test]
fn every_active_mapping_entry_has_a_fixture_fact() {
    let m = map_library(&fixture_library(), &iris());
    let mut used = BTreeSet::new();
    for f in &m.facts {
        if let Term::Iri(p) = &f.predicate {
            if p == RDF_TYPE {
                if let Term::Iri(o) = &f.object {
                    used.insert(o.clone());
                }
            } else {
                used.insert(p.clone());
            }
        }
    }
    let mappings = active_mappings();
    assert!(
        mappings.len() >= 30,
        "found only {} active entries",
        mappings.len()
    );
    let mut failures = Vec::new();
    for (entry, terms) in &mappings {
        for t in terms {
            if !used.contains(&format!("{SEM}{t}")) {
                failures.push(format!("sem:{entry} (sem:{t})"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "active mapping entries with no fixture fact:\n{}",
        failures.join("\n")
    );
}

#[test]
fn also_produces_reads_the_terms_of_its_sentence() {
    assert_eq!(
        also_produces("Also produces sem:waist, sem:hip and sem:waistToHipRatio. A value that cannot be derived is absent."),
        ["waist", "hip", "waistToHipRatio"]
    );
    assert_eq!(
        also_produces("A parent with the same type flag also produces sem:broaderRole or sem:broaderRelation (since 1.2.0)."),
        ["broaderRole", "broaderRelation"]
    );
}
