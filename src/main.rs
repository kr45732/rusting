use futures::stream::StreamExt;
use std::{env, error::Error, str::FromStr, sync::Arc};
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{Cluster, Event};
use twilight_http::Client as HttpClient;
use twilight_model::{
    application::interaction::InteractionType,
    gateway::Intents,
    id::{marker::GuildMarker, Id},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let token =
        String::from("MTAxNDcwNDAyNDUxNzM0OTQ3OA.Gql-Bc.3datEF57DYt_Vswq7E6c6w51Sc8RBSfeclISBw");

    // Use intents to only receive guild message events.

    // A cluster is a manager for multiple shards that by default
    // creates as many shards as Discord recommends.
    let (cluster, mut events) = Cluster::new(
        token.to_owned(),
        Intents::GUILD_MESSAGES.union(Intents::MESSAGE_CONTENT),
    )
    .await?;
    let cluster = Arc::new(cluster);

    // Start up the cluster.
    let cluster_spawn = Arc::clone(&cluster);

    // Start all shards in the cluster in the background.
    tokio::spawn(async move {
        cluster_spawn.up().await;
    });

    // HTTP is separate from the gateway, so create a new client.
    let http = Arc::new(HttpClient::new(token));
    let id = http
        .current_user_application()
        .exec()
        .await
        .unwrap()
        .model()
        .await
        .unwrap()
        .id;
    http.interaction(id)
        .create_guild_command(Id::from_str("869217817680044042").unwrap())
        .chat_input("ping", "ping deez nuts")
        .unwrap()
        .exec()
        .await;

    // Since we only care about new messages, make the cache only
    // cache new messages.
    let cache = InMemoryCache::builder()
        .resource_types(ResourceType::MESSAGE)
        .build();

    // Process each event as they come in.
    while let Some((shard_id, event)) = events.next().await {
        // Update the cache with the event.
        cache.update(&event);

        tokio::spawn(handle_event(shard_id, event, Arc::clone(&http)));
    }

    Ok(())
}

use twilight_interactions::command::{CommandModel, CreateCommand, ResolvedUser};

#[derive(CommandModel, CreateCommand)]
#[command(name = "hello", desc = "Say hello to other members")]
struct HelloCommand {
    /// Message to send
    message: String,
    /// User to send the message to
    user: Option<ResolvedUser>,
}

async fn handle_event(
    shard_id: u64,
    event: Event,
    http: Arc<HttpClient>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match event {
        Event::MessageCreate(msg) if msg.content == "!ping" => {
            http.create_message(msg.channel_id)
                .content("Pong!")?
                .exec()
                .await?;
        }
        Event::ShardConnected(_) => {
            println!("Connected on shard {shard_id}");
        }
        Event::InteractionCreate(i) => {
            if i.kind == InteractionType::ApplicationCommand {
                InteractionResponse::
            //   let a = http.interaction(http
            //     .current_user_application()
            //     .exec()
            //     .await
            //     .unwrap()
            //     .model()
            //     .await
            //     .unwrap()
            //     .id).create_response(i.id, &i.token, InteractionResponse::)
            }
        }

        // Other events here...
        _ => {}
    }

    Ok(())
}
