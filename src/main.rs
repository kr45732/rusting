use bot::{
    config::Config,
    structs::{CommandOptionBuilder, ServerConfig},
    utils::{default_embed, get_discord_info, get_timestamp_millis, SELF_USER_ID},
};
use futures::stream::StreamExt;
use std::fmt::Write;
use std::{error::Error, str::FromStr, sync::Arc};
use tokio::sync::Mutex;
use twilight_gateway::{Cluster, Event};
use twilight_http::Client as HttpClient;
use twilight_model::{
    application::{
        command::CommandOption,
        interaction::{
            application_command::{CommandData, CommandOptionValue},
            InteractionData,
        },
    },
    gateway::{payload::incoming::InteractionCreate, Intents},
    http::interaction::{InteractionResponse, InteractionResponseType},
    id::Id,
};
use twilight_util::builder::embed::EmbedFieldBuilder;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load_or_panic().await;
    config.initialize_database().await?;

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

    register_commands(&config, &http).await?;

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

async fn register_commands(
    config: &Config,
    http: &Arc<twilight_http::client::Client>,
) -> anyhow::Result<()> {
    let self_user_id = SELF_USER_ID.lock().await.unwrap();

    let _ = http
        .interaction(self_user_id)
        .create_guild_command(config.guild_id)
        .chat_input("verify", "Link your Hypixel account")?
        .command_options(&[CommandOption::String(
            CommandOptionBuilder::new("player", "Your in-game username")
                .set_required(true)
                .into(),
        )])?
        .exec()
        .await;

    // settings view
    // settings verified_role <@role>
    // settings guild_role <guild> <@role>
    // settings reqs remove <guild> type
    // settings reqs set <guild> <type> <amount>
    let _ = http
        .interaction(self_user_id)
        .create_guild_command(config.guild_id)
        .chat_input("settings", "View or configure the bot's settings")?
        .command_options(&[CommandOption::String(
            CommandOptionBuilder::new("command", "Subcommand to execute")
                .set_required(true)
                .into(),
        )])?
        .exec()
        .await;

    let _ = http
        .interaction(self_user_id)
        .create_guild_command(config.guild_id)
        .chat_input("user", "See what account a user is linked to")?
        .command_options(&[CommandOption::User(
            CommandOptionBuilder::new("user", "Discord user")
                .set_required(true)
                .into(),
        )])?
        .exec()
        .await;

    let _ = http
        .interaction(self_user_id)
        .create_guild_command(config.guild_id)
        .chat_input("reqs", "Check if a player meets the requirements")?
        .command_options(&[
            CommandOption::User(
                CommandOptionBuilder::new("player", "Player username")
                    .set_required(true)
                    .into(),
            ),
            CommandOption::User(
                CommandOptionBuilder::new("profile", "Profile name")
                    .set_required(true)
                    .into(),
            ),
        ])?
        .exec()
        .await;

    let _ = http
        .interaction(self_user_id)
        .create_guild_command(config.guild_id)
        .chat_input("help", "Display the help menu")?
        .exec()
        .await;

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

                let result = match interaction_data.name.as_str() {
                    "verify" => {
                        handle_verify_command(&http, config, &interaction, interaction_data).await
                    }
                    "settings" => {
                        handle_settings_command(&http, config, &interaction, interaction_data).await
                    }
                    "user" => {
                        handle_user_command(&http, config, &interaction, interaction_data).await
                    }
                    "help" => {
                        handle_help_command(&http, config, &interaction, interaction_data).await
                    }
                    "reqs" => {
                        handle_reqs_command(&http, config, &interaction, interaction_data).await
                    }
                    _ => {
                        handle_unknown_command(&http, config, &interaction, interaction_data).await
                    }
                };

                if let Err(err) = result {
                    let _ = http
                        .interaction(SELF_USER_ID.lock().await.unwrap())
                        .create_followup(&interaction.token)
                        .embeds(&[default_embed("Error").description(err.to_string()).build()])?
                        .exec()
                        .await?;
                }
            }
        }
        _ => {}
    }

    Ok(())
}

async fn handle_unknown_command(
    http: &Arc<HttpClient>,
    config: Arc<Mutex<Config>>,
    interaction: &Box<InteractionCreate>,
    interaction_data: &Box<CommandData>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let _ = http
        .interaction(SELF_USER_ID.lock().await.unwrap())
        .create_followup(&interaction.token)
        .embeds(&[default_embed("Error")
            .description("Unknown Command")
            .build()])?
        .exec()
        .await?;

    Ok(())
}

async fn handle_reqs_command(
    http: &Arc<HttpClient>,
    config: Arc<Mutex<Config>>,
    interaction: &Box<InteractionCreate>,
    interaction_data: &Box<CommandData>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut player = String::new();
    for opt in &interaction_data.options {
        if opt.name == "player" {
            if let CommandOptionValue::String(opt_str) = &opt.value {
                player = opt_str.to_string();
            }
        }
    }

    let mut profile = String::new();
    for opt in &interaction_data.options {
        if opt.name == "profile" {
            if let CommandOptionValue::String(opt_str) = &opt.value {
                profile = opt_str.to_string();
            }
        }
    }

    let mut config = config.lock().await;

    let uuid_response = config.hypixel_api.username_to_uuid(&player).await?;
    let sb_response = config
        .hypixel_api
        .get_skyblock_profiles_by_uuid(&uuid_response.uuid)
        .await?;
    let sb_profile = if profile.is_empty() {
        sb_response.get_last_played_profile()
    } else {
        sb_response.get_profile_by_name(&profile)
    }
    .ok_or("No profile found")?;

    let mut eb = default_embed("Requirement Checker");

    let pool = config.database.get().await?;
    let server_config = ServerConfig::read_config(&pool).await;

    for req in server_config.guild_reqs {
        let cur_reqs = req.1;
        eb = eb.field(EmbedFieldBuilder::new(req.0, "").build());
    }

    let _ = http
        .interaction(SELF_USER_ID.lock().await.unwrap())
        .create_followup(&interaction.token)
        .embeds(&[eb.build()])?
        .exec()
        .await?;

    Ok(())
}

async fn handle_verify_command(
    http: &Arc<HttpClient>,
    config: Arc<Mutex<Config>>,
    interaction: &Box<InteractionCreate>,
    interaction_data: &Box<CommandData>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let eb;

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
        eb = default_embed("Verify").description(err);
    } else {
        let user = interaction.member.as_ref().unwrap().user.as_ref().unwrap();
        let user_tag = format!("{}#{}", user.name, user.discriminator());
        let api_discord_tag = discord_info.discord.unwrap();

        if api_discord_tag != user_tag {
            eb = default_embed("Verify").description(format!(
                "Your Discord tag (`{}`) does not match the in-game Discord tag (`{}`)",
                user_tag, api_discord_tag
            ));
        } else {
            let user_id = user.id.to_string();
            let username = discord_info.username.unwrap();
            let uuid = discord_info.uuid.unwrap();

            let pool = config.database.get().await?;
            let _ = pool
                .query(
                    "DELETE FROM linked_accounts WHERE discord = $1 OR username = $2 or uuid = $3",
                    &[&user_id, &username, &uuid],
                )
                .await;
            let db_res = pool.query("INSERT INTO linked_accounts (last_updated, discord, username, uuid) VALUES ($1, $2, $3, $4)", &[&get_timestamp_millis(), &user_id, &username, &uuid] ).await;

            if db_res.is_ok() {
                let server_config = ServerConfig::read_config(&pool).await;

                http.add_guild_member_role(
                    interaction.guild_id.unwrap(),
                    user.id,
                    Id::from_str(&server_config.verified_role)?,
                )
                .exec()
                .await?;

                if let Ok(guild_res) = config.hypixel_api.get_guild_by_player(&uuid).await {
                    if let Some(player_guild) = guild_res.guild {
                        if let Some(guild_member_role) =
                            server_config.guild_roles.get(&player_guild.id)
                        {
                            http.add_guild_member_role(
                                interaction.guild_id.unwrap(),
                                user.id,
                                Id::from_str(guild_member_role)?,
                            )
                            .exec()
                            .await?;
                        }
                    }
                }

                eb = default_embed("Verify")
                    .description(format!("Successfully linked {} to {}", user_tag, username));
            } else {
                eb = default_embed("Verify").description("Error inserting into database");
            }
        }
    }

    let _ = http
        .interaction(SELF_USER_ID.lock().await.unwrap())
        .create_followup(&interaction.token)
        .embeds(&[eb.build()])?
        .exec()
        .await?;

    Ok(())
}

async fn handle_settings_command(
    http: &Arc<HttpClient>,
    config: Arc<Mutex<Config>>,
    interaction: &Box<InteractionCreate>,
    interaction_data: &Box<CommandData>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let eb;

    let mut command = String::new();
    for opt in &interaction_data.options {
        if opt.name == "command" {
            if let CommandOptionValue::String(opt_str) = &opt.value {
                command = opt_str.to_lowercase();
            }
        }
    }

    let mut config = config.lock().await;
    let pool = config.database.get().await?;
    let mut server_config = ServerConfig::read_config(&pool).await;

    let cmd_args: Vec<_> = command.split(' ').collect();
    if cmd_args.len() == 2 && cmd_args.first().unwrap() == &"verified_role" {
        let verified_role =
            Id::from_str(&cmd_args.get(1).unwrap().replace("<@&", "").replace('>', ""))?;
        let mut found_role = false;

        let roles = http
            .roles(interaction.guild_id.unwrap())
            .exec()
            .await?
            .model()
            .await?;
        for role in roles {
            if role.id == verified_role {
                found_role = true;
                break;
            }
        }

        if found_role {
            server_config.verified_role = verified_role.to_string();
            server_config.write_config(&pool).await;
            eb = default_embed("Settings")
                .description(format!("Set verified role to <@&{}>", verified_role));
        } else {
            eb = default_embed("Settings")
                .description(format!("Invalid role: <@&{}>", verified_role));
        }
    } else if cmd_args.len() >= 3 && cmd_args.first().unwrap() == &"guild_role" {
        let guild_role_raw = cmd_args.last().unwrap();
        let guild_role = Id::from_str(&guild_role_raw.replace("<@&", "").replace('>', ""))?;
        let guild_name = command
            .split("guild_role ")
            .last()
            .unwrap()
            .split(guild_role_raw)
            .next()
            .unwrap()
            .trim();
        let mut found_role = false;

        for role in http
            .roles(interaction.guild_id.unwrap())
            .exec()
            .await?
            .model()
            .await?
        {
            if role.id == guild_role {
                found_role = true;
            }
        }

        if !found_role {
            eb = default_embed("Settings").description(format!("Invalid role: <@&{}>", guild_role));
        } else {
            let guild = config
                .hypixel_api
                .get_guild_by_name(guild_name)
                .await?
                .guild
                .ok_or("Invalid guild")?;
            server_config
                .guild_roles
                .insert(guild.id, guild_role.to_string());
            server_config.write_config(&pool).await;
            eb = default_embed("Settings").description(format!(
                "Set guild role for {} to <@&{}>",
                guild.name, guild_role
            ));
        }
    } else if cmd_args.len() == 1 && cmd_args.get(0).unwrap() == &"view" {
        let mut out = format!(
            "Verified Role: <@&{}>\nGuild Roles:",
            server_config.verified_role
        );
        for guild_role in server_config.guild_roles {
            let guild_res = config.hypixel_api.get_guild_by_id(&guild_role.0).await?;
            write!(
                out,
                "\n  • {}: <@&{}>",
                guild_res.guild.ok_or("Invalid guild")?.name,
                guild_role.1
            )?;
        }
        write!(out, "\nReqs:")?;
        for guild_req in server_config.guild_reqs {
            write!(
                out,
                "\n  • {}: slayer = {}, skills = {}, cata = {}, weight = {}",
                guild_req.0,
                guild_req.1.slayer,
                guild_req.1.skills,
                guild_req.1.catacombs,
                guild_req.1.weight
            )?;
        }
        eb = default_embed("Settings").description(out);
    } else if cmd_args.len() >= 3 && cmd_args.get(0).unwrap() == &"reqs" {
        let guild_name = cmd_args.get(2).unwrap();
        if cmd_args.get(1).unwrap() == &"clear" {
            server_config.guild_reqs.remove(&guild_name.to_string());
            server_config.write_config(&pool).await;
            eb = default_embed("Settings").description(format!("Cleared reqs for {}", guild_name));
        } else if cmd_args.get(1).unwrap() == &"set" && cmd_args.len() == 5 {
            let mut cur_reqs = server_config
                .guild_reqs
                .remove(&guild_name.to_string())
                .unwrap_or_default();

            let req_name = cmd_args.get(3).unwrap();
            let req_amt: i64 = cmd_args.get(4).unwrap().parse()?;

            let mut valid_req = true;
            match cmd_args.get(3).unwrap() {
                &"slayer" => cur_reqs.slayer = req_amt,
                &"skills" => cur_reqs.skills = req_amt,
                &"catacombs" => cur_reqs.catacombs = req_amt,
                &"weight" => cur_reqs.weight = req_amt,
                _ => valid_req = false,
            };

            if valid_req {
                server_config
                    .guild_reqs
                    .insert(guild_name.to_string(), cur_reqs);

                server_config.write_config(&pool).await;
                eb = default_embed("Settings").description(format!(
                    "Set {} req to {} for {}",
                    req_name, req_amt, guild_name
                ));
            } else {
                eb = default_embed("Settings").description("Invalid requirement type")
            }
        } else {
            eb = default_embed("Settings").description("Invalid command");
        }
    } else {
        eb = default_embed("Settings").description("Invalid command");
    }

    let _ = http
        .interaction(SELF_USER_ID.lock().await.unwrap())
        .create_followup(&interaction.token)
        .embeds(&[eb.build()])?
        .exec()
        .await?;

    Ok(())
}

async fn handle_user_command(
    http: &Arc<HttpClient>,
    config: Arc<Mutex<Config>>,
    interaction: &Box<InteractionCreate>,
    interaction_data: &Box<CommandData>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut user = &Id::from_str("1")?;
    for opt in &interaction_data.options {
        if opt.name == "user" {
            if let CommandOptionValue::User(opt_user) = &opt.value {
                user = opt_user;
            }
        }
    }

    let config = config.lock().await;
    let pool = config.database.get().await?;
    let db_res_vec = pool
        .query(
            "SELECT * FROM linked_accounts WHERE discord = $1",
            &[&user.to_string()],
        )
        .await?;
    let db_res = db_res_vec.get(0).ok_or("User is not linked")?;

    let username: String = db_res.get("username");
    let uuid: String = db_res.get("uuid");

    let _ = http
        .interaction(SELF_USER_ID.lock().await.unwrap())
        .create_followup(&interaction.token)
        .embeds(&[default_embed("User Information")
            .description(format!(
                "<@{}> is linked to [{}](https://mine.ly/{})",
                user, username, uuid
            ))
            .build()])?
        .exec()
        .await?;

    Ok(())
}

async fn handle_help_command(
    http: &Arc<HttpClient>,
    config: Arc<Mutex<Config>>,
    interaction: &Box<InteractionCreate>,
    interaction_data: &Box<CommandData>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let _ = http
        .interaction(SELF_USER_ID.lock().await.unwrap())
        .create_followup(&interaction.token)
        .embeds(&[default_embed("Help")
            .description(
                "`/help`
                `/verify <player>`
                `/reqs <player> <profile>`
                `/user <@user>`
                `/settings view`
                `/settings verified_role <@role>`
                `/settings guild_role <guild> <@role>`
                `/settings reqs remove <guild> type`
                `/settings reqs set <guild> <type> <amount>`",
            )
            .build()])?
        .exec()
        .await?;

    Ok(())
}
