//! Explaining one relation instance (spec 004 T018; research R10).

mod common;
use engine::relations::explain::explain;
use serde_json::json;

fn ex(scene: &str, rel: &str, a: &str, b: &str) -> serde_json::Value {
    explain(
        &common::scene(scene),
        &common::example_model(),
        &common::labels(),
        rel,
        a,
        b,
    )
    .unwrap()
}

#[test]
fn an_automatic_instance_names_its_rule_facts_link_and_roles() {
    let e = ex("109", "45", "7", "8");
    assert_eq!(e["applied"], "auto");
    assert_eq!(e["holds"], true);
    assert_eq!(e["ruleId"], "r1");
    assert_eq!(
        e["endA"],
        json!({ "performer": "7", "met": { "label": "50" } })
    );
    assert_eq!(
        e["endB"],
        json!({ "performer": "8", "met": { "label": "51" } })
    );
    assert_eq!(
        e["link"],
        json!({ "action": "21", "by": "pairing", "pairingId": "p109" })
    );
    assert_eq!(e["addedRoles"], json!({ "7": ["60"], "8": ["61"] }));
}

#[test]
fn a_role_end_met_through_a_narrower_role_says_which() {
    let model = common::example_model();
    let s = common::scene("110");
    let e = engine::relations::evaluate::matches(&s, &model, &common::labels()).unwrap();
    let p = engine::relations::diff::plan(&s, &e, &model)
        .unwrap()
        .unwrap();
    let applied = common::apply_patch(&s, &p.partial());
    let x = explain(&applied, &model, &common::labels(), "40", "16", "17").unwrap();
    assert_eq!(x["applied"], "auto");
    assert_eq!(x["endA"]["met"], json!({ "role": "30", "via": ["31"] }));
    assert_eq!(x["endB"]["met"], json!({ "role": "32", "via": ["34"] }));
    assert_eq!(x["link"]["action"], "23");
}

#[test]
fn hand_applied_and_absent_instances() {
    assert_eq!(ex("107", "45", "7", "8"), json!({ "applied": "hand" }));
    assert_eq!(ex("105", "45", "7", "8"), json!({ "applied": "none" }));
}

#[test]
fn a_stale_automatic_instance_reports_that_it_no_longer_holds() {
    let e = ex("112", "45", "7", "8");
    assert_eq!(e["applied"], "auto");
    assert_eq!(e["holds"], false);
}
