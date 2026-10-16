mod common;
use stash_adapter::{Client, ServerConnection};

fn client(port: u16) -> Client {
    Client::from_server_connection(&ServerConnection {
        scheme: "http".into(),
        host: "127.0.0.1".into(),
        port,
        session_cookie: None,
        dir: String::new(),
        plugin_dir: String::new(),
    })
    .unwrap()
}

#[test]
fn reads_this_plugins_settings_only() {
    let (port, rx) = common::fake_stash(1, |_| {
        r#"{"data":{"configuration":{"plugins":{"semantic-engine":{"other":"x","serverId":"abc"}}}}}"#
            .to_string()
    });
    let settings = client(port).plugin_settings("semantic-engine").unwrap();
    assert_eq!(settings["other"], "x");
    assert_eq!(settings["serverId"], "abc");
    let body: serde_json::Value = serde_json::from_str(&rx.recv().unwrap()).unwrap();
    let q = body["query"].as_str().unwrap();
    assert!(
        q.contains("configuration") && q.contains("plugins(include:"),
        "{q}"
    );
    assert_eq!(
        body["variables"]["include"],
        serde_json::json!(["semantic-engine"])
    );
}

#[test]
fn missing_plugin_settings_read_as_empty() {
    let (port, _rx) = common::fake_stash(1, |_| {
        r#"{"data":{"configuration":{"plugins":{}}}}"#.to_string()
    });
    assert!(client(port)
        .plugin_settings("semantic-engine")
        .unwrap()
        .is_empty());
}

#[test]
fn writing_a_setting_merges_with_existing_settings() {
    // configurePlugin replaces the whole map (research R4), so the write must carry every key.
    let (port, rx) = common::fake_stash(2, |body| {
        common::echo_configure(body).unwrap_or_else(|| {
            r#"{"data":{"configuration":{"plugins":{"semantic-engine":{"other":"x"}}}}}"#
                .to_string()
        })
    });
    let stored = client(port)
        .set_plugin_setting(
            "semantic-engine",
            "serverId",
            serde_json::json!("new-value"),
        )
        .unwrap();
    assert_eq!(stored["other"], "x");
    assert_eq!(stored["serverId"], "new-value");
    let _read = rx.recv().unwrap();
    let write: serde_json::Value = serde_json::from_str(&rx.recv().unwrap()).unwrap();
    assert!(write["query"].as_str().unwrap().contains("configurePlugin"));
    assert_eq!(write["variables"]["pluginId"], "semantic-engine");
    assert_eq!(
        write["variables"]["input"],
        serde_json::json!({"other": "x", "serverId": "new-value"})
    );
}
