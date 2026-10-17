// SPDX-FileCopyrightText: 2026 Miikka Koskinen
//
// SPDX-License-Identifier: MIT

#[cfg(not(coverage))]
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let data_dir = std::path::PathBuf::from(
        std::env::var("KUVASIVU_DATA_DIR").unwrap_or_else(|_| ".".to_string()),
    );
    let cache_dir = std::path::PathBuf::from(
        std::env::var("KUVASIVU_CACHE_DIR")
            .unwrap_or_else(|_| data_dir.join("cache").to_string_lossy().to_string()),
    );
    std::fs::create_dir_all(data_dir.join("photos")).ok();
    std::fs::create_dir_all(&cache_dir).ok();

    let (app, thumbnail_worker) = kuvasivu::build_app(&data_dir, &cache_dir);
    tokio::spawn(thumbnail_worker.run());

    let port = std::env::var("KUVASIVU_PORT").unwrap_or_else(|_| "3000".to_string());

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .unwrap();
    tracing::info!("listening on http://localhost:{port}");
    axum::serve(listener, app).await.unwrap();
}

#[cfg(coverage)]
fn main() {}
