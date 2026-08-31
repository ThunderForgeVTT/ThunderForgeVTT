// async-graphql's MergedObject-generated resolve_field()/find_entity()
// dispatch nests one level deeper per merged Query/Mutation root member
// (see graphql.rs's QueryRoot/MutationRoot). Spec 015's ModerationQuery/
// ModerationMutation pushed this past the compiler's default 128-deep
// type-layout recursion limit — only surfaces in a full `cargo run`/
// release build that actually instantiates the live AppSchema, NOT in
// `cargo check`/`cargo test` (see docs/adrs/20260823-043-*.md's
// implementation notes). Raise, don't work around — every future merged
// query/mutation module will need this headroom too.
#![recursion_limit = "512"]

mod adapters;
mod admin;
mod attributes; // Phase 8: a system's own attribute set, from its manifest
mod auth;
mod auth_middleware;
mod canvas_assets_serve;
mod config;
mod db_types;
mod door_effects; // Spec 030: doors, as a contributor to the interaction seam
mod errors;
mod graphql;
mod interaction; // Spec 030: the effect registry, and the rules the GraphQL layer obeys
mod light_effects; // Spec 030: lighting, as a contributor to the interaction seam
mod lore_assets_serve; // Spec 012: authenticated proxy for lore image assets (mirrors canvas_assets_serve)
mod map_import;
mod markdown; // Spec 012: lore wiki GFM rendering, [[link]] resolution, slug generation
mod models;
mod moderation; // Spec 015: DMCA notice-and-takedown content moderation
mod network;
mod peer_signaling; // Spec 028: opaque WebRTC signaling relay between live sessions
mod pubsub;
mod scene_assets_serve; // Spec 022: authenticated proxy for scene preview images (mirrors lore_assets_serve)
mod scene_fingerprint; // Spec 028: derived scene content fingerprints
mod schema; // Add this line
mod serve;
mod session; // Phase 4.9.B.2: Session lifecycle management
mod state;
mod status_display; // Spec 029: resolving what each viewer is told
mod storage; // Spec 002: RustFS canvas image asset storage
mod system_hooks;
mod systems;
#[cfg(test)]
mod test_support; // Spec 002: shared fixtures for tests/tests requiring a live DB + RustFS
mod users;
mod utils;
mod world;
mod world_events;

use crate::config::{Config, Directories};
use crate::graphql::{AppSchema, MutationRoot, QueryRoot, SubscriptionRoot}; // Added SubscriptionRoot
use crate::state::AppState;
use async_graphql::http::{ALL_WEBSOCKET_PROTOCOLS, GraphQLPlaygroundConfig, playground_source};
use async_graphql::{Data, Schema}; // Added Data
use async_graphql_axum::{GraphQLProtocol, GraphQLRequest, GraphQLResponse, GraphQLWebSocket}; // Added GraphQLWebSocket
use axum::{
    Extension, Router,
    extract::{DefaultBodyLimit, State, WebSocketUpgrade},
    http::StatusCode,
    middleware::{from_fn, from_fn_with_state},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose};
use clap::Parser;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{RunQueryDsl, pg::PgConnection};
use std::net::SocketAddr;
use tokio::sync::broadcast;
use tower_cookies::{CookieManagerLayer, Key};
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::{EnvFilter, Registry, layer::SubscriberExt, util::SubscriberInitExt};

async fn graphql_playground() -> impl IntoResponse {
    Html(playground_source(
        GraphQLPlaygroundConfig::new("/api/graphql").subscription_endpoint("/api/ws"),
    ))
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

/// Spec 015 (FR-002): the ONLY GraphQL entry point reachable without an
/// authenticated session — `/api/graphql` itself is wrapped in
/// `require_authenticated_user` at the router-layer (below), before any
/// resolver's own auth logic ever runs, so `submitTakedownNotice`'s
/// resolver-level "no auth required" was previously unreachable in
/// practice. No `AuthenticatedUser` is inserted into the execution
/// context here; every resolver OTHER than the explicitly-public ones
/// (which never call `authenticated_user(ctx)`) still fails cleanly with
/// "Authentication required" if invoked through this route — this is not
/// a broader bypass, it just removes the transport-level all-or-nothing
/// gate for the one mutation that must be reachable by an anonymous
/// rights holder.
async fn graphql_public_handler(
    Extension(schema): Extension<AppSchema>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

async fn graphql_ws_handler(
    Extension(schema): Extension<AppSchema>, // Changed from State to Extension
    Extension(auth_user): Extension<auth_middleware::AuthenticatedUser>,
    protocol: GraphQLProtocol,
    ws: WebSocketUpgrade,
) -> Response {
    ws.protocols(ALL_WEBSOCKET_PROTOCOLS)
        .on_upgrade(move |socket| async move {
            // Counted, never logged per connection: a reconnect storm is
            // hundreds of these in a second, and the point of the number is
            // to say how many sockets are attached *now* — which is what
            // separates "the server stopped sending" from "the clients went
            // away". It is reported with the delivery counters every 10s.
            use crate::graphql::subscription_metrics::SOCKETS_OPEN;
            use std::sync::atomic::Ordering;
            SOCKETS_OPEN.fetch_add(1, Ordering::Relaxed);
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
            SOCKETS_OPEN.fetch_sub(1, Ordering::Relaxed);
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

#[derive(serde::Serialize)]
struct ServiceStatus {
    key: &'static str,
    up: bool,
    latency_ms: u128,
}

#[derive(serde::Serialize)]
struct StatusPageResponse {
    services: Vec<ServiceStatus>,
}

/// Public, unauthenticated status snapshot backing the frontend's `/status`
/// page. Deliberately reports only a stable `key` per subsystem (never a
/// real hostname, image tag, or connection string) — the page itself has
/// no auth gate, so nothing here should help an anonymous visitor map the
/// deployment's real infrastructure.
async fn status_handler(State(state): State<AppState>) -> impl IntoResponse {
    let db_start = std::time::Instant::now();
    let db_up = tokio::task::spawn_blocking(move || match state.db_pool.get() {
        Ok(mut conn) => diesel::sql_query("SELECT 1").execute(&mut conn).is_ok(),
        Err(_) => false,
    })
    .await
    .unwrap_or(false);
    let db_latency_ms = db_start.elapsed().as_millis();

    let storage_start = std::time::Instant::now();
    let rustfs_cfg = storage::rustfs::RustFsConfig::from_env();
    let storage_up = storage::rustfs::health_check(&rustfs_cfg).await.is_ok();
    let storage_latency_ms = storage_start.elapsed().as_millis();

    axum::Json(StatusPageResponse {
        services: vec![
            ServiceStatus {
                key: "core",
                up: true,
                latency_ms: 0,
            },
            ServiceStatus {
                key: "database",
                up: db_up,
                latency_ms: db_latency_ms,
            },
            ServiceStatus {
                key: "storage",
                up: storage_up,
                latency_ms: storage_latency_ms,
            },
        ],
    })
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

/// Spec 024 (FR-004, FR-005), ADR-047: which `SessionAdjudicator`
/// implementation `CRUCIBLE_MODE`/`CRUCIBLE_ENDPOINT` resolve to — the pure,
/// testable half of adjudicator selection (see T016, quickstart.md §3).
/// `mode` unset/`"local"` (the default, zero-config path every self-hosted
/// deployment gets — SC-001) resolves to `Local`. `"remote"` requires a
/// valid `endpoint` and resolves to `Remote`. Any other `mode`, or
/// `"remote"` with a missing/malformed `endpoint`, is an `Err` naming the
/// problem — this function never exits the process itself; `build_adjudicator`
/// (below) is the impure wrapper that does that (SC-003).
enum CrucibleModeChoice {
    Local,
    Remote(reqwest::Url),
}

fn resolve_crucible_mode(
    mode: Option<&str>,
    endpoint: Option<&str>,
) -> Result<CrucibleModeChoice, String> {
    match mode.unwrap_or("local") {
        "local" => Ok(CrucibleModeChoice::Local),
        "remote" => {
            let endpoint = endpoint.ok_or_else(|| {
                "CRUCIBLE_MODE=remote requires CRUCIBLE_ENDPOINT to be set".to_string()
            })?;
            let url = reqwest::Url::parse(endpoint).map_err(|err| {
                format!("CRUCIBLE_ENDPOINT is not a valid URL ({endpoint:?}): {err}")
            })?;
            Ok(CrucibleModeChoice::Remote(url))
        }
        other => Err(format!(
            "Unrecognized CRUCIBLE_MODE {other:?} — accepted values are \"local\" or \"remote\""
        )),
    }
}

/// Reads `CRUCIBLE_MODE`/`CRUCIBLE_ENDPOINT`, resolves them via
/// [`resolve_crucible_mode`], and constructs the corresponding
/// `SessionAdjudicator` — or exits the process immediately with a clear
/// error (SC-003), before the server begins accepting connections, per
/// research.md §4's fail-fast-at-boot convention.
fn build_adjudicator() -> std::sync::Arc<dyn thunderforge_crucible::SessionAdjudicator + Send + Sync>
{
    let mode = std::env::var("CRUCIBLE_MODE").ok();
    let endpoint = std::env::var("CRUCIBLE_ENDPOINT").ok();

    match resolve_crucible_mode(mode.as_deref(), endpoint.as_deref()) {
        Ok(CrucibleModeChoice::Local) => {
            std::sync::Arc::new(thunderforge_crucible::local::LocalAdjudicator)
        }
        Ok(CrucibleModeChoice::Remote(url)) => {
            std::sync::Arc::new(thunderforge_crucible::remote::RemoteAdjudicator::new(url))
        }
        Err(message) => {
            eprintln!("[Server] {message} — exiting.");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod crucible_mode_tests {
    use super::*;

    #[test]
    fn defaults_to_local_when_unset() {
        assert!(matches!(
            resolve_crucible_mode(None, None),
            Ok(CrucibleModeChoice::Local)
        ));
    }

    #[test]
    fn explicit_local_resolves_to_local() {
        assert!(matches!(
            resolve_crucible_mode(Some("local"), None),
            Ok(CrucibleModeChoice::Local)
        ));
    }

    #[test]
    fn remote_with_a_valid_endpoint_resolves_to_remote() {
        let result = resolve_crucible_mode(Some("remote"), Some("http://127.0.0.1:8090"));
        assert!(matches!(result, Ok(CrucibleModeChoice::Remote(_))));
    }

    #[test]
    fn remote_with_no_endpoint_is_an_error() {
        assert!(resolve_crucible_mode(Some("remote"), None).is_err());
    }

    #[test]
    fn remote_with_a_malformed_endpoint_is_an_error() {
        assert!(resolve_crucible_mode(Some("remote"), Some("not a url")).is_err());
    }

    #[test]
    fn an_unrecognized_mode_is_an_error() {
        assert!(resolve_crucible_mode(Some("not-a-real-mode"), None).is_err());
    }
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

    // Use 10000 buffer size for broadcast channel to allow backpressure handling
    // Per-world channels rather than one for the whole process. See
    // `thunderforge_pg_sockets::router` for the measurement that motivated it.
    let world_events: thunderforge_pg_sockets::SharedWorldRouter<_> =
        std::sync::Arc::new(thunderforge_pg_sockets::WorldRouter::new());
    let (presence_sender, _) = broadcast::channel(10000); // Phase 4.9.B.3: Presence changes
    let presence = std::sync::Arc::new(thunderforge_presence::PresenceRegistry::new());

    let key = Key::from(&general_purpose::STANDARD.decode(&config.secret).unwrap());

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env");
    let manager = ConnectionManager::<PgConnection>::new(database_url);

    // Sized on purpose, because the default is a number nobody chose.
    //
    // This used to be a bare `Pool::builder().build(manager)`, which takes
    // r2d2's `max_size` of **10** — a figure unrelated to this workload,
    // where nearly every database access runs inside `spawn_blocking` and
    // holds a connection for its duration, and a single busy world can put a
    // hundred of those in flight at once.
    //
    // The rules live in `thunderforge_pg::pool` so they can be tested as
    // rules; this is only the wiring.
    let sizing = thunderforge_pg::pool_sizing_from_env();

    let db_pool = Pool::builder()
        .max_size(sizing.max_size)
        // A few connections kept warm so early requests do not each pay for a
        // handshake. Left unset, r2d2 instead opens `max_size` connections
        // eagerly at startup — a surprising number of Postgres backends for a
        // process that may be about to idle.
        .min_idle(Some(sizing.min_idle))
        // Fail in seconds rather than the default half-minute: a request that
        // cannot get a connection is already in trouble, and making it wait
        // 30s turns a small capacity problem into a timeout cascade whose
        // cause has long since scrolled away.
        .connection_timeout(std::time::Duration::from_secs(
            sizing.connection_timeout_secs,
        ))
        .build(manager)
        .expect("Failed to create DB pool.");

    eprintln!(
        "[Server] 🗄️  Database pool: max_size={} min_idle={} connection_timeout={}s",
        sizing.max_size, sizing.min_idle, sizing.connection_timeout_secs
    );

    // Spec 024, ADR-047: which `SessionAdjudicator` to use, read once at
    // startup — mirrors the `DATABASE_URL` fail-fast-at-boot convention
    // above (research.md §4) rather than validating lazily on first use.
    let adjudicator = build_adjudicator();

    let app_state = AppState {
        config,
        directories: directories.clone(),
        world_events: world_events.clone(),
        presence_sender: presence_sender.clone(),
        presence: presence.clone(),
        key,
        db_pool: db_pool.clone(),
        system_hooks: std::sync::Arc::new(tokio::sync::RwLock::new(
            system_hooks::SystemHookRegistry::new(),
        )),
        adjudicator,
    };

    // Materialize any OAUTH_*-env-var-configured provider instances (ADR-041)
    // before the app starts accepting connections, so the sign-in screen's
    // first load already reflects them.
    eprintln!("[Server] 🚀 Materializing environment-configured OAuth providers");
    if let Err(err) = admin::materialize_env_oauth_providers(&db_pool).await {
        eprintln!("[Server] ⚠️  Failed to materialize env-configured OAuth providers: {err}");
    }

    // Spawn the PostgreSQL LISTEN background task
    eprintln!("[Server] 🚀 Starting PostgreSQL LISTEN background task");
    network::spawn_listen_task(db_pool.clone(), world_events);

    // Drop presence for worlds everyone has left.
    //
    // `in_world` prunes people whenever somebody asks about a world, but a
    // world nobody asks about again would hold its map forever. Own task, own
    // schedule, deliberately far off any hot path: this walks every shard, and
    // the event router established at some cost that an all-shards scan does
    // not belong on the delivery path.
    {
        let presence = presence.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                let dropped = presence.sweep(std::time::Instant::now());
                if dropped > 0 {
                    eprintln!("[Presence] 🧹 Released {dropped} empty world(s)");
                }
            }
        });
    }

    // Spawn the presence listener task (Phase 4.9.B.3)
    eprintln!("[Server] 🚀 Starting presence listener task");
    network::spawn_presence_listener_task(presence_sender);

    // Spawn the session cleanup task (Phase 4.9.B.2)
    eprintln!("[Server] 🚀 Starting session cleanup task");
    session::spawn_session_cleanup_task(db_pool.clone());

    // Spec 028 T125: fill in `content_hash` for assets written before the
    // column existed. Paced deliberately and allowed to take as long as it
    // takes — a NULL hash already means "the client must fetch this", so
    // the system is correct the whole time this is unfinished, merely
    // wasteful. Nothing waits on it.
    eprintln!("[Server] 🚀 Starting canvas asset content-hash backfill task");
    storage::backfill::spawn_content_hash_backfill_task(db_pool.clone());

    let schema = Schema::build(
        QueryRoot::default(),
        MutationRoot::default(),
        SubscriptionRoot,
    )
    .data(app_state.clone())
    .finish();

    auth::ensure_admin_bootstrap_code(&app_state)
        .await
        .expect("Failed to initialize bootstrap admin setup state");
    admin::ensure_admin_defaults(&app_state)
        .await
        .expect("Failed to initialize admin configuration state");

    // Spec 002 (FR-020): bootstrap the RustFS bucket so `docker compose
    // up` + this one command is the whole local-dev provisioning story,
    // no manual bucket-creation step. Non-fatal: a server started before
    // RustFS is reachable (or in a deployment without asset storage
    // configured yet) should still come up; the first asset write will
    // surface a clear storage error instead.
    {
        let rustfs_cfg = storage::rustfs::RustFsConfig::from_env();
        if let Err(e) = storage::rustfs::ensure_bucket(&rustfs_cfg).await {
            eprintln!(
                "[Server] ⚠️  RustFS bucket bootstrap failed (asset uploads will fail until this is resolved): {e}"
            );
        }
    }

    let world_router = world::router().route_layer(from_fn_with_state(
        app_state.clone(),
        auth_middleware::require_authenticated_user,
    ));
    let user_router = users::router().route_layer(from_fn_with_state(
        app_state.clone(),
        auth_middleware::require_authenticated_user,
    ));
    let map_import_router = map_import::router().route_layer(from_fn_with_state(
        app_state.clone(),
        auth_middleware::require_authenticated_user,
    ));
    let canvas_assets_router = canvas_assets_serve::router().route_layer(from_fn_with_state(
        app_state.clone(),
        auth_middleware::require_authenticated_user,
    ));
    let lore_assets_router = lore_assets_serve::router().route_layer(from_fn_with_state(
        app_state.clone(),
        auth_middleware::require_authenticated_user,
    ));
    let scene_assets_router = scene_assets_serve::router().route_layer(from_fn_with_state(
        app_state.clone(),
        auth_middleware::require_authenticated_user,
    ));

    let graphql_router = Router::new()
        .route(
            "/graphql",
            get(graphql_playground)
                .post(graphql_handler)
                // Spec 002: uploadCanvasImage sends multipart-encoded
                // image bytes through this endpoint (GraphQL multipart
                // request spec) — axum's default 2MB body limit would
                // reject any real image well before storage/transcode's
                // own MAX_UPLOAD_BYTES check ever runs, mirroring the
                // same fix already applied to map_import's REST route.
                .route_layer(DefaultBodyLimit::max(storage::transcode::MAX_UPLOAD_BYTES)),
        )
        .route("/ws", get(graphql_ws_handler))
        .route("/events/{world_id}", get(network::websocket_handler)) // Phase 4.9.B.2: Event WebSocket with session tracking
        .route_layer(from_fn_with_state(
            app_state.clone(),
            auth_middleware::require_authenticated_user,
        ));

    // Spec 015 (FR-002): deliberately NOT wrapped in
    // `require_authenticated_user` — see `graphql_public_handler`'s docs.
    let public_graphql_router =
        Router::new().route("/graphql/public", post(graphql_public_handler));

    let api_router = Router::new()
        .route("/healthz", get(liveness_handler))
        .route("/readyz", get(readiness_handler))
        .route("/status", get(status_handler))
        .merge(graphql_router)
        .merge(public_graphql_router)
        .merge(auth::router())
        .merge(user_router)
        .merge(world_router)
        .merge(map_import_router)
        .merge(canvas_assets_router)
        .merge(lore_assets_router)
        .merge(scene_assets_router);

    let systems_admin_router = systems::admin_router().route_layer(from_fn_with_state(
        app_state.clone(),
        auth_middleware::require_admin_user,
    ));

    let app = Router::new()
        .nest(
            "/api",
            api_router.nest("/systems", systems::router().merge(systems_admin_router)),
        )
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
        // Compress responses. The engine wasm dominates first load and was
        // going out uncompressed: ~24.7MB release-built, ~4.15MB brotli. That
        // ratio is not incidental — wasm is highly repetitive, so it
        // compresses far better than typical binary content.
        //
        // Content negotiation means a client that asks for neither encoding
        // still gets identity, so this cannot break anything that was working;
        // it only takes the win where the browser already advertised support.
        .layer(CompressionLayer::new().br(true).gzip(true))
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
