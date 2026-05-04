mod adapters;
mod auth;
mod auth_middleware;
mod config;
mod db_types;
mod errors;
mod graphql;
mod models;
mod schema; // Add this line
mod serve;
mod state;
mod users;
mod utils;
mod world;

use crate::config::{Config, Directories};
use crate::graphql::{AppSchema, MutationRoot, QueryRoot, SubscriptionRoot}; // Added SubscriptionRoot
use crate::state::AppState;
use async_graphql::http::{ALL_WEBSOCKET_PROTOCOLS, GraphQLPlaygroundConfig, playground_source};
use async_graphql::{Data, Schema}; // Added Data
use async_graphql_axum::{GraphQLProtocol, GraphQLRequest, GraphQLResponse, GraphQLWebSocket}; // Added GraphQLWebSocket
use axum::{
    Extension, Router,
    extract::{State, WebSocketUpgrade},
    http::StatusCode,
    middleware::{from_fn, from_fn_with_state},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use base64::{Engine as _, engine::general_purpose};
use clap::Parser;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{RunQueryDsl, pg::PgConnection};
use std::net::SocketAddr;
use tokio::sync::broadcast;
use tower_cookies::{CookieManagerLayer, Key};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::{EnvFilter, Registry, layer::SubscriberExt, util::SubscriberInitExt};

async fn graphql_playground() -> impl IntoResponse {
    Html(playground_source(GraphQLPlaygroundConfig::new("/graphql")))
}

async fn graphql_handler(
    Extension(schema): Extension<AppSchema>,
    Extension(auth_user): Extension<auth_middleware::AuthenticatedUser>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    schema
        .execute(req.into_inner().data(auth_user))
        .await
        .into()
}

async fn graphql_ws_handler(
    Extension(schema): Extension<AppSchema>, // Changed from State to Extension
    Extension(auth_user): Extension<auth_middleware::AuthenticatedUser>,
    protocol: GraphQLProtocol,
    ws: WebSocketUpgrade,
) -> Response {
    ws.protocols(ALL_WEBSOCKET_PROTOCOLS)
        .on_upgrade(move |socket| async move {
            GraphQLWebSocket::new(socket, schema, protocol)
                .on_connection_init(move |_value| {
                    let auth_user = auth_user.clone();
                    async move {
                        let mut data = Data::default();
                        data.insert(auth_user);
                        Ok(data)
                    }
                })
                .serve()
                .await;
        })
}

/// Liveness probe endpoint.
async fn liveness_handler() -> impl IntoResponse {
    StatusCode::OK
}

/// Readiness probe endpoint.
async fn readiness_handler(State(state): State<AppState>) -> impl IntoResponse {
    // Check database connection
    match state.db_pool.get() {
        Ok(mut conn) => match diesel::sql_query("SELECT 1").execute(&mut conn) {
            Ok(_) => StatusCode::OK,
            Err(_) => StatusCode::SERVICE_UNAVAILABLE,
        },
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}

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
    #[arg(
        short,
        long,
        default_value_t = 30000,
        help = "Port to bind the server to"
    )]
    port: u16,
    #[arg(short, long, help = "Where do you want ThunderForgeVTT to store data?")]
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

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let formatting_layer = BunyanFormattingLayer::new("thunderforge".into(), std::io::stdout);

    Registry::default()
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(formatting_layer)
        .init();

    let mut config = Config::from_env();
    if let Some(data_path) = cli.data_path {
        config.data_path = data_path;
    }

    let directories = Directories::from(String::from(&config.data_path));
    directories.create_if_not_present();

    let (world_event_sender, _) = broadcast::channel(1024);

    let key = Key::from(&general_purpose::STANDARD.decode(&config.secret).unwrap());

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env");
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let db_pool = Pool::builder()
        .build(manager)
        .expect("Failed to create DB pool.");

    let schema = Schema::build(
        QueryRoot::default(),
        MutationRoot::default(),
        SubscriptionRoot,
    )
    .data(db_pool.clone())
    .finish();

    let app_state = AppState {
        config,
        directories: directories.clone(),
        world_event_sender,
        key,
        db_pool,
    };

    auth::ensure_admin_bootstrap_code(&app_state)
        .await
        .expect("Failed to initialize bootstrap admin setup state");

    let world_router = world::router().route_layer(from_fn_with_state(
        app_state.clone(),
        auth_middleware::require_authenticated_user,
    ));

    let graphql_router = Router::new()
        .route("/graphql", get(graphql_playground).post(graphql_handler))
        .route("/ws", get(graphql_ws_handler))
        .route_layer(from_fn_with_state(
            app_state.clone(),
            auth_middleware::require_authenticated_user,
        ));

    let app = Router::new()
        .route("/healthz", get(liveness_handler))
        .route("/readyz", get(readiness_handler))
        .merge(graphql_router)
        .merge(auth::router())
        .merge(world_router)
        .merge(serve::router(&directories))
        .fallback(errors::handler_404)
        .with_state(app_state.clone())
        .layer(from_fn(auth_middleware::rate_limit_auth_requests))
        .layer(from_fn_with_state(
            app_state.clone(),
            auth_middleware::require_csrf_for_session,
        ))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .layer(CookieManagerLayer::new())
        .layer(Extension(schema));

    let addr = SocketAddr::new(cli.ip_address.parse().unwrap(), cli.port);
    tracing::debug!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("signal received, starting graceful shutdown");
}
