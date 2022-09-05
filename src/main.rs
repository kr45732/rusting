use bot::{config::Config, statics::SELF_USER_ID};
use futures::stream::StreamExt;
use rs_pixel::util::generic_json::Property;
use serde::Deserialize;
use std::{
    collections::HashMap,
    error::Error,
    str::FromStr,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::{Mutex, MutexGuard};
use twilight_gateway::{Cluster, Event};
use twilight_http::Client as HttpClient;
use twilight_model::{
    application::{
        command::{ChoiceCommandOptionData, CommandOption},
        interaction::{application_command::CommandOptionValue, InteractionData},
    },
    gateway::Intents,
    http::interaction::{InteractionResponse, InteractionResponseType},
    id::Id,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load_or_panic().await;

    let pool = config.database.get().await?;
    pool.simple_query(
        "CREATE TABLE IF NOT EXISTS linked_accounts (
            uuid TEXT PRIMARY KEY,
            username TEXT UNIQUE,
            discord TEXT UNIQUE,
            last_updated BIGINT
        )",
    )
    .await?;
    pool.simple_query(
        "CREATE TABLE IF NOT EXISTS config (
            id serial NOT NULL PRIMARY KEY,
            config json NOT NULL
        )",
    )
    .await?;

    let (cluster, mut events) = Cluster::new(
        config.bot_token.clone(),
        Intents::GUILD_MESSAGES.union(Intents::MESSAGE_CONTENT),
    )
    .await?;
    let cluster = Arc::new(cluster);
    let cluster_spawn = Arc::clone(&cluster);
    tokio::spawn(async move {
        cluster_spawn.up().await;
    });

    let http = Arc::new(HttpClient::new(config.bot_token.clone()));

    let _ = SELF_USER_ID.lock().await.insert(
        http.current_user_application()
            .exec()
            .await?
            .model()
            .await?
            .id,
    );

    let mut verify_command_opt = ChoiceCommandOptionData::default();
    verify_command_opt.name = "player".to_string();
    verify_command_opt.description = "Your in-game username".to_string();
    verify_command_opt.required = true;
    let _ = http
        .interaction(SELF_USER_ID.lock().await.unwrap())
        .create_guild_command(config.guild_id)
        .chat_input("verify", "Link your Hypixel account")?
        .command_options(&[CommandOption::String(verify_command_opt)])?
        .exec()
        .await;

    // settings set verified_role <@role>
    // settings add guild <@role>
    let mut settings_command_opt = ChoiceCommandOptionData::default();
    settings_command_opt.name = "command".to_string();
    settings_command_opt.description = "Command".to_string();
    settings_command_opt.required = true;
    let _ = http
        .interaction(SELF_USER_ID.lock().await.unwrap())
        .create_guild_command(config.guild_id)
        .chat_input("settings", "Configure the bot's settings")?
        .command_options(&[CommandOption::String(settings_command_opt)])?
        .exec()
        .await;

    // let cache = InMemoryCache::builder()
    //     .resource_types(ResourceType::MESSAGE)
    //     .build();

    let config_clone = Arc::new(Mutex::new(config));

    while let Some((shard_id, event)) = events.next().await {
        // cache.update(&event);

        tokio::spawn(handle_event(
            shard_id,
            event,
            Arc::clone(&http),
            Arc::clone(&config_clone),
        ));
    }

    Ok(())
}

async fn handle_event(
    shard_id: u64,
    event: Event,
    http: Arc<HttpClient>,
    config: Arc<Mutex<Config>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match event {
        Event::ShardConnected(_) => {
            println!("Connected on shard {shard_id}");
        }
        Event::InteractionCreate(interaction) => {
            if let InteractionData::ApplicationCommand(interaction_data) =
                interaction.data.as_ref().unwrap()
            {
                let _ = http
                    .interaction(SELF_USER_ID.lock().await.unwrap())
                    .create_response(
                        interaction.id,
                        &interaction.token,
                        &InteractionResponse {
                            kind: InteractionResponseType::DeferredChannelMessageWithSource,
                            data: None,
                        },
                    )
                    .exec()
                    .await?;

                let content = match interaction_data.name.as_str() {
                    "verify" => {
                        let mut player = String::new();
                        for opt in &interaction_data.options {
                            if opt.name == "player" {
                                if let CommandOptionValue::String(opt_str) = &opt.value {
                                    player = opt_str.to_string();
                                }
                            }
                        }

                        let mut config = config.lock().await;

                        let discord_info = get_discord_info(&mut config, player).await;
                        if let Some(err) = discord_info.error {
                            err
                        } else {
                            let user = interaction.member.as_ref().unwrap().user.as_ref().unwrap();
                            let user_tag = format!("{}#{}", user.name, user.discriminator());
                            let api_discord_tag = discord_info.discord.unwrap();

                            if api_discord_tag != user_tag {
                                format!(
                                    "Your Discord tag is {} but the in-game Discord tag is {}",
                                    user_tag, api_discord_tag
                                )
                            } else {
                                let user_id = user.id.to_string();
                                let username = discord_info.username.unwrap();
                                let uuid = discord_info.uuid.unwrap();

                                let pool = config.database.get().await?;
                                let _ = pool.query("DELETE FROM linked_account WHERE discord = $1 OR username = $2 or uuid = $3", &[&user_id, &username, &uuid] ).await;
                                let db_res = pool.query("INSERT INTO linked_account (last_updated, discord, username, uuid) VALUES ($1, $2, $3, $4)", &[&get_timestamp_millis(), &user_id, &username, &uuid] ).await;

                                if db_res.is_ok() {
                                    let server_config_vec =
                                        pool.query("SELECT * FROM config", &[]).await?;
                                    let server_config: ServerConfig = serde_json::from_str(
                                        server_config_vec.first().unwrap().get("config"),
                                    )?;

                                    http.add_guild_member_role(
                                        interaction.guild_id.unwrap(),
                                        user.id,
                                        Id::from_str(&server_config.verified_role)?,
                                    )
                                    .exec()
                                    .await?;

                                    format!("Successfully linked {} to {}", user_tag, username)
                                } else {
                                    "Error inserting into database".to_string()
                                }
                            }
                        }
                    }
                    _ => "Unknown Command".to_string(),
                };

                let _ = http
                    .interaction(SELF_USER_ID.lock().await.unwrap())
                    .create_followup(&interaction.token)
                    .content(&content)?
                    .exec()
                    .await?;
            }
        }
        _ => {}
    }

    Ok(())
}

pub async fn get_discord_info(config: &mut MutexGuard<'_, Config>, player: String) -> DiscordInfo {
    match config.hypixel_api.username_to_uuid(&player).await {
        Ok(uuid_res) => match config.hypixel_api.get_player_by_uuid(&uuid_res.uuid).await {
            Ok(player_res) => match player_res.get_string_property("socialMedia.links.DISCORD") {
                Some(discord_tag) => DiscordInfo {
                    username: Some(uuid_res.username),
                    uuid: Some(uuid_res.uuid),
                    discord: Some(discord_tag),
                    error: None,
                },
                None => {
                    DiscordInfo::from_err(format!("{} is not linked on Hypixel", uuid_res.username))
                }
            },
            Err(err) => DiscordInfo::from_err(err.to_string()),
        },
        Err(err) => DiscordInfo::from_err(err.to_string()),
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

pub fn get_timestamp_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

#[derive(Deserialize)]
pub struct ServerConfig {
    pub verified_role: String,
    pub guild_roles: HashMap<String, String>,
}
