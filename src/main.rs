#[cfg(not(coverage))]
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let photos_dir = std::path::PathBuf::from("photos");
    std::fs::create_dir_all(&photos_dir).ok();

    let app = kuvasivu::build_router(photos_dir);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    tracing::info!("listening on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}

#[cfg(coverage)]
fn main() {}
