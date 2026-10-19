//! Operations of the measurement-only plugin `semantic-engine-spike`
//! (contracts/spike-plugin.md). Replies use contract 1.0.0's envelope so the harness can reuse
//! its parser; they are not part of that contract.

pub mod build;
pub mod hooks;
pub mod query;
pub mod write;

use engine::ErrorCode;
use plugin::envelope::{stash_output, Envelope};
use plugin::input::{self, PluginInput};
use plugin::log::Logger;
use serde_json::{json, Value};
use std::path::PathBuf;

/// The spike plugin's id in Stash (its manifest file name).
pub const PLUGIN_ID: &str = "semantic-engine-spike";

/// What every operation gets: the parsed input and the plugin's folders.
pub struct Ctx<'a> {
    pub input: &'a PluginInput,
    /// `<PluginDir>`, where Stash installed the plugin.
    pub plugin_dir: PathBuf,
    /// `<PluginDir>/store`, the spike's stores and snapshots.
    pub store_dir: PathBuf,
}

impl Ctx<'_> {
    pub fn arg_str(&self, name: &str) -> Option<&str> {
        self.input.args.get(name).and_then(Value::as_str)
    }
    pub fn arg_u64(&self, name: &str) -> Option<u64> {
        let v = self.input.args.get(name)?;
        v.as_u64()
            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
    }
}

type Handler = fn(&Ctx, &mut Logger) -> Envelope;

/// Every operation the spike plugin answers, by `args.op`. User stories add theirs here.
pub const REGISTRY: &[(&str, Handler)] = &[
    ("ping", ping),
    ("query", query::op),
    ("write", write::op),
    ("buildFromStash", build::op),
    ("recordHook", hooks::op),
];

pub fn names() -> Vec<&'static str> {
    REGISTRY.iter().map(|(n, _)| *n).collect()
}

/// A failed operation with the message only.
pub fn fail(op: &str, code: ErrorCode, message: impl std::fmt::Display) -> Envelope {
    Envelope::error(op, code, &message.to_string(), None)
}

pub fn dispatch(ctx: &Ctx, logger: &mut Logger) -> Envelope {
    let op = ctx.input.op().unwrap_or("");
    match REGISTRY.iter().find(|(n, _)| *n == op) {
        Some((_, handler)) => handler(ctx, logger),
        None => Envelope::error(
            op,
            ErrorCode::UnknownOperation,
            if op.is_empty() {
                "no operation given; set args.op"
            } else {
                "unknown operation"
            },
            Some(json!({ "known": names() })),
        ),
    }
}

/// Runs one invocation: Stash's stdin in, Stash's output JSON out.
pub fn run(stdin: &[u8], logger: &mut Logger) -> Value {
    let env = match input::parse(stdin) {
        Ok(input) => {
            if let Some(c) = &input.server_connection.session_cookie {
                logger.add_secret(&c.value);
            }
            let plugin_dir = PathBuf::from(&input.server_connection.plugin_dir);
            if input.server_connection.plugin_dir.is_empty() {
                fail(
                    input.op().unwrap_or(""),
                    ErrorCode::InvalidInput,
                    "server_connection.PluginDir is empty; the spike stores its data there",
                )
            } else {
                let ctx = Ctx {
                    input: &input,
                    store_dir: plugin_dir.join("store"),
                    plugin_dir,
                };
                dispatch(&ctx, logger)
            }
        }
        Err(e) => fail(
            "",
            ErrorCode::InvalidInput,
            format!("could not read the plugin input from Stash: {e}"),
        ),
    };
    stash_output(env)
}

fn ping(_: &Ctx, _: &mut Logger) -> Envelope {
    Envelope::ok("ping", json!({ "pong": true, "plugin": PLUGIN_ID }))
}
