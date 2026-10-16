//! Logging to Stash through stderr with Stash's level prefix: 0x01, a level character, 0x02,
//! then the message (research R5). Registered secrets are never written.

use std::io::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warning,
    Error,
    Progress,
}

impl Level {
    fn char(self) -> u8 {
        match self {
            Level::Trace => b't',
            Level::Debug => b'd',
            Level::Info => b'i',
            Level::Warning => b'w',
            Level::Error => b'e',
            Level::Progress => b'p',
        }
    }
}

pub fn format_line(level: Level, message: &str, secrets: &[&str]) -> Vec<u8> {
    let mut text = message.replace(['\n', '\r'], " ");
    for s in secrets.iter().filter(|s| !s.is_empty()) {
        text = text.replace(s, "<redacted>");
    }
    let mut out = vec![0x01, level.char(), 0x02];
    out.extend_from_slice(text.as_bytes());
    out.push(b'\n');
    out
}

/// Writes log lines to a sink (stderr in the plugin), redacting registered secrets.
pub struct Logger {
    sink: Box<dyn Write>,
    secrets: Vec<String>,
}

impl Logger {
    pub fn stderr() -> Logger {
        Logger {
            sink: Box::new(std::io::stderr()),
            secrets: Vec::new(),
        }
    }

    pub fn to(sink: Box<dyn Write>) -> Logger {
        Logger {
            sink,
            secrets: Vec::new(),
        }
    }

    pub fn add_secret(&mut self, secret: &str) {
        if !secret.is_empty() {
            self.secrets.push(secret.to_string());
        }
    }

    pub fn log(&mut self, level: Level, message: &str) {
        let secrets: Vec<&str> = self.secrets.iter().map(String::as_str).collect();
        let _ = self.sink.write_all(&format_line(level, message, &secrets));
        let _ = self.sink.flush();
    }

    pub fn info(&mut self, message: &str) {
        self.log(Level::Info, message)
    }

    pub fn error(&mut self, message: &str) {
        self.log(Level::Error, message)
    }
}
