//! Spec 024 (User Story 2): thin binary entrypoint for `crucible-server` —
//! standalone, out-of-process adjudication, per `plan.md`'s "thin binary
//! wrapper" convention. All routing logic lives in `thunderforge_crucible::server`
//! (shared with the in-process integration test) — this binary only reads
//! its listen address and serves it.

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8090);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|err| panic!("crucible-server: failed to bind {addr}: {err}"));

    eprintln!("[crucible-server] listening on {addr}");

    axum::serve(listener, thunderforge_crucible::server::router())
        .await
        .expect("crucible-server: failed to serve");
}
