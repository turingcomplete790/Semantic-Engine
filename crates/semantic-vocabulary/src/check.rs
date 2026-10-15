//! The vocabulary check: rules V001 to V014 (see contracts/vocabulary-check.md).

use crate::{TermInfo, TermKind, Vocabulary, NS, RDF, RDFS, XSD};
use oxrdf::{Subject, Term};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Violation {
    pub rule: String,
    pub term: Option<String>,
    pub file: Option<String>,
    pub line: Option<usize>,
    pub message: String,
}

fn v(rule: &str, term: Option<&str>, file: Option<&str>, message: String) -> Violation {
    Violation {
        rule: rule.into(),
        term: term.map(|s| s.into()),
        file: file.map(|s| s.into()),
        line: None,
        message,
    }
}

fn locate(voc: &Vocabulary, file: &str, iri: &str) -> Option<usize> {
    let local = iri.strip_prefix(NS)?;
    let needle = format!("sem:{local} ");
    let f = voc.files.iter().find(|f| f.name == file)?;
    f.text
        .lines()
        .position(|l| l.starts_with(&needle))
        .map(|i| i + 1)
}

fn short(iri: &str) -> String {
    iri.strip_prefix(NS)
        .map(|s| format!("sem:{s}"))
        .unwrap_or_else(|| iri.to_string())
}

fn semver(s: &str) -> Option<(u64, u64, u64)> {
    let mut it = s.split('.');
    let a = it.next()?.parse().ok()?;
    let b = it.next()?.parse().ok()?;
    let c = it.next()?.parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some((a, b, c))
}

const FORBIDDEN_STASH_CLASSES: [&str; 5] = ["Gallery", "Studio", "Group", "Movie", "SceneMarker"];
const MUST_REFERENCE: [&str; 5] = ["Scene", "Image", "Performer", "Tag", "Marker"];
const STATUSES: [&str; 4] = ["Active", "Deprecated", "Planned", "Derived"];

pub fn run(voc: &Vocabulary) -> Vec<Violation> {
    let mut out = Vec::new();
    let terms: Vec<TermInfo> = voc.terms().collect();
    let term_set: BTreeSet<String> = terms.iter().map(|t| t.iri.clone()).collect();
    let term_map: BTreeMap<&str, &TermInfo> = terms.iter().map(|t| (t.iri.as_str(), t)).collect();

    // V002: only sem, rdf, rdfs, xsd namespaces.
    let allowed = [NS, RDF, RDFS, XSD];
    for (file, t) in voc.all_triples() {
        let mut iris: Vec<String> = Vec::new();
        if let Subject::NamedNode(s) = &t.subject {
            iris.push(s.as_str().into());
        }
        iris.push(t.predicate.as_str().into());
        match &t.object {
            Term::NamedNode(n) => iris.push(n.as_str().into()),
            Term::Literal(l) => iris.push(l.datatype().as_str().into()),
            _ => {}
        }
        for iri in iris {
            if !allowed.iter().any(|p| iri.starts_with(p)) {
                out.push(v(
                    "V002",
                    Some(&iri),
                    Some(file),
                    format!("namespace not allowed: {iri}"),
                ));
            }
        }
    }

    // V003: every sem: IRI used is defined.
    let mut used: BTreeMap<String, String> = BTreeMap::new();
    for (file, t) in voc.all_triples() {
        let mut iris: Vec<String> = Vec::new();
        if let Subject::NamedNode(s) = &t.subject {
            iris.push(s.as_str().into());
        }
        iris.push(t.predicate.as_str().into());
        if let Term::NamedNode(n) = &t.object {
            iris.push(n.as_str().into());
        }
        for iri in iris {
            if iri.starts_with(NS) {
                used.entry(iri).or_insert_with(|| file.to_string());
            }
        }
    }
    for (iri, file) in &used {
        if !voc.is_defined(iri) {
            let mut x = v(
                "V003",
                Some(iri),
                Some(file),
                format!("{} is used but not defined", short(iri)),
            );
            x.line = None;
            out.push(x);
        }
    }

    // V004: exactly one domain and one range per property.
    for t in terms.iter().filter(|t| t.kind == TermKind::Property) {
        for (pred, label) in [("rdfs:domain", "domain"), ("rdfs:range", "range")] {
            let n = t.values(pred).len();
            if n != 1 {
                let mut x = v(
                    "V004",
                    Some(&t.iri),
                    Some(&t.file),
                    format!("{} has {n} {label} values, expected 1", short(&t.iri)),
                );
                x.line = locate(voc, &t.file, &t.iri);
                out.push(x);
            }
        }
    }

    // helpers over mapping / retired entries
    let mapping_subjects = voc.typed(&format!("{NS}Mapping"));
    let retired_subjects = voc.typed(&format!("{NS}RetiredField"));

    // V005: annotations on terms.
    let check_ann = |out: &mut Vec<Violation>,
                     iri: &str,
                     file: &str,
                     vals: &dyn Fn(&str) -> Vec<String>,
                     langs_ok: bool,
                     need_comment: bool| {
        let mut need = vec![
            ("sourceRevision", Some(1usize)),
            ("status", Some(1)),
            ("since", Some(1)),
            ("source", None),
        ];
        if need_comment {
            need.push(("rdfs:label", Some(1)));
            need.push(("rdfs:comment", Some(1)));
        }
        for (name, exact) in need {
            let n = vals(name).len();
            let bad = match exact {
                Some(e) => n != e,
                None => n == 0,
            };
            if bad {
                let mut x = v(
                    "V005",
                    Some(iri),
                    Some(file),
                    format!("{} annotation {name} has {n} value(s)", short(iri)),
                );
                x.line = locate(voc, file, iri);
                out.push(x);
            }
        }
        if !langs_ok {
            out.push(v(
                "V005",
                Some(iri),
                Some(file),
                format!("{} label must be language-tagged @en", short(iri)),
            ));
        }
        for s in vals("status") {
            let local = s.strip_prefix(NS).unwrap_or("");
            if !STATUSES.contains(&local) {
                out.push(v(
                    "V005",
                    Some(iri),
                    Some(file),
                    format!("{} has unknown status {s}", short(iri)),
                ));
            }
        }
    };
    for t in &terms {
        let langs_ok = t.label_langs.len() == 1 && t.label_langs[0].as_deref() == Some("en");
        check_ann(
            &mut out,
            &t.iri,
            &t.file,
            &|n| t.values(n),
            langs_ok || t.label_langs.len() != 1,
            true,
        );
    }
    let entry_props = |iri: &str| voc.term_like(iri);
    for (iri, file) in mapping_subjects.iter().chain(retired_subjects.iter()) {
        let props = entry_props(iri);
        let get = |n: &str| props.get(&crate::expand(n)).cloned().unwrap_or_default();
        check_ann(&mut out, iri, file, &get, true, false);
    }

    // V006: scope on properties and assignment classes.
    let mut subclass: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for t in terms.iter().filter(|t| t.kind == TermKind::Class) {
        subclass.insert(t.iri.clone(), t.values("rdfs:subClassOf"));
    }
    let is_assignment = |iri: &str| -> bool {
        let roots = [format!("{NS}TagAssignment"), format!("{NS}Pairing")];
        let mut stack = vec![iri.to_string()];
        let mut seen = BTreeSet::new();
        while let Some(c) = stack.pop() {
            if roots.contains(&c) {
                return true;
            }
            if seen.insert(c.clone()) {
                if let Some(p) = subclass.get(&c) {
                    stack.extend(p.iter().cloned());
                }
            }
        }
        false
    };
    for t in &terms {
        let needs = t.kind == TermKind::Property || is_assignment(&t.iri);
        if needs {
            let n = t.values("scope").len();
            if n != 1 {
                let mut x = v(
                    "V006",
                    Some(&t.iri),
                    Some(&t.file),
                    format!("{} needs exactly one sem:scope, has {n}", short(&t.iri)),
                );
                x.line = locate(voc, &t.file, &t.iri);
                out.push(x);
            }
        }
    }

    // V007 and V008: deprecated and planned.
    let status_of = |props: &BTreeMap<String, Vec<String>>| -> String {
        props
            .get(&crate::expand("status"))
            .and_then(|v| v.first())
            .and_then(|s| s.strip_prefix(NS))
            .unwrap_or("")
            .to_string()
    };
    let mut all_entries: Vec<(String, String, BTreeMap<String, Vec<String>>)> = Vec::new();
    for t in &terms {
        all_entries.push((t.iri.clone(), t.file.clone(), t.props.clone()));
    }
    for (iri, file) in mapping_subjects.iter().chain(retired_subjects.iter()) {
        all_entries.push((iri.clone(), file.clone(), voc.term_like(iri)));
    }
    for (iri, file, props) in &all_entries {
        let st = status_of(props);
        if st == "Deprecated" {
            let rep = props
                .get(&crate::expand("replacedBy"))
                .cloned()
                .unwrap_or_default();
            let note = props
                .get(&crate::expand("retiredNote"))
                .cloned()
                .unwrap_or_default();
            if rep.is_empty() && note.is_empty() {
                out.push(v(
                    "V007",
                    Some(iri),
                    Some(file),
                    format!(
                        "{} is deprecated without replacedBy or retiredNote",
                        short(iri)
                    ),
                ));
            }
            for r in rep {
                match term_map.get(r.as_str()) {
                    None => out.push(v(
                        "V007",
                        Some(iri),
                        Some(file),
                        format!("replacement {} of {} is not defined", short(&r), short(iri)),
                    )),
                    Some(t) => {
                        let s = t
                            .values("status")
                            .first()
                            .and_then(|s| s.strip_prefix(NS).map(|x| x.to_string()))
                            .unwrap_or_default();
                        if s != "Active" {
                            out.push(v(
                                "V007",
                                Some(iri),
                                Some(file),
                                format!(
                                    "replacement {} of {} is not active",
                                    short(&r),
                                    short(iri)
                                ),
                            ));
                        }
                    }
                }
            }
        }
        if st == "Planned" {
            let c = props
                .get(&format!("{RDFS}comment"))
                .cloned()
                .unwrap_or_default();
            if !c
                .first()
                .map(|s| s.starts_with("Planned:"))
                .unwrap_or(false)
            {
                out.push(v(
                    "V008",
                    Some(iri),
                    Some(file),
                    format!(
                        "planned entry {} must have a comment starting \"Planned:\"",
                        short(iri)
                    ),
                ));
            }
        }
    }

    // V009: version declared once and matching the change log.
    let versions = voc.declared_versions();
    if versions.len() != 1 {
        out.push(v(
            "V009",
            None,
            None,
            format!("version declared {} times, expected once", versions.len()),
        ));
    }
    match (&voc.changelog, versions.first()) {
        (None, _) => out.push(v(
            "V009",
            None,
            Some("CHANGELOG.md"),
            "CHANGELOG.md is missing".into(),
        )),
        (Some(cl), Some(ver)) => {
            let top = cl
                .lines()
                .find(|l| l.starts_with("## "))
                .map(|l| l[3..].trim().to_string());
            let top_ver = top
                .as_deref()
                .and_then(|t| t.split_whitespace().next())
                .unwrap_or("");
            if top_ver != ver {
                out.push(v(
                    "V009",
                    None,
                    Some("CHANGELOG.md"),
                    format!("version {ver} does not match top change log entry {top_ver:?}"),
                ));
            }
        }
        _ => {}
    }

    // V010: mapping entries.
    for (iri, file) in &mapping_subjects {
        let p = voc.term_like(iri);
        let get = |n: &str| p.get(&crate::expand(n)).cloned().unwrap_or_default();
        for need in ["fromEntity", "fromField", "allowedValues"] {
            if get(need).is_empty() {
                out.push(v(
                    "V010",
                    Some(iri),
                    Some(file),
                    format!("mapping {} lacks {need}", short(iri)),
                ));
            }
        }
        let produces = get("produces");
        if produces.is_empty() {
            out.push(v(
                "V010",
                Some(iri),
                Some(file),
                format!("mapping {} lacks produces", short(iri)),
            ));
        }
        for t in produces {
            if !term_set.contains(&t) {
                out.push(v(
                    "V010",
                    Some(iri),
                    Some(file),
                    format!("mapping {} produces undefined {}", short(iri), short(&t)),
                ));
            }
        }
    }
    for (iri, file) in &retired_subjects {
        let p = voc.term_like(iri);
        if p.contains_key(&crate::expand("produces")) {
            out.push(v(
                "V010",
                Some(iri),
                Some(file),
                format!(
                    "retired field {} must not produce a term (FR-016)",
                    short(iri)
                ),
            ));
        }
    }

    // V011: TERMS.md matches.
    match &voc.terms_md {
        None => out.push(v(
            "V011",
            None,
            Some("TERMS.md"),
            "TERMS.md is missing".into(),
        )),
        Some(t) => {
            if t.trim_end() != voc.generate_terms_md().trim_end() {
                out.push(v(
                    "V011",
                    None,
                    Some("TERMS.md"),
                    "TERMS.md is out of date; run vocab-check --write-terms".into(),
                ));
            }
        }
    }

    // V012: a term is defined in exactly one file.
    let mut where_defined: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for ty in [format!("{RDFS}Class"), format!("{RDF}Property")] {
        for (iri, file) in voc.typed(&ty) {
            where_defined.entry(iri).or_default().insert(file);
        }
    }
    for (iri, files) in where_defined {
        if files.len() > 1 {
            out.push(v(
                "V012",
                Some(&iri),
                None,
                format!("{} is defined in several files: {:?}", short(&iri), files),
            ));
        }
    }

    // V013: Stash entities are referenced, not redefined.
    for t in terms.iter().filter(|t| t.kind == TermKind::Class) {
        let local = t.local();
        if FORBIDDEN_STASH_CLASSES.contains(&local) {
            out.push(v(
                "V013",
                Some(&t.iri),
                Some(&t.file),
                format!("{} duplicates a Stash entity", short(&t.iri)),
            ));
        }
        if MUST_REFERENCE.contains(&local) && t.values("references").is_empty() {
            out.push(v(
                "V013",
                Some(&t.iri),
                Some(&t.file),
                format!("{} must carry sem:references", short(&t.iri)),
            ));
        }
    }

    // V014: compare with the last release snapshot.
    if let (Some(cur_s), Some(last)) = (
        voc.version(),
        voc.snapshots
            .iter()
            .filter(|s| semver(&s.version).is_some())
            .max_by_key(|s| semver(&s.version).unwrap()),
    ) {
        match (semver(&cur_s), semver(&last.version)) {
            (Some(cur), Some(old)) => {
                if cur < old {
                    out.push(v(
                        "V014",
                        None,
                        None,
                        format!("version {cur_s} is lower than released {}", last.version),
                    ));
                }
                let now = voc.snapshot();
                let mut breaking = Vec::new();
                for (name, old_t) in &last.terms {
                    match now.terms.get(name) {
                        None => breaking.push(format!("sem:{name} was removed or renamed")),
                        Some(new_t) if new_t != old_t => {
                            breaking.push(format!("sem:{name} changed domain or range or kind"))
                        }
                        _ => {}
                    }
                }
                let added: Vec<_> = now
                    .terms
                    .keys()
                    .filter(|k| !last.terms.contains_key(*k))
                    .collect();
                if !breaking.is_empty() && cur.0 <= old.0 {
                    for b in breaking {
                        out.push(v(
                            "V014",
                            None,
                            None,
                            format!("{b}; needs a MAJOR above {}", last.version),
                        ));
                    }
                }
                if !added.is_empty() && cur <= old {
                    out.push(v(
                        "V014",
                        None,
                        None,
                        format!(
                            "{} term(s) added ({}…) but version {cur_s} is not above {}",
                            added.len(),
                            short(&format!("{NS}{}", added[0])),
                            last.version
                        ),
                    ));
                }
            }
            _ => out.push(v(
                "V014",
                None,
                None,
                "version is not MAJOR.MINOR.PATCH".into(),
            )),
        }
    }
    out
}
