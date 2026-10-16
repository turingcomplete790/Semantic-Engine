use stash_adapter::{Client, ServerConnection, SessionCookie};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;

fn conn(host: &str, port: u16, cookie: Option<&str>) -> ServerConnection {
    ServerConnection {
        scheme: "http".into(),
        host: host.into(),
        port,
        session_cookie: cookie.map(|v| SessionCookie {
            name: "session".into(),
            value: v.into(),
        }),
        dir: String::new(),
        plugin_dir: String::new(),
    }
}

#[test]
fn endpoint_maps_wildcard_hosts_to_localhost() {
    assert_eq!(
        conn("0.0.0.0", 9999, None).endpoint(),
        "http://localhost:9999/graphql"
    );
    assert_eq!(
        conn("::", 9999, None).endpoint(),
        "http://localhost:9999/graphql"
    );
    assert_eq!(
        conn("stash.lan", 443, None).endpoint(),
        "http://stash.lan:443/graphql"
    );
    let mut c = conn("stash.lan", 443, None);
    c.scheme = "https".into();
    assert_eq!(c.endpoint(), "https://stash.lan:443/graphql");
}

/// One-request fake Stash: returns (request line, headers, body) and answers with `reply`.
fn fake_stash(reply: &'static str) -> (u16, mpsc::Receiver<(String, Vec<String>, String)>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut first = String::new();
        reader.read_line(&mut first).unwrap();
        let mut headers = Vec::new();
        let mut len = 0usize;
        loop {
            let mut h = String::new();
            reader.read_line(&mut h).unwrap();
            let h = h.trim_end().to_string();
            if h.is_empty() {
                break;
            }
            if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
                len = v.trim().parse().unwrap();
            }
            headers.push(h);
        }
        let mut body = vec![0; len];
        reader.read_exact(&mut body).unwrap();
        let mut s = stream;
        write!(
            s,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            reply.len(),
            reply
        )
        .unwrap();
        tx.send((first, headers, String::from_utf8(body).unwrap()))
            .unwrap();
    });
    (port, rx)
}

#[test]
fn sends_the_session_cookie_to_the_invoking_server_only() {
    let (port, rx) = fake_stash(r#"{"data":{"version":{"version":"v0.31.1"}}}"#);
    let client = Client::from_server_connection(&conn("127.0.0.1", port, Some("abc123"))).unwrap();
    let version = client.stash_version().unwrap();
    assert_eq!(version.as_deref(), Some("v0.31.1"));
    let (first, headers, body) = rx.recv().unwrap();
    assert!(first.starts_with("POST /graphql "), "{first}");
    assert!(
        headers
            .iter()
            .any(|h| h.eq_ignore_ascii_case("cookie: session=abc123")),
        "{headers:?}"
    );
    assert!(body.contains("version"), "{body}");
}

#[test]
fn unreachable_server_is_reported() {
    // Bind then drop to get a port nothing listens on.
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let client = Client::from_server_connection(&conn("127.0.0.1", port, None)).unwrap();
    let err = client.stash_version().unwrap_err();
    assert!(
        matches!(err, stash_adapter::Error::Unreachable(_)),
        "{err:?}"
    );
}
