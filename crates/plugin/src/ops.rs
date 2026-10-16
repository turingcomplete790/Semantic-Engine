//! Operation and task registry. Operations are added by the user-story phases.

use crate::envelope::Envelope;
use crate::input::PluginInput;
use crate::log::Logger;
use engine::ErrorCode;

type Handler = fn(&PluginInput, &mut Logger) -> Envelope;

/// Every operation and task the plugin answers, by `args.op` name.
pub const REGISTRY: &[(&str, Handler)] = &[];

pub fn names() -> Vec<&'static str> {
    REGISTRY.iter().map(|(n, _)| *n).collect()
}

pub fn dispatch(input: &PluginInput, logger: &mut Logger) -> Envelope {
    let op = input.op().unwrap_or("");
    match REGISTRY.iter().find(|(n, _)| *n == op) {
        Some((_, handler)) => handler(input, logger),
        None => Envelope::error(
            op,
            ErrorCode::UnknownOperation,
            if op.is_empty() {
                "no operation given; set args.op"
            } else {
                "unknown operation"
            },
            Some(serde_json::json!({ "known": names() })),
        ),
    }
}
