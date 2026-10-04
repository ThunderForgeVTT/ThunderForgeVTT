//! Thin binary entrypoint for `mapforge-server`, mirroring
//! `crucible-server`'s convention: all routing lives in
//! `thunderforge_mapforge::server`, and this only resolves a corpus directory
//! and a port, then serves.

use thunderforge_mapforge::{server, source::MapSource};

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8095);

    // Defaults to the repo's own fixture corpus — the whole point is that this
    // treats `examples/maps` as its object store.
    let root = std::env::var("MAPFORGE_ROOT").unwrap_or_else(|_| "examples/maps".to_string());
    let source = MapSource::new(&root);

    let maps = source.list();
    eprintln!("[mapforge] serving {} maps from {root}", maps.len());
    for name in &maps {
        eprintln!("[mapforge]   {name}");
    }

    // Localhost only. This service has no authentication of any kind; binding
    // it to 0.0.0.0 would publish the corpus to the network.
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|err| panic!("mapforge-server: failed to bind {addr}: {err}"));

    eprintln!("[mapforge] listening on http://{addr}");
    axum::serve(listener, server::router(source))
        .await
        .expect("mapforge-server: failed to serve");
}
