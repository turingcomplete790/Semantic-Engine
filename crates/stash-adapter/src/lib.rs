//! The only code that talks to Stash (constitution Principle V). Every operation is a Cynic
//! typed operation checked against the pinned schema `schemas/stash-v0.31.1.graphql`
//! (constitution v1.1.0).

pub mod queries;

use cynic::http::ReqwestBlockingExt;
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

#[derive(Debug, Error)]
pub enum Error {
    #[error("Stash is unreachable: {0}")]
    Unreachable(String),
    #[error("Stash returned an error: {0}")]
    Graphql(String),
    #[error("could not build the HTTP client: {0}")]
    Setup(String),
}

pub struct Client {
    http: reqwest::blocking::Client,
    endpoint: String,
}

impl Client {
    pub fn from_server_connection(conn: &ServerConnection) -> Result<Client, Error> {
        // TLS uses rustls with the ring provider; installing it twice is harmless.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(c) = &conn.session_cookie {
            let value = format!("{}={}", c.name, c.value);
            let mut hv = reqwest::header::HeaderValue::from_str(&value)
                .map_err(|_| Error::Setup("session cookie is not a valid header value".into()))?;
            hv.set_sensitive(true);
            headers.insert(reqwest::header::COOKIE, hv);
        }
        let http = reqwest::blocking::Client::builder()
            .default_headers(headers)
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| Error::Setup(e.to_string()))?;
        Ok(Client {
            http,
            endpoint: conn.endpoint(),
        })
    }

    pub(crate) fn run<Q, V>(&self, op: cynic::Operation<Q, V>) -> Result<Q, Error>
    where
        Q: serde::de::DeserializeOwned + 'static,
        V: serde::Serialize,
    {
        let resp = self
            .http
            .post(&self.endpoint)
            .run_graphql(op)
            .map_err(|e| match e {
                cynic::http::CynicReqwestError::ReqwestError(e) => {
                    Error::Unreachable(e.to_string())
                }
                other => Error::Graphql(other.to_string()),
            })?;
        if let Some(errs) = resp.errors.filter(|e| !e.is_empty()) {
            let msg: Vec<String> = errs.into_iter().map(|e| e.message).collect();
            return Err(Error::Graphql(msg.join("; ")));
        }
        resp.data
            .ok_or_else(|| Error::Graphql("response had no data".into()))
    }

    /// The server's version string, for example `v0.31.1`.
    pub fn stash_version(&self) -> Result<Option<String>, Error> {
        use cynic::QueryBuilder;
        Ok(self.run(queries::VersionQuery::build(()))?.version.version)
    }
}
