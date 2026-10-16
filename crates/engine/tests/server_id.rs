use engine::server_id::{
    ensure_server_id, is_valid_server_id, read_server_id, Outcome, ServerIdError, SettingsStore,
    StoreError, SETTING_KEY,
};
use serde_json::{json, Map, Value};
use std::cell::RefCell;

#[derive(Default)]
struct Memory {
    map: RefCell<Map<String, Value>>,
    writes: RefCell<usize>,
}

impl SettingsStore for Memory {
    fn read(&self) -> Result<Map<String, Value>, StoreError> {
        Ok(self.map.borrow().clone())
    }
    fn write(&self, map: Map<String, Value>) -> Result<Map<String, Value>, StoreError> {
        *self.writes.borrow_mut() += 1;
        *self.map.borrow_mut() = map.clone();
        Ok(map)
    }
}

fn with(v: Value) -> Memory {
    let m = Memory::default();
    *m.map.borrow_mut() = v.as_object().unwrap().clone();
    m
}

#[test]
fn generates_once_and_keeps_other_settings() {
    let store = with(json!({"other": "keep"}));
    let first = ensure_server_id(&store).unwrap();
    let Outcome::Generated(id) = first else {
        panic!("expected generated, got {first:?}")
    };
    assert!(is_valid_server_id(&id), "{id}");
    assert_eq!(store.map.borrow()["other"], "keep");
    assert_eq!(store.map.borrow()[SETTING_KEY], json!(id));
    assert_eq!(
        ensure_server_id(&store).unwrap(),
        Outcome::Existing(id.clone())
    );
    assert_eq!(*store.writes.borrow(), 1, "a second run must not write");
}

#[test]
fn an_empty_value_counts_as_absent() {
    let store = with(json!({ "serverId": "" }));
    assert!(matches!(
        ensure_server_id(&store).unwrap(),
        Outcome::Generated(_)
    ));
}

#[test]
fn invalid_value_is_reported_and_never_overwritten() {
    let store = with(json!({ "serverId": "Not-A-UUID" }));
    assert_eq!(
        ensure_server_id(&store).unwrap_err(),
        ServerIdError::Invalid("Not-A-UUID".into())
    );
    assert_eq!(*store.writes.borrow(), 0);
    assert_eq!(store.map.borrow()[SETTING_KEY], "Not-A-UUID");
}

#[test]
fn reading_never_generates() {
    let store = Memory::default();
    assert_eq!(
        read_server_id(&store).unwrap_err(),
        ServerIdError::NotGenerated
    );
    assert_eq!(*store.writes.borrow(), 0);
}

#[test]
fn validity_rule_matches_the_data_model() {
    assert!(is_valid_server_id("00000000-0000-4000-8000-000000000001"));
    for bad in [
        "00000000-0000-4000-8000-00000000000A",
        "00000000000040008000000000000001",
        "{00000000-0000-4000-8000-000000000001}",
        "",
    ] {
        assert!(!is_valid_server_id(bad), "{bad}");
    }
}
