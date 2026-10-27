//! The vocabulary's own facts, written with every build (research R7), so field rules resolve
//! through `pluginFieldName` and queries can follow sub-classes and sub-properties.

use crate::graph::{from_ox_triple, Fact};

/// The graph the vocabulary's facts are kept in (spec 008 research R1).
pub const VOCAB_OWNER: &str = "urn:semantic-stash:vocab:";

/// The vocabulary files that describe the graph. `stash-mapping.ttl` describes Stash keys, and
/// is not written.
pub const GRAPH_FILES: [&str; 3] = ["core.ttl", "labels.ttl", "timeline.ttl"];

/// Every fact of [`GRAPH_FILES`], from the vocabulary embedded in the engine.
pub fn vocabulary_facts() -> Vec<Fact> {
    let vocabulary = semantic_vocabulary::embedded().expect("the embedded vocabulary parses");
    vocabulary
        .files
        .iter()
        .filter(|f| GRAPH_FILES.contains(&f.name.as_str()))
        .flat_map(|f| f.triples.iter().map(from_ox_triple))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Term;

    const SEM: &str = "urn:semantic-stash:vocab:";

    #[test]
    fn vocabulary_facts_hold_the_graph_terms_and_not_the_mapping() {
        let facts = vocabulary_facts();
        let has =
            |s: &str, p: &str, o: Term| facts.contains(&Fact::new(Term::iri(s), Term::iri(p), o));
        assert!(has(
            &format!("{SEM}bustSize"),
            &format!("{SEM}pluginFieldName"),
            Term::string("__derived_bust")
        ));
        assert!(has(
            &format!("{SEM}pairsA"),
            "http://www.w3.org/2000/01/rdf-schema#subPropertyOf",
            Term::iri(format!("{SEM}pairs"))
        ));
        assert!(!facts
            .iter()
            .any(|f| matches!(&f.subject, Term::Iri(s) if s.starts_with(&format!("{SEM}map_")))));
    }
}
