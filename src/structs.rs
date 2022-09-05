use deadpool_postgres::Object;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize, Serialize)]
pub struct ServerConfig {
    #[serde(default = "Default::default")]
    pub verified_role: String,
    #[serde(default = "Default::default")]
    pub guild_roles: HashMap<String, String>,
}

impl ServerConfig {
    pub async fn from_db(pool: &Object) -> Self {
        let server_config_vec = pool
            .query("SELECT * FROM config LIMIT 1", &[])
            .await
            .unwrap();

        serde_json::from_value(server_config_vec.first().unwrap().get("config")).unwrap()
    }

    pub async fn update_db(&self, pool: &Object) {
        pool
            .query("INSERT INTO config (id, config) VALUES(1, $1) ON CONFLICT (id) DO UPDATE SET config = EXCLUDED.config", &[&serde_json::to_value(self).unwrap()])
            .await.unwrap();
    }
}

pub struct DiscordInfo {
    pub username: Option<String>,
    pub uuid: Option<String>,
    pub discord: Option<String>,
    pub error: Option<String>,
}

impl DiscordInfo {
    pub fn from_err(err: String) -> Self {
        Self {
            username: None,
            uuid: None,
            discord: None,
            error: Some(err),
        }
    }
}
