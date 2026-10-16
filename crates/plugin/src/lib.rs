//! The Stash plugin: reads Stash's raw-interface input, dispatches one operation, and writes
//! the result envelope (contracts/operations.md).

pub mod envelope;
pub mod input;
pub mod log;
pub mod ops;

use std::io::Read;

/// Runs one plugin invocation: `stdin` bytes in, the full Stash output JSON out.
pub fn run(stdin: &[u8], logger: &mut log::Logger) -> serde_json::Value {
    let env = match input::parse(stdin) {
        Ok(input) => {
            if let Some(c) = &input.server_connection.session_cookie {
                logger.add_secret(&c.value);
            }
            ops::dispatch(&input, logger)
        }
        Err(e) => envelope::Envelope::error(
            "",
            engine::ErrorCode::InvalidInput,
            &format!("could not read the plugin input from Stash: {e}"),
            None,
        ),
    };
    envelope::stash_output(env)
}

/// Reads all of stdin; an unreadable stdin is treated as empty input.
pub fn read_stdin() -> Vec<u8> {
    let mut buf = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut buf);
    buf
}
