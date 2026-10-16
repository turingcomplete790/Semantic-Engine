//! Plugin settings in Stash's configuration (research R4).
//!
//! Read with `configuration { plugins(include: [id]) }`; written with `configurePlugin`, which
//! replaces the plugin's whole settings map, so writes always merge with what is stored.

use crate::{schema, Client, Error, JsonMap, PluginConfigMap};
use serde_json::{Map, Value};

#[derive(cynic::QueryVariables, Debug)]
pub struct PluginSettingsVars {
    pub include: Option<Vec<cynic::Id>>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "PluginSettingsVars")]
pub struct PluginSettingsQuery {
    pub configuration: ConfigPlugins,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "ConfigResult", variables = "PluginSettingsVars")]
pub struct ConfigPlugins {
    #[arguments(include: $include)]
    pub plugins: PluginConfigMap,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct ConfigurePluginVars {
    pub plugin_id: cynic::Id,
    pub input: JsonMap,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "ConfigurePluginVars")]
pub struct ConfigurePlugin {
    #[arguments(plugin_id: $plugin_id, input: $input)]
    pub configure_plugin: JsonMap,
}

fn as_map(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

impl Client {
    /// This plugin's settings map; empty when nothing is stored yet.
    pub fn plugin_settings(&self, plugin_id: &str) -> Result<Map<String, Value>, Error> {
        use cynic::QueryBuilder;
        let op = PluginSettingsQuery::build(PluginSettingsVars {
            include: Some(vec![cynic::Id::new(plugin_id)]),
        });
        let all = as_map(self.run(op)?.configuration.plugins.0);
        Ok(all.get(plugin_id).cloned().map(as_map).unwrap_or_default())
    }

    /// Replaces this plugin's settings map and returns the map Stash stored.
    pub fn write_plugin_settings(
        &self,
        plugin_id: &str,
        settings: Map<String, Value>,
    ) -> Result<Map<String, Value>, Error> {
        use cynic::MutationBuilder;
        let op = ConfigurePlugin::build(ConfigurePluginVars {
            plugin_id: cynic::Id::new(plugin_id),
            input: JsonMap(Value::Object(settings)),
        });
        Ok(as_map(self.run(op)?.configure_plugin.0))
    }

    /// Sets one setting, keeping every other stored setting; returns the stored map.
    pub fn set_plugin_setting(
        &self,
        plugin_id: &str,
        key: &str,
        value: Value,
    ) -> Result<Map<String, Value>, Error> {
        let mut settings = self.plugin_settings(plugin_id)?;
        settings.insert(key.to_string(), value);
        self.write_plugin_settings(plugin_id, settings)
    }
}
