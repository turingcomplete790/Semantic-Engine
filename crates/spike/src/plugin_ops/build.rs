//! `buildFromStash`: fetch the library's semantic data with bounded, flag-filtered queries and
//! build a published store from it (spec FR-011).
//!
//! The fetch is `stash_adapter::fixtures`' capture: tags where a flag is not null, scenes and
//! images where a semantic custom field is not null, and the performers they reference (one
//! request by ids). No per-item requests. Arguments: `store` (default `stash`).

use super::{fail, Ctx};
use crate::store::{self, LockMode};
use crate::synth::{self, class_for_flag, SERVER_ID};
use engine::ErrorCode;
use oxigraph::model::Quad;
use plugin::envelope::Envelope;
use plugin::log::Logger;
use semantic_vocabulary::iri::ItemIris;
use serde_json::{json, Map, Value};
use std::time::{Duration, Instant};

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// A custom field holding JSON text, parsed; `Null` when absent or malformed.
fn json_field(cf: &Map<String, Value>, key: &str) -> Value {
    cf.get(key)
        .and_then(Value::as_str)
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(Value::Null)
}

fn ok_id(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Quads for one scene or image's assignments (`roleTagAssignments`, `actionAssignments`,
/// `pluggableAssignments`), keyed by performer id as the plugin stores them.
fn media_quads(
    iris: &ItemIris,
    media: &str,
    kind: &str,
    cf: &Map<String, Value>,
    out: &mut Vec<Quad>,
) {
    out.push(synth::typed(media, kind));
    let mut performers = std::collections::BTreeSet::new();
    for (key, inst) in [
        ("roleTagAssignments", "role"),
        ("pluggableAssignments", "pluggable"),
    ] {
        if let Value::Object(by_perf) = json_field(cf, key) {
            for (perf, tags) in by_perf {
                for t in tags
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                {
                    if ok_id(&perf) && ok_id(t) {
                        performers.insert(perf.clone());
                        synth::instance(out, iris, media, &perf, inst, t, None);
                    }
                }
            }
        }
    }
    if let Value::Object(by_perf) = json_field(cf, "actionAssignments") {
        for (perf, list) in by_perf {
            for a in list.as_array().into_iter().flatten() {
                let Some(t) = a["actionTagId"].as_str() else {
                    continue;
                };
                if ok_id(&perf) && ok_id(t) {
                    performers.insert(perf.clone());
                    let var = a["variations"][0].as_str();
                    synth::instance(out, iris, media, &perf, "action", t, var);
                }
            }
        }
    }
    for p in performers {
        out.push(synth::link(media, "hasPerformer", &iris.performer(&p)));
    }
}

pub fn op(ctx: &Ctx, _: &mut Logger) -> Envelope {
    let name = ctx.arg_str("store").unwrap_or("stash");
    let t0 = Instant::now();
    let client = match stash_adapter::Client::from_server_connection(&ctx.input.server_connection) {
        Ok(c) => c,
        Err(e) => return fail("buildFromStash", ErrorCode::StashUnreachable, e),
    };
    let capture = match client.capture_semantic_fixtures() {
        Ok(c) => c,
        Err(e) => return fail("buildFromStash", ErrorCode::StashUnreachable, e),
    };
    let fetch_ms = ms(t0.elapsed());

    let t1 = Instant::now();
    let iris = ItemIris::new(SERVER_ID).expect("valid example server id");
    let mut quads = Vec::new();
    for (id, row) in &capture.tags {
        let tag = iris.tag(id);
        if let Some(name) = row["name"].as_str() {
            quads.push(synth::label(&tag, name));
        }
        if let Value::Object(cf) = &row["custom_fields"] {
            for (flag, v) in cf {
                let set = v.as_i64().is_some_and(|n| n != 0) || v.as_bool() == Some(true);
                if let (true, Some(class)) = (set, class_for_flag(flag)) {
                    quads.push(synth::typed(&tag, class));
                }
            }
        }
    }
    for (id, row) in &capture.performers {
        let p = iris.performer(id);
        quads.push(synth::typed(&p, "Performer"));
        if let Some(name) = row["name"].as_str() {
            quads.push(synth::label(&p, name));
        }
    }
    let empty = Map::new();
    for (kind, items) in [("Scene", &capture.scenes), ("Image", &capture.images)] {
        for (id, row) in items {
            let media = if kind == "Scene" {
                iris.scene(id)
            } else {
                iris.image(id)
            };
            let cf = row["custom_fields"].as_object().unwrap_or(&empty);
            media_quads(&iris, &media, kind, cf, &mut quads);
        }
    }
    let triples = quads.len();

    let dir = ctx.store_dir.join(name);
    let _ = std::fs::remove_dir_all(&dir);
    let w = match store::open_writer(&dir, LockMode::Wait(Duration::from_secs(30))) {
        Ok(w) => w,
        Err(e) => return fail("buildFromStash", ErrorCode::Internal, e),
    };
    if let Err(e) = w.store().bulk_loader().load_quads(quads) {
        return fail(
            "buildFromStash",
            ErrorCode::Internal,
            format!("bulk load: {e}"),
        );
    }
    let generation = match store::publish(&w) {
        Ok(g) => g,
        Err(e) => return fail("buildFromStash", ErrorCode::Internal, e),
    };
    let build_ms = ms(t1.elapsed());
    let items =
        capture.tags.len() + capture.performers.len() + capture.scenes.len() + capture.images.len();
    Envelope::ok(
        "buildFromStash",
        json!({
            "store": name,
            "items": items,
            "counts": { "tags": capture.tags.len(), "performers": capture.performers.len(), "scenes": capture.scenes.len(), "images": capture.images.len() },
            "requests": capture.queries.len(),
            "triples": triples,
            "generation": generation,
            "timings": { "fetch": fetch_ms, "build": build_ms, "total": ms(t0.elapsed()) },
        }),
    )
}
