//! `catch-up`: what can find a change the hooks missed (spec FR-007, research R7, tasks T032).
//!
//! - `freshness-updated-at`: whether each scripted save in `hooks.json` bumped `updated_at`
//!   (custom-field-only changes and play activity included), and what one `updated_at > t`
//!   query per type costs.
//! - `freshness-id-diff`: deletions leave nothing to filter on, so they are found by listing
//!   every id of a type and comparing; this times that listing and extrapolates per id.

use super::hooks::TABLE;
use super::{summary, Bench, Outcome};
use crate::measure::{
    rfc3339_from_secs, stash_time, HookObservation, Kind as MKind, Library, Measurement, Mode,
    Question,
};
use crate::plugin_ops::hooks::epoch_ms;
use serde_json::{json, Value};
use stash_adapter::spike::{Kind, Page};
use std::time::Instant;

const KINDS: [(Kind, &str); 4] = [
    (Kind::Scene, "scenes"),
    (Kind::Image, "images"),
    (Kind::Performer, "performers"),
    (Kind::Tag, "tags"),
];
/// The owner's Production library on 2026-10-16 (scenes, with clips) and the image count the
/// viewer recorded (viewer specs/005 research); used only for the marked estimates.
const PRODUCTION_SCENES: f64 = 49_683.0;
const PRODUCTION_IMAGES: f64 = 29_057.0;

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

pub fn command(b: &mut Bench) -> Outcome {
    let runs = b.global.runs.unwrap_or(100);
    let per = runs.div_ceil(KINDS.len());
    let table: Vec<HookObservation> = std::fs::read_to_string(b.global.out.join(TABLE))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| serde_json::from_value(v["rows"].clone()).ok())
        .unwrap_or_default();
    let bumps: Vec<Value> = table
        .iter()
        .filter(|r| r.mode == Mode::Scripted && r.updated_at_bumped.is_some())
        .map(|r| json!({ "entity": r.entity, "client": r.client, "mutation": r.mutation, "items": r.items, "updatedAtBumped": r.updated_at_bumped, "note": r.note }))
        .collect();
    if bumps.is_empty() {
        b.warn(
            "no scripted rows in hooks.json yet; run `spike-bench hooks` first for the bump table",
        );
    }

    let client = b.client()?;
    let stats = client.stats()?;
    let offset = super::hooks::server_offset(client)?;
    let hour_ago = (epoch_ms() / 1000.0) as u64 - 3600;
    let since = stash_time(hour_ago, offset);
    // The same instant in UTC: Stash compares the text, so this finds the wrong set.
    let since_utc = rfc3339_from_secs(hour_ago);
    let utc_trap: Vec<Value> = KINDS
        .iter()
        .map(|(kind, name)| {
            let local = client
                .find_custom_fields(*kind, Some(&since), None, Page::first(1))
                .map(|r| r.0)
                .unwrap_or(-1);
            let utc = client
                .find_custom_fields(*kind, Some(&since_utc), None, Page::first(1))
                .map(|r| r.0)
                .unwrap_or(-1);
            json!({ "type": name, "localFormatCount": local, "utcFormatCount": utc })
        })
        .collect();
    let (mut all, mut by_type) = (Vec::new(), serde_json::Map::new());
    for (kind, name) in KINDS {
        let mut v = Vec::new();
        let mut count = 0;
        for _ in 0..per {
            let t = Instant::now();
            count = client
                .find_custom_fields(kind, Some(&since), None, Page::first(1))?
                .0;
            v.push(ms(t));
        }
        all.extend(&v);
        by_type.insert(
            name.into(),
            json!({ "summary": summary(&v), "changedInLastHour": count }),
        );
    }
    let lib = Library::test(
        stats.scene_count as u64,
        stats.image_count as u64,
        stats.performer_count as u64,
        stats.tag_count as u64,
        0,
    );
    let mut m = Measurement::new(
        "freshness-updated-at",
        Question::Freshness,
        MKind::Latency,
        &b.command_line(),
        b.server.clone(),
        lib.clone(),
    )
    .with_samples(all);
    m.variant = "updated-at".into();
    m.extra.insert(
        "query".into(),
        json!("find<Type>(filter: updated_at GREATER_THAN <one hour ago>, per_page 1): count only"),
    );
    m.extra.insert("byType".into(), Value::Object(by_type));
    m.extra.insert("bumpTable".into(), json!(bumps));
    m.extra.insert("thresholdFormat".into(), json!({ "local": since, "utc": since_utc, "serverOffsetSeconds": offset, "sameInstantCounts": utc_trap,
        "rule": "Stash stores updated_at as local-time text and compares filter values as text, to the second; send thresholds in that form, and never together with ids (the filter is then ignored)" }));
    b.record(&m)?;

    let client = b.client()?;
    let (mut all, mut by_type) = (Vec::new(), serde_json::Map::new());
    let mut scene_ms_per_id = 0.0;
    let mut image_ms_per_id = 0.0;
    for (kind, name) in KINDS {
        let mut v = Vec::new();
        let mut n = 0;
        for _ in 0..per {
            let t = Instant::now();
            n = client.ids_only(kind)?.len();
            v.push(ms(t));
        }
        let s = summary(&v);
        let per_id = s["median"].as_f64().unwrap_or(0.0) / n.max(1) as f64;
        match kind {
            Kind::Scene => scene_ms_per_id = per_id,
            Kind::Image => image_ms_per_id = per_id,
            _ => {}
        }
        all.extend(&v);
        by_type.insert(
            name.into(),
            json!({ "summary": s, "ids": n, "msPerId": per_id }),
        );
    }
    let mut m = Measurement::new(
        "freshness-id-diff",
        Question::Freshness,
        MKind::Latency,
        &b.command_line(),
        b.server.clone(),
        lib,
    )
    .with_samples(all);
    m.variant = "id-diff".into();
    m.extra.insert("byType".into(), Value::Object(by_type));
    m.extra.insert(
        "productionEstimate".into(),
        json!({
            "estimate": true,
            "basis": "median ms per id at the test library's size, times Production counts (49,683 scenes on 2026-10-16; 29,057 images from the viewer's 005 research)",
            "scenesMs": scene_ms_per_id * PRODUCTION_SCENES,
            "imagesMs": image_ms_per_id * PRODUCTION_IMAGES,
        }),
    );
    b.record(&m)
}
