//! The tags a Relation or Role search criterion matches (spec 004 T033, US3, FR-013).

mod common;
use std::collections::BTreeSet;

fn set(xs: &[&str]) -> BTreeSet<String> {
    xs.iter().map(|s| s.to_string()).collect()
}

#[test]
fn recursive_criteria_include_every_narrower_term() {
    let m = common::example_model();
    let h = &m.hierarchy;
    assert_eq!(h.expand_criterion("40", true), set(&["40", "41"]));
    assert_eq!(h.expand_criterion("30", true), set(&["30", "31"]));
    assert_eq!(h.expand_criterion("32", true), set(&["32", "33", "34"]));
}

#[test]
fn non_recursive_criteria_match_only_the_named_tag() {
    let m = common::example_model();
    assert_eq!(m.hierarchy.expand_criterion("40", false), set(&["40"]));
    assert_eq!(m.hierarchy.expand_criterion("30", false), set(&["30"]));
}

#[test]
fn a_tag_without_narrower_terms_matches_itself() {
    let m = common::example_model();
    assert_eq!(m.hierarchy.expand_criterion("41", true), set(&["41"]));
}
