use deadpool_postgres::Object;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use twilight_model::application::command::{BaseCommandOptionData, ChoiceCommandOptionData};

#[derive(Deserialize, Serialize)]
pub struct ServerConfig {
    #[serde(default = "Default::default")]
    pub verified_role: String,
    #[serde(default = "Default::default")]
    pub guild_roles: HashMap<String, String>,
    #[serde(default = "Default::default")]
    pub guild_reqs: HashMap<String, GuildReqs>,
}

#[derive(Deserialize, Serialize)]
pub struct GuildReqs {
    pub slayer: i64,
    pub skills: i64,
    pub catacombs: i64,
    pub weight: i64,
}

impl Default for GuildReqs {
    fn default() -> Self {
        GuildReqs {
            slayer: 0,
            skills: 0,
            catacombs: 0,
            weight: 0,
        }
    }
}

impl ServerConfig {
    pub async fn read_config(pool: &Object) -> Self {
        let server_config_vec = pool
            .query("SELECT * FROM config LIMIT 1", &[])
            .await
            .unwrap();

        serde_json::from_value(server_config_vec.first().unwrap().get("config")).unwrap()
    }

    pub async fn write_config(&self, pool: &Object) {
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

pub struct CommandOptionBuilder(ChoiceCommandOptionData);

impl CommandOptionBuilder {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self(ChoiceCommandOptionData {
            autocomplete: false,
            choices: Vec::new(),
            description: description.into(),
            description_localizations: None,
            max_length: None,
            min_length: None,
            name: name.into(),
            name_localizations: None,
            required: false,
        })
    }

    pub fn set_required(mut self, required: bool) -> Self {
        self.0.required = required;
        self
    }
}

impl Into<ChoiceCommandOptionData> for CommandOptionBuilder {
    fn into(self) -> ChoiceCommandOptionData {
        self.0
    }
}

impl Into<BaseCommandOptionData> for CommandOptionBuilder {
    fn into(self) -> BaseCommandOptionData {
        BaseCommandOptionData {
            description: self.0.description,
            description_localizations: self.0.description_localizations,
            name: self.0.name,
            name_localizations: self.0.name_localizations,
            required: self.0.required,
        }
    }
}
