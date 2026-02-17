#[cfg(not(coverage))]
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let data_dir = std::path::PathBuf::from(
        std::env::var("KUVASIVU_DATA_DIR").unwrap_or_else(|_| ".".to_string()),
    );
    std::fs::create_dir_all(data_dir.join("photos")).ok();

    let app = kuvasivu::build_router(&data_dir);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    tracing::info!("listening on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}

#[cfg(coverage)]
fn main() {}
