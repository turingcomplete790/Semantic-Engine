//! `spike-bench`, the measurement harness (contracts/spike-bench.md).
//!
//! Connection rules: `STASH_TEST_URL` and `STASH_TEST_API_KEY` come from the environment; any
//! URL other than the test server is refused unless `--allow-host <url>` names it, and the
//! subcommands that write to Stash are refused on any other host even then. The API key never
//! reaches stdout, stderr, or `results/`.

pub mod build;
pub mod call_cost;
pub mod catch_up;
pub mod clients;
pub mod concurrency;
pub mod hooks;
pub mod payload;
pub mod seed;
pub mod snapshot;

use crate::measure::{redact, Server};
use crate::plugin_ops::PLUGIN_ID;
use serde_json::{Map, Value};
use stash_adapter::Client;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

/// The only server the spike writes to (spec FR-012).
pub const TEST_URL: &str = "http://localhost:9998";
pub const DEFAULT_OUT: &str = "specs/003-engine-design-spike/results";

/// Every subcommand in the contract.
pub const SUBCOMMANDS: [&str; 11] = [
    "seed",
    "cleanup",
    "synth",
    "build",
    "call-cost",
    "concurrency",
    "hooks",
    "catch-up",
    "payload",
    "snapshot",
    "report",
];
/// Subcommands that write to Stash: test server only, whatever `--allow-host` says.
pub const WRITES: [&str; 3] = ["seed", "cleanup", "hooks"];
/// Subcommands that never talk to Stash.
pub const LOCAL: [&str; 2] = ["synth", "report"];

/// Exit codes (contracts/spike-bench.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Ok = 0,
    RunFailed = 1,
    Usage = 2,
    Refused = 3,
    Unreachable = 4,
}

#[derive(Debug, Clone)]
pub struct Global {
    pub runs: Option<usize>,
    pub scale: String,
    pub out: PathBuf,
    pub allow_host: Option<String>,
}

/// One harness run: the subcommand, its own arguments, and (unless local) the connection.
pub struct Bench<'a> {
    pub sub: String,
    pub global: Global,
    pub args: Vec<String>,
    pub url: String,
    pub client: Option<Client>,
    pub server: Server,
    pub plugins_dir: Option<PathBuf>,
    secrets: Vec<String>,
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
}

/// A failed subcommand: its exit code and a message (credentials removed before printing).
#[derive(Debug)]
pub struct Failure {
    pub exit: Exit,
    pub message: String,
}

impl Failure {
    pub fn new(exit: Exit, message: impl Into<String>) -> Failure {
        Failure {
            exit,
            message: message.into(),
        }
    }
}

impl From<stash_adapter::Error> for Failure {
    fn from(e: stash_adapter::Error) -> Self {
        let exit = match e {
            stash_adapter::Error::Unreachable(_) => Exit::Unreachable,
            _ => Exit::RunFailed,
        };
        Failure::new(exit, e.to_string())
    }
}

pub type Outcome = Result<(), Failure>;

/// The result of one `runPluginOperation` call through Stash.
#[derive(Debug, Clone)]
pub struct OpCall {
    /// The spike plugin's envelope (`ok`, `op`, `result` or `error`).
    pub envelope: Value,
    /// Client-side wall time in milliseconds, request to response.
    pub wall_ms: f64,
}

impl OpCall {
    pub fn ok(&self) -> bool {
        self.envelope["ok"] == Value::Bool(true)
    }
    pub fn result(&self) -> &Value {
        &self.envelope["result"]
    }
}

const USAGE: &str = "usage: spike-bench <subcommand> [--runs N] [--scale S] [--out DIR] [--allow-host URL] [options]

subcommands: seed, cleanup, synth, build, call-cost, concurrency, hooks, catch-up, payload,
             snapshot, report (contracts/spike-bench.md)

environment: STASH_TEST_URL, STASH_TEST_API_KEY (writes only to http://localhost:9998)";

impl Bench<'_> {
    pub fn say(&mut self, msg: impl AsRef<str>) {
        let line = self.redact(msg.as_ref());
        let _ = writeln!(self.out, "{line}");
    }

    pub fn warn(&mut self, msg: impl AsRef<str>) {
        let line = self.redact(msg.as_ref());
        let _ = writeln!(self.err, "{line}");
    }

    pub fn redact(&self, text: &str) -> String {
        let s: Vec<&str> = self.secrets.iter().map(String::as_str).collect();
        redact(text, &s)
    }

    pub fn secrets(&self) -> Vec<&str> {
        self.secrets.iter().map(String::as_str).collect()
    }

    /// The connected client; local subcommands have none.
    pub fn client(&self) -> Result<&Client, Failure> {
        self.client
            .as_ref()
            .ok_or_else(|| Failure::new(Exit::Usage, "this subcommand has no Stash connection"))
    }

    /// The value after `--name`, if given.
    pub fn opt(&self, name: &str) -> Option<&str> {
        self.args
            .iter()
            .position(|a| a == name)
            .and_then(|i| self.args.get(i + 1))
            .map(String::as_str)
    }

    pub fn flag(&self, name: &str) -> bool {
        self.args.iter().any(|a| a == name)
    }

    pub fn opt_u64(&self, name: &str) -> Result<Option<u64>, Failure> {
        self.opt(name)
            .map(|v| {
                v.parse().map_err(|_| {
                    Failure::new(Exit::Usage, format!("{name} needs a number, got {v:?}"))
                })
            })
            .transpose()
    }

    /// The command line, for result records (never contains the key: it is not an argument).
    pub fn command_line(&self) -> String {
        let mut parts = vec!["spike-bench".to_string(), self.sub.clone()];
        parts.extend(self.args.iter().cloned());
        if let Some(r) = self.global.runs {
            parts.push(format!("--runs {r}"));
        }
        parts.push(format!("--scale {}", self.global.scale));
        self.redact(&parts.join(" "))
    }

    /// Runs one spike plugin operation through `runPluginOperation` and times it.
    pub fn call_op(&self, op: &str, args: Map<String, Value>) -> Result<OpCall, Failure> {
        call_op(self.client()?, op, args)
    }

    /// The spike plugin's folder in the test instance (`STASH_TEST_PLUGINS_DIR`).
    pub fn plugin_dir(&self) -> Result<PathBuf, Failure> {
        self.plugins_dir
            .as_ref()
            .map(|d| d.join(PLUGIN_ID))
            .ok_or_else(|| Failure::new(Exit::Usage, "STASH_TEST_PLUGINS_DIR is not set"))
    }

    /// `<plugin folder>/store`, where the spike plugin keeps its stores.
    pub fn store_root(&self) -> Result<PathBuf, Failure> {
        Ok(self.plugin_dir()?.join("store"))
    }

    /// Writes a measurement under `--out` and prints where.
    pub fn record(&mut self, m: &crate::measure::Measurement) -> Outcome {
        let secrets = self.secrets.clone();
        let s: Vec<&str> = secrets.iter().map(String::as_str).collect();
        let path = crate::measure::write_result(&self.global.out, m, &s)
            .map_err(|e| Failure::new(Exit::RunFailed, format!("{}: {e}", m.id)))?;
        self.say(format!(
            "{}: median {:.2}, p95 {:.2}, runs {} -> {}",
            m.id,
            m.median,
            m.p95,
            m.runs,
            path.display()
        ));
        Ok(())
    }
}

/// Runs one spike plugin operation through `runPluginOperation` and times it (usable from
/// threads that share the client).
pub fn call_op(client: &Client, op: &str, mut args: Map<String, Value>) -> Result<OpCall, Failure> {
    args.insert("op".into(), Value::String(op.into()));
    let start = Instant::now();
    let output = client.run_plugin_operation(PLUGIN_ID, args)?;
    let wall_ms = start.elapsed().as_secs_f64() * 1000.0;
    if output.get("ok").is_none() {
        return Err(Failure::new(
            Exit::RunFailed,
            format!("the spike plugin returned no envelope: {output}"),
        ));
    }
    Ok(OpCall {
        envelope: output,
        wall_ms,
    })
}

/// Arguments for a plugin call, from `(name, value)` pairs.
pub fn args(pairs: &[(&str, Value)]) -> Map<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

/// `{ runs, median, p95, min, max }` of a list of values.
pub fn summary(samples: &[f64]) -> Value {
    let m = crate::measure::Measurement::new(
        "summary",
        crate::measure::Question::Process,
        crate::measure::Kind::Observation,
        "",
        Server::local(),
        crate::measure::Library::test(0, 0, 0, 0, 0),
    )
    .with_samples(samples.to_vec());
    serde_json::json!({ "runs": m.runs, "median": m.median, "p95": m.p95, "min": m.min, "max": m.max })
}

fn normalize(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

fn parse(argv: &[String]) -> Result<(String, Global, Vec<String>), String> {
    let mut it = argv.iter();
    let sub = it.next().ok_or("no subcommand given")?.clone();
    if !SUBCOMMANDS.contains(&sub.as_str()) {
        return Err(format!("unknown subcommand {sub:?}"));
    }
    let mut g = Global {
        runs: None,
        scale: "test".into(),
        out: PathBuf::from(DEFAULT_OUT),
        allow_host: None,
    };
    let mut rest = Vec::new();
    while let Some(a) = it.next() {
        let mut value = |name: &str| it.next().cloned().ok_or(format!("{name} needs a value"));
        match a.as_str() {
            "--runs" => {
                let v = value("--runs")?;
                g.runs = Some(
                    v.parse()
                        .map_err(|_| format!("--runs needs a number, got {v:?}"))?,
                );
            }
            "--scale" => {
                let v = value("--scale")?;
                if crate::synth::parse_scale(&v).is_none() {
                    return Err(format!(
                        "--scale must be test, 50k, 100k, 250k, or a scene count; got {v:?}"
                    ));
                }
                g.scale = v;
            }
            "--out" => g.out = PathBuf::from(value("--out")?),
            "--allow-host" => g.allow_host = Some(normalize(&value("--allow-host")?)),
            _ => rest.push(a.clone()),
        }
    }
    Ok((sub, g, rest))
}

/// The whole harness, testable: arguments (without the program name), an environment lookup,
/// and the two output streams. Returns the exit code.
pub fn main_with(
    argv: &[String],
    env: &dyn Fn(&str) -> Option<String>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    if argv.first().is_some_and(|a| a == "-h" || a == "--help") {
        let _ = writeln!(out, "{USAGE}");
        return Exit::Ok as i32;
    }
    let (sub, global, args) = match parse(argv) {
        Ok(p) => p,
        Err(e) => {
            let _ = writeln!(err, "error: {e}\n\n{USAGE}");
            return Exit::Usage as i32;
        }
    };

    let local = LOCAL.contains(&sub.as_str());
    let key = env("STASH_TEST_API_KEY").unwrap_or_default();
    let secrets: Vec<String> = [key.clone()]
        .into_iter()
        .filter(|k| !k.is_empty())
        .collect();
    let fail = |err: &mut dyn Write, f: Failure| {
        let line = redact(
            &f.message,
            &secrets.iter().map(String::as_str).collect::<Vec<_>>(),
        );
        let _ = writeln!(err, "error: {line}");
        f.exit as i32
    };

    let mut url = String::new();
    let mut client = None;
    let mut server = Server::local();
    if !local {
        let Some(raw_url) = env("STASH_TEST_URL").filter(|u| !u.trim().is_empty()) else {
            return fail(err, Failure::new(Exit::Usage, "STASH_TEST_URL is not set"));
        };
        if key.is_empty() {
            return fail(
                err,
                Failure::new(Exit::Usage, "STASH_TEST_API_KEY is not set"),
            );
        }
        url = normalize(&raw_url);
        if url != TEST_URL {
            if WRITES.contains(&sub.as_str()) {
                return fail(
                    err,
                    Failure::new(
                        Exit::Refused,
                        format!(
                            "{sub} writes to Stash and runs only against {TEST_URL}, not {url}"
                        ),
                    ),
                );
            }
            if global.allow_host.as_deref() != Some(url.as_str()) {
                return fail(
                    err,
                    Failure::new(
                        Exit::Refused,
                        format!("refusing {url}: not the test server {TEST_URL} (pass --allow-host {url} to read from it)"),
                    ),
                );
            }
        }
        let c = match Client::with_api_key(&url, &key) {
            Ok(c) => c,
            Err(e) => return fail(err, Failure::from(e)),
        };
        match c.stash_version() {
            Ok(v) => {
                server = Server {
                    stash_version: v,
                    host: url.clone(),
                }
            }
            Err(e) => return fail(err, Failure::from(e)),
        }
        client = Some(c);
    }

    let mut bench = Bench {
        sub: sub.clone(),
        global,
        args,
        url,
        client,
        server,
        plugins_dir: env("STASH_TEST_PLUGINS_DIR")
            .filter(|d| !d.is_empty())
            .map(PathBuf::from),
        secrets,
        out,
        err,
    };
    match dispatch(&mut bench) {
        Ok(()) => Exit::Ok as i32,
        Err(f) => {
            let line = bench.redact(&f.message);
            bench.warn(format!("error: {line}"));
            f.exit as i32
        }
    }
}

fn dispatch(b: &mut Bench) -> Outcome {
    match b.sub.as_str() {
        "seed" => seed::seed_command(b),
        "cleanup" => seed::cleanup_command(b),
        "synth" => build::synth_command(b),
        "build" => build::build_command(b),
        "call-cost" => call_cost::command(b),
        "concurrency" => concurrency::command(b),
        "hooks" => hooks::command(b),
        "catch-up" => catch_up::command(b),
        "payload" => payload::command(b),
        "snapshot" => snapshot::command(b),
        other => Err(Failure::new(
            Exit::Usage,
            format!("{other} is not implemented in this build yet"),
        )),
    }
}
