mod auth;
mod config;
mod errors;
mod serve;
mod schema; // Add this line
mod state;
mod utils;
mod world;

use crate::config::{Config, Directories};
use crate::state::AppState;
use axum::{routing::get, Router};
use base64::{engine::general_purpose, Engine as _};
use clap::Parser;
use std::net::SocketAddr;
use tokio::sync::broadcast;
use tower_cookies::{CookieManagerLayer, Key};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::pg::PgConnection;

#[derive(Parser, Debug)]
#[command(name = "thunderforge")]
#[command(about = "A virtual tabletop for the modern era.")]
struct Cli {
    #[arg(
        short,
        long,
        default_value = "127.0.0.1",
        help = "IP address to bind the server to"
    )]
    ip_address: String,
    #[arg(short, long, default_value_t = 30000, help = "Port to bind the server to")]
    port: u16,
    #[arg(
        short,
        long,
        help = "Where do you want ThunderForgeVTT to store data?"
    )]
    data_path: Option<String>,
    #[arg(
        short,
        long,
        default_value = "redis://127.0.0.1/",
        help = "What redis url would you like to connect to?"
    )]
    redis_url: String,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let mut config = load_config();
    if let Some(data_path) = cli.data_path {
        config.data_path = data_path;
    }

    let directories = Directories::from(String::from(&config.data_path));
    directories.create_if_not_present();

    let (world_event_sender, _) = broadcast::channel(1024);

    let key = Key::from(
        &general_purpose::STANDARD
            .decode(&config.secret)
            .unwrap(),
    );

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set in .env");
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let db_pool = Pool::builder()
        .build(manager)
        .expect("Failed to create DB pool.");

    let app_state = AppState {
        config,
        directories: directories.clone(),
        world_event_sender,
        key,
        db_pool,
    };

    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        .merge(auth::router())
        .merge(world::router())
        .merge(serve::router(&directories))
        .fallback(errors::handler_404)
        .with_state(app_state)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .layer(CookieManagerLayer::new());

    let addr = SocketAddr::new(cli.ip_address.parse().unwrap(), cli.port);
    tracing::debug!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn load_config() -> Config {
    let dir = std::env::current_dir().unwrap();
    let current_dir = dir.as_path();
    let config = &current_dir.join("config.json");
    if config.exists() {
        let data = std::fs::read_to_string(&config).unwrap();
        serde_json::from_str(&data).unwrap()
    } else {
        Config::default()
    }
}
