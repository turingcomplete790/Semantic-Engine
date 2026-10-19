//! The hook table: reading the hook log, merging rows, and the coverage rule (tasks T029).

use spike::bench::hooks::{merge_table, missing_pairs, read_new};
use spike::measure::{Entity, HookObservation, Mode, SavingClient};
use std::fs;

fn row(entity: Entity, client: SavingClient, mode: Mode, mutation: &str) -> HookObservation {
    HookObservation {
        entity,
        client,
        mode,
        mutation: mutation.into(),
        items: 1,
        hooks_received: vec![],
        delay: None,
        save_latency: Some(10.0),
        save_latency_no_hook: None,
        updated_at_bumped: None,
        missed: false,
        out_of_order: false,
        repeats: 1,
        note: None,
    }
}

#[test]
fn read_new_returns_only_lines_after_the_offset() {
    let dir = std::env::temp_dir().join(format!("spike-hooklog-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let log = dir.join("hooks.jsonl");
    fs::write(
        &log,
        "{\"type\":\"Tag.Update.Post\",\"id\":1,\"receivedMs\":1.0}\n",
    )
    .unwrap();
    let offset = fs::metadata(&log).unwrap().len();
    let mut text = fs::read_to_string(&log).unwrap();
    text.push_str("{\"type\":\"Scene.Update.Post\",\"id\":5,\"receivedMs\":2.0,\"inputFields\":[\"custom_fields\"]}\n");
    fs::write(&log, text).unwrap();
    let new = read_new(&log, offset);
    assert_eq!(new.len(), 1);
    assert_eq!(new[0].hook_type, "Scene.Update.Post");
    assert_eq!(new[0].id, Some(5));
    assert_eq!(new[0].input_fields, vec!["custom_fields"]);
}

#[test]
fn scripted_rows_replace_and_manual_rows_add() {
    let out = std::env::temp_dir().join(format!("spike-hooktable-{}", std::process::id()));
    let _ = fs::remove_dir_all(&out);
    let a = row(
        Entity::Scene,
        SavingClient::WebUi,
        Mode::Scripted,
        "sceneUpdate",
    );
    merge_table(&out, std::slice::from_ref(&a), &[]).unwrap();
    let mut a2 = a.clone();
    a2.save_latency = Some(20.0);
    let all = merge_table(&out, &[a2], &[]).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].save_latency, Some(20.0));
    let m = row(
        Entity::Scene,
        SavingClient::WebUi,
        Mode::Manual,
        "Scene.Update.Post",
    );
    merge_table(&out, std::slice::from_ref(&m), &[]).unwrap();
    let all = merge_table(&out, &[m], &[]).unwrap();
    assert_eq!(all.len(), 3);
}

#[test]
fn coverage_lists_every_missing_pair() {
    let rows = vec![row(
        Entity::Tag,
        SavingClient::Viewer,
        Mode::Scripted,
        "(none)",
    )];
    let missing = missing_pairs(&rows, Mode::Scripted);
    assert_eq!(missing.len(), 11);
    assert!(!missing.contains(&(Entity::Tag, SavingClient::Viewer)));
    assert_eq!(missing_pairs(&rows, Mode::Manual).len(), 12);
}
