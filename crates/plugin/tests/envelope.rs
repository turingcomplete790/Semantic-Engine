use std::io::Write;
use std::process::{Command, Stdio};

fn run(stdin: &str) -> serde_json::Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_semantic-engine"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "plugin must exit 0 when it produces an envelope"
    );
    serde_json::from_slice(&out.stdout).expect("stdout must be the Stash output JSON")
}

const INPUT: &str = r#"{"server_connection":{"Scheme":"http","Host":"0.0.0.0","Port":9999,
  "SessionCookie":{"Name":"session","Value":"secret-cookie-value","Path":"/"},
  "Dir":"/stash","PluginDir":"/stash/plugins/semantic-engine"},
  "args":{"op":"nope"}}"#;

#[test]
fn parses_the_go_cased_stash_input() {
    let input = plugin::input::parse(INPUT.as_bytes()).unwrap();
    let c = &input.server_connection;
    assert_eq!(c.scheme, "http");
    assert_eq!(c.host, "0.0.0.0");
    assert_eq!(c.port, 9999);
    assert_eq!(
        c.session_cookie.as_ref().unwrap().value,
        "secret-cookie-value"
    );
    assert_eq!(c.plugin_dir, "/stash/plugins/semantic-engine");
    assert_eq!(input.args["op"], "nope");
}

#[test]
fn empty_stdin_is_invalid_input_inside_an_envelope() {
    let v = run("");
    assert!(v["error"].is_null());
    assert_eq!(v["output"]["ok"], false);
    assert_eq!(v["output"]["error"]["code"], "INVALID_INPUT");
    assert_eq!(v["output"]["contractVersion"], engine::CONTRACT_VERSION);
}

#[test]
fn non_json_and_missing_connection_are_invalid_input() {
    for s in ["not json", r#"{"args":{"op":"ping"}}"#] {
        let v = run(s);
        assert_eq!(v["output"]["error"]["code"], "INVALID_INPUT", "{s}");
        assert!(v["error"].is_null());
    }
}

#[test]
fn every_envelope_has_ok_op_and_contract_version() {
    let v = run(INPUT);
    let o = &v["output"];
    assert!(o["ok"].is_boolean());
    assert_eq!(o["op"], "nope");
    assert_eq!(o["contractVersion"], engine::CONTRACT_VERSION);
    assert!(
        !v.to_string().contains("secret-cookie-value"),
        "cookie must never be echoed"
    );
}
