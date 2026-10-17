// SPDX-FileCopyrightText: 2026 Miikka Koskinen
//
// SPDX-License-Identifier: MIT

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
    setup_with_album_and_config("")
}

fn setup_with_album_and_config(site_toml: &str) -> TestEnv {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("site.toml"), site_toml).unwrap();
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
async fn test_status_page() {
    let env = setup_with_album();
    let (status, body) = get(env.router, "/status").await;
    assert_eq!(status, StatusCode::OK);
    insta::assert_snapshot!("status_page", body);
}

#[tokio::test]
async fn test_album_page() {
    let env = setup_with_album();
    let (status, body) = get(env.router, "/album/test-album").await;
    assert_eq!(status, StatusCode::OK);
    insta::assert_snapshot!("album_page", body);
}

#[tokio::test]
async fn test_album_page_social_preview() {
    let env = setup_with_album_and_config("base_url = \"https://photos.example.com/\"\n");
    let (status, body) = get(env.router, "/album/test-album").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains(
        r#"<meta property="og:url" content="https://photos.example.com/album/test-album">"#
    ));
    assert!(body.contains(
        r#"<meta property="og:image" content="https://photos.example.com/thumbs/test-album/medium/photo-a.jpg">"#
    ));
    assert!(body.contains(r#"<meta property="og:title" content="Test Album">"#));
    assert!(body.contains(r#"<meta property="og:description" content="A test album.">"#));
}

#[tokio::test]
async fn test_index_page_social_preview() {
    let env = setup_with_album_and_config("base_url = \"https://photos.example.com\"\n");
    let (_, body) = get(env.router, "/").await;
    assert!(body.contains(r#"<meta property="og:url" content="https://photos.example.com/">"#));
    assert!(body.contains(
        r#"<meta property="og:image" content="https://photos.example.com/thumbs/test-album/medium/photo-a.jpg">"#
    ));
}

#[tokio::test]
async fn test_photo_page_social_preview() {
    let env = setup_with_album_and_config("base_url = \"https://photos.example.com\"\n");
    let (_, body) = get(env.router, "/album/test-album/photo-b.jpg").await;
    assert!(body.contains(
        r#"<meta property="og:url" content="https://photos.example.com/album/test-album/photo-b.jpg">"#
    ));
    assert!(body.contains(
        r#"<meta property="og:image" content="https://photos.example.com/thumbs/test-album/medium/photo-b.jpg">"#
    ));
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
async fn test_serve_thumb_avif_not_rendered() {
    // AVIFs are only rendered in the background, never on request.
    let env = setup_with_album();
    assert_eq!(
        get_status(env.router, "/thumbs/test-album/small/photo-a.jpg.avif").await,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn test_background_worker_renders_jpegs_then_avifs() {
    let dir = tempfile::tempdir().unwrap();
    let album_dir = dir.path().join("photos/test-album");
    fs::create_dir_all(&album_dir).unwrap();
    // The small fixture keeps the AVIF encodes quick.
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/DSCF0263.jpg");
    fs::copy(&fixture, album_dir.join("photo-a.jpg")).unwrap();
    fs::copy(&fixture, album_dir.join("photo-b.jpg")).unwrap();
    fs::write(album_dir.join("broken.jpg"), b"not a jpeg").unwrap();
    let cache_dir = dir.path().join("cache");
    let (router, worker) = kuvasivu::build_app(dir.path(), &cache_dir);

    // Before the AVIFs are ready, pages only link to the JPEGs.
    let (_, body) = get(router.clone(), "/album/test-album").await;
    assert!(!body.contains(".avif"));

    let worker = tokio::spawn(worker.run());
    let first_avif = cache_dir.join("test-album/medium/photo-a.jpg.avif");
    let last_avif = cache_dir.join("test-album/large/photo-b.jpg.avif");
    let mut saw_first_avif = false;
    tokio::time::timeout(std::time::Duration::from_secs(60), async {
        while !last_avif.is_file() {
            if !saw_first_avif && first_avif.is_file() {
                saw_first_avif = true;
                // Every JPEG is done before the first AVIF.
                for name in [
                    "small/photo-a.jpg",
                    "medium/photo-a.jpg",
                    "screen/photo-a.jpg",
                    "small/photo-b.jpg",
                    "medium/photo-b.jpg",
                    "screen/photo-b.jpg",
                ] {
                    assert!(cache_dir.join("test-album").join(name).is_file(), "{name}");
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("worker should render all thumbnails");
    worker.abort();

    let (_, body) = get(router.clone(), "/status").await;
    assert!(
        body.contains(r#"<span class="status-count">2/3</span>"#),
        "{body}"
    );
    assert!(body.contains("1 failed"), "{body}");
    assert!(
        body.contains("<code>test-album/broken.jpg</code>"),
        "{body}"
    );
    assert!(!body.contains("worker-stopped"), "{body}");

    let (_, body) = get(router.clone(), "/album/test-album").await;
    assert!(body.contains(r#"<source srcset="/thumbs/test-album/medium/photo-a.jpg.avif""#));
    let (_, body) = get(router.clone(), "/album/test-album/photo-b.jpg").await;
    // The fixture is smaller than the medium size, so that's all the photo page offers.
    assert!(
        body.contains(r#"<source srcset="/thumbs/test-album/medium/photo-b.jpg.avif 640w" sizes="100vw" type="image/avif">"#),
        "{body}"
    );
    let (status, body, content_type) =
        get_bytes(router, "/thumbs/test-album/medium/photo-a.jpg.avif").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type, "image/avif");
    assert_eq!(&body[4..12], b"ftypavif");

    // Only the final thumbnails are left in the cache, no temp files.
    let names: Vec<_> = fs::read_dir(cache_dir.join("test-album/medium"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.starts_with('.'))
        .collect();
    assert!(names.is_empty(), "{names:?}");
}

#[test]
fn test_startup_removes_stale_temp_files() {
    let dir = tempfile::tempdir().unwrap();
    let thumb_dir = dir.path().join("cache/test-album/small");
    fs::create_dir_all(&thumb_dir).unwrap();
    // Left behind by an encode that was killed mid-write.
    fs::write(thumb_dir.join(".tmp-7-photo-a.jpg.avif"), b"").unwrap();
    fs::write(thumb_dir.join("photo-b.jpg.avif"), b"done").unwrap();

    let _ = kuvasivu::build_router(dir.path(), &dir.path().join("cache"));

    let names: Vec<_> = fs::read_dir(thumb_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, vec!["photo-b.jpg.avif"]);
}

#[tokio::test]
async fn test_serve_thumb_avif_missing_photo() {
    let env = setup_with_album();
    assert_eq!(
        get_status(env.router, "/thumbs/test-album/small/nope.jpg.avif").await,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn test_serve_thumb_large_does_not_upscale() {
    let dir = tempfile::tempdir().unwrap();
    let album_dir = dir.path().join("photos").join("test-album");
    fs::create_dir_all(&album_dir).unwrap();
    // 640x427, smaller than the large size.
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/DSCF0263.jpg");
    fs::copy(fixture, album_dir.join("photo.jpg")).unwrap();

    let router = kuvasivu::build_router(dir.path(), &dir.path().join("cache"));
    let (status, body, content_type) =
        get_bytes(router, "/thumbs/test-album/large/photo.jpg").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type, "image/jpeg");
    let img = image::load_from_memory(&body).unwrap();
    assert_eq!((img.width(), img.height()), (640, 427));
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
    fs::write(album_dir.join("photo.png"), make_minimal_png()).unwrap();

    let router = kuvasivu::build_router(dir.path(), &dir.path().join("cache"));
    let (status, _, content_type) = get_bytes(router, "/photos/test-album/photo.png").await;
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
    let (status, _, content_type) = get_bytes(router, "/photos/test-album/photo.webp").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type, "image/webp");
}

#[tokio::test]
async fn test_serve_photo_path_traversal() {
    let env = setup_with_album();
    assert_eq!(
        get_status(env.router, "/photos/%2e%2e/photo-a.jpg").await,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn test_album_path_traversal() {
    let env = setup_with_album();
    assert_eq!(
        get_status(env.router, "/album/..").await,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn test_photo_path_traversal() {
    let env = setup_with_album();
    assert_eq!(
        get_status(env.router, "/album/test-album/%2e%2e").await,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn test_thumb_path_traversal() {
    let env = setup_with_album();
    assert_eq!(
        get_status(env.router, "/thumbs/%2e%2e/small/photo.jpg").await,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn test_serve_photo_unknown_extension() {
    let dir = tempfile::tempdir().unwrap();
    let album_dir = dir.path().join("photos").join("test-album");
    fs::create_dir_all(&album_dir).unwrap();
    fs::write(album_dir.join("data.bin"), b"binary data").unwrap();

    let router = kuvasivu::build_router(dir.path(), &dir.path().join("cache"));
    assert_eq!(
        get_status(router, "/photos/test-album/data.bin").await,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn test_serve_photo_does_not_expose_album_metadata() {
    let env = setup_with_album();
    assert_eq!(
        get_status(env.router, "/photos/test-album/album.toml").await,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn test_serve_file_streams_large_photo() {
    let dir = tempfile::tempdir().unwrap();
    let album_dir = dir.path().join("photos/test-album");
    fs::create_dir_all(&album_dir).unwrap();
    let contents = vec![42; 64 * 1024];
    fs::write(album_dir.join("large.jpg"), &contents).unwrap();

    let router = kuvasivu::build_router(dir.path(), &dir.path().join("cache"));
    let response = router
        .oneshot(
            Request::builder()
                .uri("/photos/test-album/large.jpg")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let mut body = response.into_body();
    let first = body.frame().await.unwrap().unwrap().into_data().unwrap();
    assert!(first.len() < contents.len());
    let remaining = body.collect().await.unwrap().to_bytes();
    assert_eq!([first.as_ref(), remaining.as_ref()].concat(), contents);
}
