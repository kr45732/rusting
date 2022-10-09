use lazy_static::lazy_static;
use rs_pixel::util::generic_json::Property;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, MutexGuard};
use twilight_model::id::{marker::ApplicationMarker, Id};
use twilight_util::builder::embed::EmbedBuilder;

use crate::{config::Config, structs::DiscordInfo};

lazy_static! {
    pub static ref SELF_USER_ID: Mutex<Option<Id<ApplicationMarker>>> = Mutex::new(None);
}

pub fn get_timestamp_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
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

pub fn default_embed(title: &str) -> EmbedBuilder {
    EmbedBuilder::new().title(title)
}
