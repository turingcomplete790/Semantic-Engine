//! `Hierarchy::from_links` means what `Hierarchy::build` means (spec 009 T027; research R5):
//! the graph's `broaderRole` and `broaderRelation` links, loops included, give the same
//! hierarchy as the definitions they were built from.

mod common;
use engine::relations::defs::Definitions;
use engine::relations::hierarchy::Hierarchy;
use stash_adapter::relations::TagRecord;

/// The (child, broader) pairs of same-type parents: what the graph holds as broader links.
fn links(d: &Definitions) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (id, t) in &d.tags {
        for p in &t.parents {
            if (d.is_role(id) && d.is_role(p)) || (d.is_relation(id) && d.is_relation(p)) {
                out.push((id.clone(), p.clone()));
            }
        }
    }
    out
}

fn same(tags: Vec<TagRecord>, what: &str) {
    let mut d = Definitions::from_tags(&tags);
    let built = Hierarchy::build(&mut d);
    let linked = Hierarchy::from_links(links(&d));
    let ids: Vec<&String> = d.tags.keys().collect();
    assert!(!ids.is_empty());
    for id in ids {
        assert_eq!(
            built.broader(id),
            linked.broader(id),
            "{what}: broader {id}"
        );
        assert_eq!(
            built.narrower(id, true),
            linked.narrower(id, true),
            "{what}: narrower {id}"
        );
        for recursive in [false, true] {
            assert_eq!(
                built.expand_criterion(id, recursive),
                linked.expand_criterion(id, recursive),
                "{what}: expand {id} recursive={recursive}"
            );
        }
    }
}

#[test]
fn each_definitions_fixture_gives_the_same_hierarchy() {
    same(common::tags(&["example1.json"]), "example1");
    same(common::tags(&["example2.json"]), "example2");
    same(common::example_tags(), "example1 + example2");
}

#[test]
fn the_loop_fixture_gives_the_same_hierarchy() {
    let tags = common::all_tags();
    let mut d = Definitions::from_tags(&tags);
    let looped = links(&d);
    assert!(
        looped.iter().any(|(c, p)| c == "70" && p == "71"),
        "the loop is in the links: {looped:?}"
    );
    let linked = Hierarchy::from_links(looped);
    assert!(
        linked.narrower("70", true).is_empty(),
        "loop links are removed"
    );
    let _ = Hierarchy::build(&mut d);
    same(tags, "invalid (with the loop)");
}
