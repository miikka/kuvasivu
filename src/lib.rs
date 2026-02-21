mod exif;

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use askama::Template;
use axum::extract::{self, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use image::imageops::FilterType;
use serde::Deserialize;
use tower_http::services::ServeDir;

use exif::{ExifInfo, read_exif_info};

enum AppError {
    Render,
    NotFound,
}

impl From<askama::Error> for AppError {
    fn from(_: askama::Error) -> Self {
        AppError::Render
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::Render => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Failed to render template").into_response()
            }
            AppError::NotFound => StatusCode::NOT_FOUND.into_response(),
        }
    }
}

const SMALL_SIZE: u32 = 400;
const MEDIUM_SIZE: u32 = 1200;

#[derive(Deserialize)]
struct SiteConfig {
    title: Option<String>,
    footer_snippet: Option<String>,
}

#[derive(Clone)]
struct AppState {
    photos_dir: PathBuf,
    cache_dir: PathBuf,
    site_title: String,
    footer_snippet: Option<String>,
}

#[derive(Deserialize, Default)]
struct AlbumMeta {
    title: Option<String>,
    description: Option<String>,
    timespan: Option<String>,
}

struct Album {
    slug: String,
    title: String,
    description: String,
    timespan: String,
    cover: Option<String>,
}

struct Photo {
    filename: String,
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    site_title: String,
    footer_snippet: Option<String>,
    albums: Vec<Album>,
}

#[derive(Template)]
#[template(path = "album.html")]
struct AlbumTemplate {
    site_title: String,
    footer_snippet: Option<String>,
    album: Album,
    photos: Vec<Photo>,
}

#[derive(Template)]
#[template(path = "photo.html")]
struct PhotoTemplate {
    site_title: String,
    footer_snippet: Option<String>,
    album: Album,
    photo: Photo,
    prev: Option<Photo>,
    next: Option<Photo>,
    exif: ExifInfo,
}

fn load_site_config(data_dir: &Path) -> SiteConfig {
    std::fs::read_to_string(data_dir.join("site.toml"))
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or(SiteConfig { title: None, footer_snippet: None })
}

/// Validates that a user-supplied path segment is a plain filename with no
/// directory traversal components (e.g. `..`, `/`, `\`).
fn is_safe_path_segment(segment: &str) -> bool {
    Path::new(segment).file_name() == Some(OsStr::new(segment))
}

pub fn build_router(data_dir: &Path, cache_dir: &Path) -> Router {
    let config = load_site_config(data_dir);
    let photos_dir = data_dir.join("photos");
    let state = AppState {
        photos_dir,
        cache_dir: cache_dir.to_path_buf(),
        site_title: config.title.unwrap_or_else(|| "Kuvasivu".to_string()),
        footer_snippet: config.footer_snippet,
    };

    Router::new()
        .route("/", get(index))
        .route("/album/{slug}", get(album))
        .route("/album/{slug}/{filename}", get(photo))
        .route("/photos/{album}/{filename}", get(serve_photo))
        .route("/thumbs/{album}/{size}/{filename}", get(serve_thumb))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state)
}

async fn index(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let albums = scan_albums(&state.photos_dir);
    let site_title = state.site_title.to_string();
    let footer_snippet = state.footer_snippet.clone();
    Ok(Html((IndexTemplate { site_title, footer_snippet, albums }).render()?))
}

async fn album(
    State(state): State<AppState>,
    extract::Path(slug): extract::Path<String>,
) -> Result<impl IntoResponse, AppError> {
    if !is_safe_path_segment(&slug) {
        return Err(AppError::NotFound);
    }
    let album_path = state.photos_dir.join(&slug);
    if !album_path.is_dir() {
        return Err(AppError::NotFound);
    }

    let photos = list_photos(&album_path);
    let album = load_album(&slug, &album_path, &photos);

    let site_title = state.site_title.to_string();
    let footer_snippet = state.footer_snippet.clone();
    Ok(Html((AlbumTemplate { site_title, footer_snippet, album, photos }).render()?))
}

async fn photo(
    State(state): State<AppState>,
    extract::Path((slug, filename)): extract::Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    if !is_safe_path_segment(&slug) || !is_safe_path_segment(&filename) {
        return Err(AppError::NotFound);
    }
    let album_path = state.photos_dir.join(&slug);
    if !album_path.is_dir() {
        return Err(AppError::NotFound);
    }

    let photos = list_photos(&album_path);

    let index = photos
        .iter()
        .position(|p| p.filename == filename)
        .ok_or(AppError::NotFound)?;

    let prev = if index > 0 {
        Some(Photo {
            filename: photos[index - 1].filename.clone(),
        })
    } else {
        None
    };

    let next = if index + 1 < photos.len() {
        Some(Photo {
            filename: photos[index + 1].filename.clone(),
        })
    } else {
        None
    };

    let album = load_album(&slug, &album_path, &photos);

    let photo_path = album_path.join(&filename);
    let exif = read_exif_info(&photo_path);

    let photo = Photo {
        filename: filename.clone(),
    };

    let site_title = state.site_title.to_string();
    let footer_snippet = state.footer_snippet.clone();
    Ok(Html(
        (PhotoTemplate {
            site_title,
            footer_snippet,
            album,
            photo,
            prev,
            next,
            exif,
        })
        .render()?,
    ))
}

async fn serve_photo(
    State(state): State<AppState>,
    extract::Path((album, filename)): extract::Path<(String, String)>,
) -> Result<impl IntoResponse, StatusCode> {
    if !is_safe_path_segment(&album) || !is_safe_path_segment(&filename) {
        return Err(StatusCode::NOT_FOUND);
    }
    let path = state.photos_dir.join(&album).join(&filename);
    serve_file(&path).await
}

async fn serve_thumb(
    State(state): State<AppState>,
    extract::Path((album, size, filename)): extract::Path<(String, String, String)>,
) -> Result<impl IntoResponse, StatusCode> {
    if !is_safe_path_segment(&album) || !is_safe_path_segment(&filename) {
        return Err(StatusCode::NOT_FOUND);
    }
    let max_dim = match size.as_str() {
        "small" => SMALL_SIZE,
        "medium" => MEDIUM_SIZE,
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    let album_path = state.photos_dir.join(&album);
    let original = album_path.join(&filename);
    if !original.is_file() {
        return Err(StatusCode::NOT_FOUND);
    }

    let thumb_dir = state.cache_dir.join(&album).join(&size);
    let thumb_path = thumb_dir.join(&filename);

    if !thumb_path.is_file() {
        generate_thumbnail(&original, &thumb_path, &thumb_dir, max_dim)?;
    }

    serve_file(&thumb_path).await
}

fn generate_thumbnail(
    original: &Path,
    thumb_path: &Path,
    thumb_dir: &Path,
    max_dim: u32,
) -> Result<(), StatusCode> {
    std::fs::create_dir_all(thumb_dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let img = image::open(original).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let thumb = img.resize(max_dim, max_dim, FilterType::Lanczos3);
    thumb
        .save(thumb_path)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}

async fn serve_file(path: &Path) -> Result<impl IntoResponse, StatusCode> {
    if !path.is_file() {
        return Err(StatusCode::NOT_FOUND);
    }

    let body = tokio::fs::read(path)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let content_type = match path.extension().and_then(|e| e.to_str()) {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    };

    Ok((
        [
            (axum::http::header::CONTENT_TYPE, content_type),
            (
                axum::http::header::CACHE_CONTROL,
                "public, max-age=31536000, immutable",
            ),
        ],
        body,
    ))
}

fn scan_albums(photos_dir: &Path) -> Vec<Album> {
    let mut albums = Vec::new();
    let Ok(entries) = std::fs::read_dir(photos_dir) else {
        return albums;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() || entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let slug = entry.file_name().to_string_lossy().to_string();
        let photos = list_photos(&path);
        albums.push(load_album(&slug, &path, &photos));
    }

    albums.sort_by(|a, b| a.title.cmp(&b.title));
    albums
}

fn load_album(slug: &str, album_path: &Path, photos: &[Photo]) -> Album {
    let meta = load_meta(album_path);
    let cover = photos.first().map(|p| p.filename.clone());
    Album {
        title: meta.title.unwrap_or_else(|| slug_to_title(slug)),
        description: meta.description.unwrap_or_default(),
        timespan: meta
            .timespan
            .unwrap_or_else(|| derive_timespan(album_path, photos)),
        slug: slug.to_string(),
        cover,
    }
}

fn load_meta(album_path: &Path) -> AlbumMeta {
    let toml_path = album_path.join("album.toml");
    std::fs::read_to_string(&toml_path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

fn list_photos(album_path: &Path) -> Vec<Photo> {
    let mut photos = Vec::new();
    let Ok(entries) = std::fs::read_dir(album_path) else {
        return photos;
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let lower = name.to_lowercase();
        if lower.ends_with(".jpg")
            || lower.ends_with(".jpeg")
            || lower.ends_with(".png")
            || lower.ends_with(".webp")
        {
            photos.push(Photo { filename: name });
        }
    }

    photos.sort_by(|a, b| a.filename.cmp(&b.filename));
    photos
}

fn slug_to_title(slug: &str) -> String {
    slug.replace('-', " ")
        .split_whitespace()
        .map(|word| {
            let mut c = word[..1].to_uppercase();
            c.push_str(&word[1..]);
            c
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn derive_timespan(album_path: &Path, photos: &[Photo]) -> String {
    let mut dates: Vec<String> = Vec::new();

    for photo in photos {
        if let Some(date) = exif::read_exif_date(&album_path.join(&photo.filename)) {
            dates.push(date);
        }
    }

    format_date_range(&dates)
}

fn format_date_range(dates: &[String]) -> String {
    if dates.is_empty() {
        return String::new();
    }

    let mut sorted: Vec<&String> = dates.iter().collect();
    sorted.sort();
    let first = sorted[0];
    let last = sorted[sorted.len() - 1];

    let first_month = exif::format_year_month(first);
    let last_month = exif::format_year_month(last);

    if first_month == last_month {
        first_month
    } else {
        format!("{} – {}", first_month, last_month)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn slug_to_title_basic() {
        assert_eq!(slug_to_title("my-album"), "My Album");
    }

    #[test]
    fn slug_to_title_single_word() {
        assert_eq!(slug_to_title("photos"), "Photos");
    }

    #[test]
    fn slug_to_title_empty() {
        assert_eq!(slug_to_title(""), "");
    }

    #[test]
    fn slug_to_title_multiple_hyphens() {
        assert_eq!(slug_to_title("my-cool-album"), "My Cool Album");
    }

    fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/DSCF0199.jpg")
    }

    #[test]
    fn derive_timespan_empty() {
        let dir = tempfile::tempdir().unwrap();
        let photos: Vec<Photo> = vec![];
        assert_eq!(derive_timespan(dir.path(), &photos), "");
    }

    #[test]
    fn derive_timespan_single_date() {
        let dir = tempfile::tempdir().unwrap();
        fs::copy(fixture_path(), dir.path().join("photo.jpg")).unwrap();
        let photos = vec![Photo {
            filename: "photo.jpg".to_string(),
        }];
        assert_eq!(derive_timespan(dir.path(), &photos), "February 2026");
    }

    #[test]
    fn derive_timespan_no_exif() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("photo.jpg"), b"not a real jpeg").unwrap();
        let photos = vec![Photo {
            filename: "photo.jpg".to_string(),
        }];
        assert_eq!(derive_timespan(dir.path(), &photos), "");
    }

    #[test]
    fn format_date_range_empty() {
        assert_eq!(format_date_range(&[]), "");
    }

    #[test]
    fn format_date_range_single() {
        let dates = vec!["2024:06:15 12:00:00".to_string()];
        assert_eq!(format_date_range(&dates), "June 2024");
    }

    #[test]
    fn format_date_range_same_month() {
        let dates = vec![
            "2024:06:15 12:00:00".to_string(),
            "2024:06:20 12:00:00".to_string(),
        ];
        assert_eq!(format_date_range(&dates), "June 2024");
    }

    #[test]
    fn format_date_range_different_months() {
        let dates = vec![
            "2024:06:15 12:00:00".to_string(),
            "2024:09:20 12:00:00".to_string(),
        ];
        assert_eq!(
            format_date_range(&dates),
            "June 2024 – September 2024"
        );
    }

    #[test]
    fn load_meta_with_toml() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("album.toml"),
            "title = \"My Title\"\ndescription = \"My Desc\"\ntimespan = \"2024\"\n",
        )
        .unwrap();
        let meta = load_meta(dir.path());
        assert_eq!(meta.title.as_deref(), Some("My Title"));
        assert_eq!(meta.description.as_deref(), Some("My Desc"));
        assert_eq!(meta.timespan.as_deref(), Some("2024"));
    }

    #[test]
    fn load_meta_without_toml() {
        let dir = tempfile::tempdir().unwrap();
        let meta = load_meta(dir.path());
        assert!(meta.title.is_none());
        assert!(meta.description.is_none());
        assert!(meta.timespan.is_none());
    }

    #[test]
    fn load_meta_invalid_toml() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("album.toml"), "not valid {{{{ toml").unwrap();
        let meta = load_meta(dir.path());
        assert!(meta.title.is_none());
    }

    #[test]
    fn list_photos_filters_and_sorts() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("b.jpg"), b"").unwrap();
        fs::write(dir.path().join("a.png"), b"").unwrap();
        fs::write(dir.path().join("c.webp"), b"").unwrap();
        fs::write(dir.path().join("d.txt"), b"").unwrap();
        fs::write(dir.path().join("album.toml"), b"").unwrap();
        let photos = list_photos(dir.path());
        let names: Vec<&str> = photos.iter().map(|p| p.filename.as_str()).collect();
        assert_eq!(names, vec!["a.png", "b.jpg", "c.webp"]);
    }

    #[test]
    fn list_photos_missing_dir() {
        let photos = list_photos(Path::new("/nonexistent/dir"));
        assert!(photos.is_empty());
    }

    #[test]
    fn list_photos_jpeg_extension() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("photo.jpeg"), b"").unwrap();
        let photos = list_photos(dir.path());
        assert_eq!(photos.len(), 1);
        assert_eq!(photos[0].filename, "photo.jpeg");
    }

    #[test]
    fn scan_albums_skips_dotfiles_and_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".hidden")).unwrap();
        fs::create_dir(dir.path().join("visible-album")).unwrap();
        fs::write(dir.path().join("a-file.txt"), b"").unwrap();
        let albums = scan_albums(dir.path());
        assert_eq!(albums.len(), 1);
        assert_eq!(albums[0].slug, "visible-album");
    }

    #[test]
    fn scan_albums_sorts_by_title() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("z-album")).unwrap();
        fs::create_dir(dir.path().join("a-album")).unwrap();
        let albums = scan_albums(dir.path());
        assert_eq!(albums[0].title, "A Album");
        assert_eq!(albums[1].title, "Z Album");
    }

    #[test]
    fn scan_albums_nonexistent_dir() {
        let albums = scan_albums(Path::new("/nonexistent"));
        assert!(albums.is_empty());
    }

    #[test]
    fn load_album_with_meta() {
        let dir = tempfile::tempdir().unwrap();
        let album_dir = dir.path().join("test");
        fs::create_dir(&album_dir).unwrap();
        fs::write(
            album_dir.join("album.toml"),
            "title = \"Custom Title\"\ndescription = \"Desc\"\ntimespan = \"2024\"\n",
        )
        .unwrap();
        fs::write(album_dir.join("a.jpg"), b"").unwrap();
        let photos = list_photos(&album_dir);
        let album = load_album("test", &album_dir, &photos);
        assert_eq!(album.title, "Custom Title");
        assert_eq!(album.description, "Desc");
        assert_eq!(album.timespan, "2024");
        assert_eq!(album.cover.as_deref(), Some("a.jpg"));
        assert_eq!(album.slug, "test");
    }

    #[test]
    fn load_album_without_meta() {
        let dir = tempfile::tempdir().unwrap();
        let album_dir = dir.path().join("my-album");
        fs::create_dir(&album_dir).unwrap();
        let photos = list_photos(&album_dir);
        let album = load_album("my-album", &album_dir, &photos);
        assert_eq!(album.title, "My Album");
        assert_eq!(album.description, "");
        assert_eq!(album.timespan, "");
        assert!(album.cover.is_none());
    }

    #[test]
    fn app_error_not_found_response() {
        let response = AppError::NotFound.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn app_error_render_response() {
        let response = AppError::Render.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn app_error_from_askama() {
        let err: AppError = askama::Error::Custom(Box::new(std::fmt::Error)).into();
        matches!(err, AppError::Render);
    }

    #[test]
    fn safe_path_segment_accepts_normal_names() {
        assert!(is_safe_path_segment("my-album"));
        assert!(is_safe_path_segment("photo.jpg"));
        assert!(is_safe_path_segment("album_2024"));
        assert!(is_safe_path_segment(".hidden"));
    }

    #[test]
    fn safe_path_segment_rejects_traversal() {
        assert!(!is_safe_path_segment(".."));
        assert!(!is_safe_path_segment("../etc"));
        assert!(!is_safe_path_segment("foo/bar"));
        assert!(!is_safe_path_segment("."));
        assert!(!is_safe_path_segment(""));
    }

}
