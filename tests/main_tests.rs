use std::fs;
use std::path::PathBuf;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn fixture_jpg() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/DSCF0199.jpg")
}

struct TestEnv {
    _dir: tempfile::TempDir,
    router: axum::Router,
}

fn setup_with_album() -> TestEnv {
    let dir = tempfile::tempdir().unwrap();
    let photos_dir = dir.path().join("photos");
    let album_dir = photos_dir.join("test-album");
    fs::create_dir_all(&album_dir).unwrap();
    fs::write(
        album_dir.join("album.toml"),
        "title = \"Test Album\"\ndescription = \"A test album.\"\ntimespan = \"January 2024\"\n",
    )
    .unwrap();

    // Copy fixture photo as three photos for prev/next testing
    let fixture = fs::read(fixture_jpg()).unwrap();
    fs::write(album_dir.join("photo-a.jpg"), &fixture).unwrap();
    fs::write(album_dir.join("photo-b.jpg"), &fixture).unwrap();
    fs::write(album_dir.join("photo-c.jpg"), &fixture).unwrap();

    let cache_dir = dir.path().join("cache");
    fs::create_dir(&cache_dir).unwrap();
    let router = kuvasivu::build_router(dir.path(), &cache_dir);
    TestEnv { _dir: dir, router }
}

fn make_minimal_png() -> Vec<u8> {
    // Minimal valid 1x1 white PNG
    let mut img = image::RgbImage::new(1, 1);
    img.put_pixel(0, 0, image::Rgb([255, 255, 255]));
    let mut buf = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buf);
    img.write_to(&mut cursor, image::ImageFormat::Png).unwrap();
    buf
}

fn setup_empty() -> TestEnv {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("photos")).unwrap();
    let cache_dir = dir.path().join("cache");
    fs::create_dir(&cache_dir).unwrap();
    let router = kuvasivu::build_router(dir.path(), &cache_dir);
    TestEnv { _dir: dir, router }
}

async fn get(router: axum::Router, uri: &str) -> (StatusCode, String) {
    let response = router
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&body).to_string())
}

async fn get_status(router: axum::Router, uri: &str) -> StatusCode {
    router
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status()
}

async fn get_bytes(router: axum::Router, uri: &str) -> (StatusCode, Vec<u8>, String) {
    let response = router
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let content_type = response
        .headers()
        .get("content-type")
        .map(|v| v.to_str().unwrap().to_string())
        .unwrap_or_default();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, body.to_vec(), content_type)
}

// --- Snapshot tests ---

#[tokio::test]
async fn test_index_page() {
    let env = setup_with_album();
    let (status, body) = get(env.router, "/").await;
    assert_eq!(status, StatusCode::OK);
    insta::assert_snapshot!("index_page", body);
}

#[tokio::test]
async fn test_index_empty() {
    let env = setup_empty();
    let (status, body) = get(env.router, "/").await;
    assert_eq!(status, StatusCode::OK);
    insta::assert_snapshot!("index_empty", body);
}

#[tokio::test]
async fn test_album_page() {
    let env = setup_with_album();
    let (status, body) = get(env.router, "/album/test-album").await;
    assert_eq!(status, StatusCode::OK);
    insta::assert_snapshot!("album_page", body);
}

#[tokio::test]
async fn test_photo_page_first() {
    let env = setup_with_album();
    let (status, body) = get(env.router, "/album/test-album/photo-a.jpg").await;
    assert_eq!(status, StatusCode::OK);
    insta::assert_snapshot!("photo_page_first", body);
}

#[tokio::test]
async fn test_photo_page_middle() {
    let env = setup_with_album();
    let (status, body) = get(env.router, "/album/test-album/photo-b.jpg").await;
    assert_eq!(status, StatusCode::OK);
    insta::assert_snapshot!("photo_page_middle", body);
}

#[tokio::test]
async fn test_photo_page_last() {
    let env = setup_with_album();
    let (status, body) = get(env.router, "/album/test-album/photo-c.jpg").await;
    assert_eq!(status, StatusCode::OK);
    insta::assert_snapshot!("photo_page_last", body);
}

// --- Status code tests ---

#[tokio::test]
async fn test_album_not_found() {
    let env = setup_with_album();
    assert_eq!(
        get_status(env.router, "/album/nonexistent").await,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn test_photo_not_found_album() {
    let env = setup_with_album();
    assert_eq!(
        get_status(env.router, "/album/nonexistent/foo.jpg").await,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn test_photo_not_found_file() {
    let env = setup_with_album();
    assert_eq!(
        get_status(env.router, "/album/test-album/nonexistent.jpg").await,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn test_serve_photo() {
    let env = setup_with_album();
    let (status, body, content_type) =
        get_bytes(env.router, "/photos/test-album/photo-a.jpg").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type, "image/jpeg");
    assert!(!body.is_empty());
}

#[tokio::test]
async fn test_serve_photo_cache_header() {
    let env = setup_with_album();
    let response = env
        .router
        .oneshot(
            Request::builder()
                .uri("/photos/test-album/photo-a.jpg")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let cache_control = response
        .headers()
        .get("cache-control")
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(cache_control, "public, max-age=31536000, immutable");
}

#[tokio::test]
async fn test_serve_photo_missing() {
    let env = setup_with_album();
    assert_eq!(
        get_status(env.router, "/photos/test-album/nope.jpg").await,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn test_serve_thumb_small() {
    let env = setup_with_album();
    let (status, body, content_type) =
        get_bytes(env.router, "/thumbs/test-album/small/photo-a.jpg").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type, "image/jpeg");
    assert!(!body.is_empty());
}

#[tokio::test]
async fn test_serve_thumb_medium() {
    let env = setup_with_album();
    let (status, body, content_type) =
        get_bytes(env.router, "/thumbs/test-album/medium/photo-a.jpg").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type, "image/jpeg");
    assert!(!body.is_empty());
}

#[tokio::test]
async fn test_serve_thumb_invalid_size() {
    let env = setup_with_album();
    assert_eq!(
        get_status(env.router, "/thumbs/test-album/huge/photo-a.jpg").await,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn test_serve_thumb_missing_photo() {
    let env = setup_with_album();
    assert_eq!(
        get_status(env.router, "/thumbs/test-album/small/nope.jpg").await,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn test_serve_thumb_cached() {
    let dir = tempfile::tempdir().unwrap();
    let album_dir = dir.path().join("photos").join("test-album");
    fs::create_dir_all(&album_dir).unwrap();

    let fixture = fs::read(fixture_jpg()).unwrap();
    fs::write(album_dir.join("photo.jpg"), &fixture).unwrap();

    // Pre-generate the thumbnail so the cache path is hit
    let cache_dir = dir.path().join("cache");
    let thumb_dir = cache_dir.join("test-album").join("small");
    fs::create_dir_all(&thumb_dir).unwrap();
    let img = image::open(album_dir.join("photo.jpg")).unwrap();
    let thumb = img.resize(400, 400, image::imageops::FilterType::Lanczos3);
    thumb.save(thumb_dir.join("photo.jpg")).unwrap();

    let router = kuvasivu::build_router(dir.path(), &cache_dir);
    let (status, body, content_type) =
        get_bytes(router, "/thumbs/test-album/small/photo.jpg").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type, "image/jpeg");
    assert!(!body.is_empty());
}

#[tokio::test]
async fn test_serve_photo_png() {
    let dir = tempfile::tempdir().unwrap();
    let album_dir = dir.path().join("photos").join("test-album");
    fs::create_dir_all(&album_dir).unwrap();
    fs::write(album_dir.join("photo.png"), &make_minimal_png()).unwrap();

    let router = kuvasivu::build_router(dir.path(), &dir.path().join("cache"));
    let (status, _, content_type) =
        get_bytes(router, "/photos/test-album/photo.png").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type, "image/png");
}

#[tokio::test]
async fn test_serve_photo_webp() {
    let dir = tempfile::tempdir().unwrap();
    let album_dir = dir.path().join("photos").join("test-album");
    fs::create_dir_all(&album_dir).unwrap();
    // Create a minimal webp using image crate
    let img = image::RgbImage::new(1, 1);
    let mut buf = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buf);
    img.write_to(&mut cursor, image::ImageFormat::WebP).unwrap();
    fs::write(album_dir.join("photo.webp"), &buf).unwrap();

    let router = kuvasivu::build_router(dir.path(), &dir.path().join("cache"));
    let (status, _, content_type) =
        get_bytes(router, "/photos/test-album/photo.webp").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type, "image/webp");
}

#[tokio::test]
async fn test_serve_photo_unknown_extension() {
    let dir = tempfile::tempdir().unwrap();
    let album_dir = dir.path().join("photos").join("test-album");
    fs::create_dir_all(&album_dir).unwrap();
    fs::write(album_dir.join("data.bin"), b"binary data").unwrap();

    let router = kuvasivu::build_router(dir.path(), &dir.path().join("cache"));
    let (status, _, content_type) =
        get_bytes(router, "/photos/test-album/data.bin").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type, "application/octet-stream");
}
