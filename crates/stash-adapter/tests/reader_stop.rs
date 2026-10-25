//! Progress and stopping during a full read (spec 005 T013; FR-006, SC-009).

mod common;

use common::{client, library_handler, root_field, serve};
use serde_json::{json, Value};
use stash_adapter::reader::{LibraryReader, Phase, Progress, RECORD_BATCH};
use stash_adapter::Error;
use std::cell::{Cell, RefCell};

/// 600 scenes with a timeline: three record batches of scenes.
fn big_library() -> Value {
    let scenes: Vec<Value> = (1..=600)
        .map(|i| {
            json!({"id": i.to_string(), "performers": [], "tags": [],
                   "updated_at": "2026-10-24T10:00:00-06:00",
                   "custom_fields": {"timeline": "[]"}})
        })
        .collect();
    json!({"version": "v0.31.1", "scenes": scenes})
}

fn scene_batches(bodies: &[String]) -> usize {
    bodies
        .iter()
        .filter(|b| {
            let v: Value = serde_json::from_str(b).unwrap();
            root_field(b) == "findScenes"
                && v["variables"]
                    .as_object()
                    .is_some_and(|m| m.values().any(|x| x.is_array()))
        })
        .count()
}

#[test]
fn progress_is_reported_after_every_request_and_never_goes_back() {
    let (port, rx) = serve(library_handler(big_library()));
    let seen = RefCell::new(Vec::<Progress>::new());
    let read = client(port)
        .full_read(&|| false, &mut |p| seen.borrow_mut().push(p))
        .unwrap();
    assert_eq!(read.scenes.len(), 600);
    let seen = seen.into_inner();
    let bodies: Vec<String> = rx.try_iter().collect();
    // One report per request of the read (all but the version check), plus the final Done.
    assert_eq!(seen.len(), bodies.len() - 1 + 1);
    let shares: Vec<f64> = seen.iter().map(Progress::share).collect();
    assert!(shares.windows(2).all(|w| w[1] >= w[0]), "{shares:?}");
    assert_eq!(seen.last().unwrap().phase, Phase::Done);
    let records: Vec<&Progress> = seen.iter().filter(|p| p.phase == Phase::Records).collect();
    assert_eq!(records.len(), 600_usize.div_ceil(RECORD_BATCH));
    assert_eq!(records.last().unwrap().done, records.last().unwrap().total);
}

#[test]
fn a_stop_after_batch_two_sends_no_third_batch() {
    let (port, rx) = serve(library_handler(big_library()));
    let batches = Cell::new(0usize);
    let result = client(port).full_read(&|| batches.get() >= 2, &mut |p| {
        if p.phase == Phase::Records {
            batches.set(p.done);
        }
    });
    assert!(matches!(result, Err(Error::Stopped)), "{result:?}");
    let bodies: Vec<String> = rx.try_iter().collect();
    assert_eq!(scene_batches(&bodies), 2);
}

#[test]
fn a_stop_before_the_start_sends_nothing() {
    let (port, rx) = serve(library_handler(big_library()));
    let result = client(port).full_read(&|| true, &mut |_| {});
    assert!(matches!(result, Err(Error::Stopped)));
    let bodies: Vec<String> = rx.try_iter().collect();
    assert!(bodies.is_empty(), "{bodies:?}");
}
