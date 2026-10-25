//! The only code that talks to Stash (constitution Principle V). Every operation is a Cynic
//! typed operation checked against the pinned schema `schemas/stash-v0.31.1.graphql`
//! (constitution v1.1.0).
//!
//! [`reader`] is the read path: the engine learns what a library holds only through its
//! `LibraryReader` (spec 005). [`relations`] keeps spec 004's reads and writes, built on the
//! reader where their requests are the same; [`plugin_config`] reads and writes the plugin's
//! settings. Reads are retried on transient failures; writes never are.

pub mod fixtures;
pub mod plugin_config;
pub mod queries;
pub mod reader;
pub mod relations;
#[cfg(feature = "spike")]
pub mod spike;
pub mod version;

use serde::Deserialize;
use std::time::Duration;
use thiserror::Error;

#[cynic::schema("stash")]
pub mod schema {}

/// Stash's JSON-valued scalars. Each needs its own Rust type for Cynic.
macro_rules! json_scalar {
    ($rust:ident, $gql:ident) => {
        #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
        #[serde(transparent)]
        pub struct $rust(pub serde_json::Value);
        cynic::impl_scalar!($rust, schema::$gql);
    };
}
json_scalar!(JsonMap, Map);
json_scalar!(JsonAny, Any);
json_scalar!(PluginConfigMap, PluginConfigMap);

/// The cookie Stash hands a plugin so it can call back as the invoking user.
#[derive(Debug, Clone, Deserialize)]
pub struct SessionCookie {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Value")]
    pub value: String,
}

/// Stash's `server_connection`, with Stash's Go field names (research R1).
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConnection {
    #[serde(rename = "Scheme")]
    pub scheme: String,
    #[serde(rename = "Host")]
    pub host: String,
    #[serde(rename = "Port")]
    pub port: u16,
    #[serde(rename = "SessionCookie", default)]
    pub session_cookie: Option<SessionCookie>,
    #[serde(rename = "Dir", default)]
    pub dir: String,
    #[serde(rename = "PluginDir", default)]
    pub plugin_dir: String,
}

impl ServerConnection {
    /// The GraphQL endpoint of the Stash server that started the plugin. A server listening on
    /// every interface is reached on `localhost`, as Stash's own plugin client does.
    pub fn endpoint(&self) -> String {
        let host = match self.host.as_str() {
            "" | "0.0.0.0" | "::" | "[::]" => "localhost",
            h => h,
        };
        format!("{}://{}:{}/graphql", self.scheme, host, self.port)
    }
}

/// What went wrong talking to Stash (spec 005 FR-016). No message carries a credential:
/// messages are built from the kind, the HTTP status and the transport's own message, which
/// names the URL but never a header (FR-019).
#[derive(Debug, Clone, Error)]
pub enum Error {
    #[error("Stash is unreachable: {0}")]
    Unreachable(String),
    #[error("Stash did not answer in time: {0}")]
    TimedOut(String),
    #[error("Stash refused the credentials")]
    NotAuthorized,
    /// The server is older than the oldest supported Stash, or reports no usable version.
    #[error("Stash {found} is not supported; the minimum is {minimum}")]
    UnsupportedVersion { found: String, minimum: String },
    #[error("Stash returned an error: {0}")]
    Graphql(String),
    #[error("Stash sent an unexpected response: {0}")]
    UnexpectedResponse(String),
    #[error("could not build the HTTP client: {0}")]
    Setup(String),
    /// A full read was asked to stop (spec 005 FR-006); no partial result is returned.
    #[error("the read was stopped")]
    Stopped,
}

impl Error {
    /// True for failures worth retrying: the server could not be reached or did not answer.
    pub fn is_transient(&self) -> bool {
        matches!(self, Error::Unreachable(_) | Error::TimedOut(_))
    }

    /// A short name of the kind, for messages that wrap it.
    pub fn kind(&self) -> &'static str {
        match self {
            Error::Unreachable(_) => "unreachable",
            Error::TimedOut(_) => "timed out",
            Error::NotAuthorized => "not authorized",
            Error::UnsupportedVersion { .. } => "unsupported version",
            Error::Graphql(_) => "error reported by Stash",
            Error::UnexpectedResponse(_) => "unexpected response",
            Error::Setup(_) => "setup",
            Error::Stopped => "stopped",
        }
    }
}

/// Installs the TLS crypto provider (rustls with ring). reqwest is built without a provider to
/// avoid aws-lc's C/CMake build, so one must be installed before any reqwest client is created.
/// Safe to call more than once.
pub fn install_tls_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// A blocking HTTP client builder for `endpoint`. Plain `http://` endpoints never use TLS, so
/// the platform certificate store is not loaded for them: loading it costs about 8 ms per
/// process, which every plugin call (hooks included) would otherwise pay (spec 004 research R6).
/// `https://` endpoints keep full certificate verification.
fn http_builder(endpoint: &str) -> reqwest::blocking::ClientBuilder {
    let b = reqwest::blocking::Client::builder();
    if endpoint.starts_with("http://") {
        b.tls_certs_only(Vec::new())
    } else {
        b
    }
}

pub struct Client {
    http: reqwest::blocking::Client,
    endpoint: String,
    headers: reqwest::header::HeaderMap,
    /// Set once the server's version has been checked (spec 005 FR-017).
    pub(crate) version_checked: std::sync::OnceLock<()>,
}

/// Waits before the retries of a read that failed transiently (spec 005 research R6).
const READ_RETRY_WAITS: [Duration; 2] = [Duration::from_millis(250), Duration::from_secs(1)];

fn build_http(
    endpoint: &str,
    headers: &reqwest::header::HeaderMap,
    connect: Duration,
    request: Duration,
) -> Result<reqwest::blocking::Client, Error> {
    http_builder(endpoint)
        .default_headers(headers.clone())
        .connect_timeout(connect)
        .timeout(request)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| Error::Setup(e.to_string()))
}

fn transport_error(e: reqwest::Error) -> Error {
    if e.is_timeout() {
        Error::TimedOut(e.to_string())
    } else {
        Error::Unreachable(e.to_string())
    }
}

impl Client {
    fn new(
        endpoint: String,
        headers: reqwest::header::HeaderMap,
        request_timeout: Duration,
    ) -> Result<Client, Error> {
        let http = build_http(&endpoint, &headers, Duration::from_secs(5), request_timeout)?;
        Ok(Client {
            http,
            endpoint,
            headers,
            version_checked: std::sync::OnceLock::new(),
        })
    }

    pub fn from_server_connection(conn: &ServerConnection) -> Result<Client, Error> {
        install_tls_provider();
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(c) = &conn.session_cookie {
            let value = format!("{}={}", c.name, c.value);
            let mut hv = reqwest::header::HeaderValue::from_str(&value)
                .map_err(|_| Error::Setup("session cookie is not a valid header value".into()))?;
            hv.set_sensitive(true);
            headers.insert(reqwest::header::COOKIE, hv);
        }
        Client::new(conn.endpoint(), headers, Duration::from_secs(60))
    }

    /// A client for tools run outside Stash (fixture capture): `base_url` is the server address,
    /// for example `http://localhost:9999`, and the API key is sent in the `ApiKey` header.
    pub fn with_api_key(base_url: &str, api_key: &str) -> Result<Client, Error> {
        install_tls_provider();
        let mut headers = reqwest::header::HeaderMap::new();
        let mut hv = reqwest::header::HeaderValue::from_str(api_key)
            .map_err(|_| Error::Setup("the API key is not a valid header value".into()))?;
        hv.set_sensitive(true);
        headers.insert("ApiKey", hv);
        let endpoint = format!("{}/graphql", base_url.trim_end_matches('/'));
        Client::new(endpoint, headers, Duration::from_secs(300))
    }

    /// The same client with other timeouts, for tests and the bench.
    pub fn with_timeouts(self, connect: Duration, request: Duration) -> Result<Client, Error> {
        let http = build_http(&self.endpoint, &self.headers, connect, request)?;
        Ok(Client { http, ..self })
    }

    /// Sends one request body and decodes the typed answer (spec 005 research R6). The status
    /// is checked before the body: 401 and 403 are rejected credentials; 502, 503 and 504 mean
    /// Stash is not reachable behind its proxy; any other non-2xx status is unexpected.
    fn send<Q: serde::de::DeserializeOwned + 'static>(&self, body: &[u8]) -> Result<Q, Error> {
        let resp = self
            .http
            .post(&self.endpoint)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body.to_vec())
            .send()
            .map_err(transport_error)?;
        let status = resp.status();
        match status.as_u16() {
            401 | 403 => return Err(Error::NotAuthorized),
            502..=504 => return Err(Error::Unreachable(format!("Stash answered HTTP {status}"))),
            s if !(200..300).contains(&s) => {
                return Err(Error::UnexpectedResponse(format!("HTTP {status}")))
            }
            _ => {}
        }
        let text = resp.text().map_err(transport_error)?;
        let parsed: cynic::GraphQlResponse<Q> = serde_json::from_str(&text).map_err(|e| {
            Error::UnexpectedResponse(format!("the response does not fit the operation: {e}"))
        })?;
        if let Some(errs) = parsed.errors.filter(|e| !e.is_empty()) {
            let msg: Vec<String> = errs.into_iter().map(|e| e.message).collect();
            return Err(Error::Graphql(msg.join("; ")));
        }
        parsed
            .data
            .ok_or_else(|| Error::UnexpectedResponse("the response had no data".into()))
    }

    fn body<Q, V: serde::Serialize>(op: &cynic::Operation<Q, V>) -> Result<Vec<u8>, Error> {
        serde_json::to_vec(op).map_err(|e| Error::Setup(format!("could not encode a request: {e}")))
    }

    /// Runs an operation once. Writes use this: a write may have landed, so it is never retried.
    pub(crate) fn run<Q, V>(&self, op: cynic::Operation<Q, V>) -> Result<Q, Error>
    where
        Q: serde::de::DeserializeOwned + 'static,
        V: serde::Serialize,
    {
        self.send(&Self::body(&op)?)
    }

    /// Runs a read, retrying transient failures at most twice (spec 005 FR-018).
    pub(crate) fn run_read<Q, V>(&self, op: cynic::Operation<Q, V>) -> Result<Q, Error>
    where
        Q: serde::de::DeserializeOwned + 'static,
        V: serde::Serialize,
    {
        let body = Self::body(&op)?;
        let mut waits = READ_RETRY_WAITS.iter();
        loop {
            match self.send(&body) {
                Err(e) if e.is_transient() => match waits.next() {
                    Some(w) => std::thread::sleep(*w),
                    None => return Err(e),
                },
                other => return other,
            }
        }
    }

    /// The server's version string, for example `v0.31.1`.
    pub fn stash_version(&self) -> Result<Option<String>, Error> {
        use cynic::QueryBuilder;
        Ok(self
            .run_read(queries::VersionQuery::build(()))?
            .version
            .version)
    }
}
