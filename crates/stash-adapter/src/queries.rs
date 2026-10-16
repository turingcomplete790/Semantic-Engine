//! Typed read operations. Each selects only the fields its caller needs (Principle VII).

use crate::schema;

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
pub struct VersionQuery {
    pub version: Version,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct Version {
    pub version: Option<String>,
}
