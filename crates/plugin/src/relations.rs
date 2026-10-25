//! Relation operations and tasks (spec 004; contracts/operations-1.1.md).

use crate::envelope::Envelope;
use crate::input::PluginInput;
use crate::log::Logger;
use engine::relations::{describe, Model, UnknownTag};
use engine::ErrorCode;
use serde_json::Value;

/// The error code for a failed Stash call (spec 005 contracts/reader-interface.md). The adapter's
/// message, which names the kind, goes with it; no new code is added, so contract 1.1.0 holds.
pub fn stash_error(e: &stash_adapter::Error) -> ErrorCode {
    use stash_adapter::Error as E;
    match e {
        E::Unreachable(_) | E::TimedOut(_) => ErrorCode::StashUnreachable,
        E::UnsupportedVersion { .. } => ErrorCode::StashTooOld,
        E::NotAuthorized | E::Graphql(_) | E::UnexpectedResponse(_) | E::Setup(_) | E::Stopped => {
            ErrorCode::Internal
        }
    }
}

/// Reads the relation definitions and builds the model, or the error envelope for `op`.
pub fn model(input: &PluginInput, op: &str, logger: &mut Logger) -> Result<Model, Box<Envelope>> {
    let fail = |e: stash_adapter::Error, logger: &mut Logger| {
        logger.error(&format!("{op}: {e}"));
        Box::new(Envelope::error(op, stash_error(&e), &e.to_string(), None))
    };
    let client = stash_adapter::Client::from_server_connection(&input.server_connection)
        .map_err(|e| fail(e, logger))?;
    let tags = client.relation_definitions().map_err(|e| fail(e, logger))?;
    Ok(Model::from_tags(&tags))
}

/// `relations`: roles and relations with their hierarchy, rules, and problems.
pub fn relations(input: &PluginInput, logger: &mut Logger) -> Envelope {
    const OP: &str = "relations";
    let tag = match input.args.get("tag") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.clone()),
        Some(Value::Number(n)) => Some(n.to_string()),
        Some(other) => {
            return Envelope::error(
                OP,
                ErrorCode::InvalidInput,
                &format!("tag must be a tag id, got {other}"),
                None,
            )
        }
    };
    let recursive = match input.args.get("recursive") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(other) => {
            return Envelope::error(
                OP,
                ErrorCode::InvalidInput,
                &format!("recursive must be true or false, got {other}"),
                None,
            )
        }
    };
    let model = match model(input, OP, logger) {
        Ok(m) => m,
        Err(env) => return *env,
    };
    match describe(&model, tag.as_deref(), recursive) {
        Ok(result) => Envelope::ok(OP, result),
        Err(UnknownTag(t)) => Envelope::error(
            OP,
            ErrorCode::InvalidInput,
            &format!("tag {t} is not a Role or Relation"),
            None,
        ),
    }
}

use crate::apply::{apply_journal, reapply, Report};
use crate::journal::Journal;
use crate::log::Level;
use crate::ops::PLUGIN_ID;
use serde_json::json;
use stash_adapter::relations::MediaKind;
use std::time::Instant;

/// The task the save hook starts (plugin manifest `tasks`).
pub const APPLY_TASK: &str = "Apply relations";

/// A string argument; numbers are accepted as their decimal text.
fn arg_text(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn journal(input: &PluginInput) -> Option<Journal> {
    let dir = &input.server_connection.plugin_dir;
    (!dir.is_empty()).then(|| Journal::new(dir))
}

/// The save hook (`Scene/Image.Create/Update.Post`): appends the saved item to the journal and,
/// for the first save of a burst, asks Stash to start "Apply relations". It does no evaluation
/// and no store work, because its time is added to the save (spec 004 FR-012; research R6). It
/// never fails the save: problems are logged and the result says what happened.
pub fn record_change(input: &PluginInput, logger: &mut Logger) -> Envelope {
    const OP: &str = "recordChange";
    let ctx = input.args.get("hookContext");
    let kind = ctx
        .and_then(|c| c.get("type"))
        .and_then(Value::as_str)
        .and_then(|t| t.split('.').next())
        .and_then(MediaKind::parse);
    let id = arg_text(ctx.and_then(|c| c.get("id")));
    let (Some(kind), Some(id), Some(journal)) = (kind, id, journal(input)) else {
        return Envelope::ok(OP, json!({ "recorded": false, "started": false }));
    };
    if let Err(e) = journal.append(kind, &id) {
        logger.error(&format!("{OP}: journal: {e}"));
        return Envelope::ok(OP, json!({ "recorded": false, "started": false }));
    }
    let started = match journal.mark_pending() {
        Ok(true) => {
            let started = stash_adapter::Client::from_server_connection(&input.server_connection)
                .and_then(|c| c.run_plugin_task(PLUGIN_ID, APPLY_TASK));
            match started {
                Ok(_) => true,
                Err(e) => {
                    logger.error(&format!("{OP}: could not start {APPLY_TASK}: {e}"));
                    let _ = journal.clear_pending();
                    false
                }
            }
        }
        Ok(false) => false,
        Err(e) => {
            logger.error(&format!("{OP}: marker: {e}"));
            false
        }
    };
    Envelope::ok(OP, json!({ "recorded": true, "started": started }))
}

fn report_result(op: &str, r: Report, started: Instant, logger: &mut Logger) -> Envelope {
    for n in &r.notes {
        logger.info(&format!("{op}: {n}"));
    }
    logger.info(&r.summary(op, started.elapsed().as_secs_f64()));
    Envelope::ok(
        op,
        json!({ "items": r.items, "changed": r.changed, "skipped": r.skipped, "unstable": r.unstable }),
    )
}

/// The "Apply relations" task (research R6, R7).
pub fn apply_relations(input: &PluginInput, logger: &mut Logger) -> Envelope {
    const OP: &str = "applyRelations";
    let started = Instant::now();
    let Some(journal) = journal(input) else {
        return Envelope::error(
            OP,
            ErrorCode::InvalidInput,
            "no plugin folder in the input",
            None,
        );
    };
    let client = match stash_adapter::Client::from_server_connection(&input.server_connection) {
        Ok(c) => c,
        Err(e) => return Envelope::error(OP, stash_error(&e), &e.to_string(), None),
    };
    let result = {
        let mut progress = |f: f64| logger.log(Level::Progress, &format!("{f:.3}"));
        apply_journal(&client, &journal, &mut progress)
    };
    match result {
        Ok(r) => report_result(OP, r, started, logger),
        Err(e) => {
            logger.error(&format!("{OP}: {e}"));
            Envelope::error(OP, stash_error(&e), &e.to_string(), None)
        }
    }
}

/// The "Re-apply relations" task (FR-011; research R9): `scope` (`all`, `scenes`, `images`) and
/// optional `ids` within it.
pub fn reapply_relations(input: &PluginInput, logger: &mut Logger) -> Envelope {
    const OP: &str = "reapplyRelations";
    let started = Instant::now();
    let kinds: Vec<MediaKind> = match input
        .args
        .get("scope")
        .and_then(Value::as_str)
        .unwrap_or("all")
    {
        "all" => vec![MediaKind::Scene, MediaKind::Image],
        "scenes" => vec![MediaKind::Scene],
        "images" => vec![MediaKind::Image],
        other => {
            return Envelope::error(
                OP,
                ErrorCode::InvalidInput,
                &format!("scope must be all, scenes, or images, got {other:?}"),
                None,
            )
        }
    };
    let ids: Option<Vec<String>> = match input.args.get("ids") {
        None | Some(Value::Null) => None,
        Some(Value::Array(xs)) => {
            let ids: Vec<String> = xs.iter().filter_map(|x| arg_text(Some(x))).collect();
            if ids.len() != xs.len() || kinds.len() != 1 {
                return Envelope::error(
                    OP,
                    ErrorCode::InvalidInput,
                    "ids must be item ids, with scope scenes or images",
                    None,
                );
            }
            Some(ids)
        }
        Some(_) => return Envelope::error(OP, ErrorCode::InvalidInput, "ids must be a list", None),
    };
    let client = match stash_adapter::Client::from_server_connection(&input.server_connection) {
        Ok(c) => c,
        Err(e) => return Envelope::error(OP, stash_error(&e), &e.to_string(), None),
    };
    let result = {
        let mut progress = |f: f64| logger.log(Level::Progress, &format!("{f:.3}"));
        reapply(&client, &kinds, ids.as_deref(), &mut progress)
    };
    match result {
        Ok(r) => report_result(OP, r, started, logger),
        Err(e) => {
            logger.error(&format!("{OP}: {e}"));
            Envelope::error(OP, stash_error(&e), &e.to_string(), None)
        }
    }
}

/// `explainRelation`: why one relation instance is on one scene or image (FR-014).
pub fn explain_relation(input: &PluginInput, logger: &mut Logger) -> Envelope {
    const OP: &str = "explainRelation";
    let mut args = Vec::new();
    for name in ["media", "id", "relationTagId", "a", "b"] {
        match arg_text(input.args.get(name)) {
            Some(v) => args.push(v),
            None => {
                return Envelope::error(
                    OP,
                    ErrorCode::InvalidInput,
                    &format!("{name} is required"),
                    None,
                )
            }
        }
    }
    let Some(kind) = MediaKind::parse(&args[0]) else {
        return Envelope::error(
            OP,
            ErrorCode::InvalidInput,
            "media must be scene or image",
            None,
        );
    };
    let model = match model(input, OP, logger) {
        Ok(m) => m,
        Err(env) => return *env,
    };
    let fail = |e: stash_adapter::Error| Envelope::error(OP, stash_error(&e), &e.to_string(), None);
    let client = match stash_adapter::Client::from_server_connection(&input.server_connection) {
        Ok(c) => c,
        Err(e) => return fail(e),
    };
    let item = match client.items_by_ids(kind, std::slice::from_ref(&args[1])) {
        Ok(items) => match items.into_iter().next() {
            Some(i) => i,
            None => {
                return Envelope::error(
                    OP,
                    ErrorCode::NotFound,
                    &format!("no {} {}", kind.as_str(), args[1]),
                    None,
                )
            }
        },
        Err(e) => return fail(e),
    };
    let labels = match client.performer_tags(&item.performers) {
        Ok(ps) => ps
            .into_iter()
            .map(|p| (p.id, p.tags.into_iter().collect()))
            .collect(),
        Err(e) => return fail(e),
    };
    match engine::relations::explain::explain(&item, &model, &labels, &args[2], &args[3], &args[4])
    {
        Ok(v) => Envelope::ok(OP, v),
        Err(e) => Envelope::error(OP, ErrorCode::Internal, &e.to_string(), None),
    }
}

/// `recommendations`: recommended vocabulary tags for one scene or image, by category (FR-018 to
/// FR-020). It only reads.
pub fn recommendations(input: &PluginInput, logger: &mut Logger) -> Envelope {
    const OP: &str = "recommendations";
    let (Some(media), Some(id)) = (
        arg_text(input.args.get("media")),
        arg_text(input.args.get("id")),
    ) else {
        return Envelope::error(
            OP,
            ErrorCode::InvalidInput,
            "media and id are required",
            None,
        );
    };
    let Some(kind) = MediaKind::parse(&media) else {
        return Envelope::error(
            OP,
            ErrorCode::InvalidInput,
            "media must be scene or image",
            None,
        );
    };
    let model = match model(input, OP, logger) {
        Ok(m) => m,
        Err(env) => return *env,
    };
    let native = stash_adapter::Client::from_server_connection(&input.server_connection)
        .and_then(|c| c.item_native_tags(kind, &id));
    match native {
        Ok(Some(tags)) => Envelope::ok(
            OP,
            serde_json::to_value(engine::recommend::recommend(&tags, &model.defs))
                .expect("serializes"),
        ),
        Ok(None) => Envelope::error(OP, ErrorCode::NotFound, &format!("no {media} {id}"), None),
        Err(e) => Envelope::error(OP, stash_error(&e), &e.to_string(), None),
    }
}
