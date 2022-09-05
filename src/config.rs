use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod, Runtime};
use rs_pixel::{ConfigBuilder, RsPixel};
use std::{env, str::FromStr, time::Duration};
use tokio_postgres::NoTls;
use twilight_model::id::{marker::GuildMarker, Id};

pub struct Config {
    pub bot_token: String,
    pub database: Pool,
    pub guild_id: Id<GuildMarker>,
    pub hypixel_api: RsPixel,
}

fn get_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("Unable to find {} environment variable", name))
}

impl Config {
    pub async fn load_or_panic() -> Self {
        let _ = dotenv::dotenv();

        let bot_token = get_env("BOT_TOKEN");
        let postgres_url = get_env("POSTGRES_URL");
        let guild_id = Id::from_str(&get_env("GUILD_ID")).unwrap();
        let api_key = get_env("API_KEY");

        let database = Pool::builder(Manager::from_config(
            postgres_url.parse::<tokio_postgres::Config>().unwrap(),
            NoTls,
            ManagerConfig {
                recycling_method: RecyclingMethod::Fast,
            },
        ))
        .max_size(16)
        .runtime(Runtime::Tokio1)
        .build()
        .unwrap();
        println!("Connected to database");

        let http_client = surf::Config::new()
            .set_timeout(Some(Duration::from_secs(15)))
            .set_max_connections_per_host(70)
            .try_into()
            .unwrap();

        let hypixel_api = RsPixel::from_config(
            &api_key,
            ConfigBuilder::default().client(http_client).into(),
        )
        .await
        .unwrap();
        println!("Initialized Hypixel API instance");

        Config {
            bot_token,
            database,
            guild_id,
            hypixel_api,
        }
    }
}
