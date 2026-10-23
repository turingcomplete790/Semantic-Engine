//! Tag recommendations, at parity with the plugin (spec 004 T036; research R13; SC-008).

use engine::recommend::recommend;
use engine::relations::defs::Definitions;
use serde_json::Value;
use stash_adapter::relations::TagRecord;
use std::collections::BTreeSet;
use std::path::Path;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/recommend")
            .join(name),
    )
    .unwrap()
}

fn defs() -> Definitions {
    let tags: Vec<TagRecord> = serde_json::from_str(&fixture("tags.json")).unwrap();
    Definitions::from_tags(&tags)
}

const CATEGORIES: [&str; 5] = ["actions", "roles", "pluggables", "miseEnScene", "relations"];

#[test]
fn every_category_matches_the_plugin_and_relations_are_added() {
    let d = defs();
    let items: Vec<Value> = serde_json::from_str(&fixture("items.json")).unwrap();
    for item in items {
        let native: Vec<String> = serde_json::from_value(item["native_tags"].clone()).unwrap();
        let got = serde_json::to_value(recommend(&native, &d)).unwrap();
        for c in CATEGORIES {
            let tags: BTreeSet<&str> = got[c]
                .as_array()
                .unwrap_or_else(|| panic!("{c} missing"))
                .iter()
                .map(|r| r["tag"].as_str().unwrap())
                .collect();
            let want: BTreeSet<&str> = item["expected"][c]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect();
            assert_eq!(tags, want, "item {} ({}), {c}", item["id"], item["note"]);
        }
    }
}

#[test]
fn each_recommendation_names_the_native_tag_that_suggested_it() {
    let d = defs();
    let r = serde_json::to_value(recommend(&["22".into(), "21".into()], &d)).unwrap();
    // Cowgirl once for itself and once for its variant.
    assert_eq!(
        r["actions"],
        serde_json::json!([{ "tag": "21", "from": "21" }, { "tag": "21", "from": "22" }])
    );
}

#[test]
fn an_item_without_vocabulary_tags_gets_empty_categories() {
    let r = serde_json::to_value(recommend(&[], &defs())).unwrap();
    for c in CATEGORIES {
        assert_eq!(r[c], serde_json::json!([]), "{c}");
    }
}
