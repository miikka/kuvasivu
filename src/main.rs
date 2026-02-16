use std::path::{Path, PathBuf};
use std::sync::Arc;

use askama::Template;
use axum::extract::{self, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use image::imageops::FilterType;
use serde::Deserialize;
use tower_http::services::ServeDir;

enum AppError {
    Render(askama::Error),
    NotFound,
}

impl From<askama::Error> for AppError {
    fn from(err: askama::Error) -> Self {
        AppError::Render(err)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::Render(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Failed to render template").into_response()
            }
            AppError::NotFound => StatusCode::NOT_FOUND.into_response(),
        }
    }
}

const PHOTOS_DIR: &str = "photos";
const THUMB_DIR: &str = ".thumbs";
const SMALL_SIZE: u32 = 400;
const MEDIUM_SIZE: u32 = 1200;

#[derive(Clone)]
struct AppState {
    photos_dir: Arc<PathBuf>,
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

struct ExifInfo {
    camera: Option<String>,
    lens: Option<String>,
    focal_length: Option<String>,
    aperture: Option<String>,
    exposure: Option<String>,
    iso: Option<String>,
}

impl ExifInfo {
    fn summary(&self) -> String {
        let mut parts = Vec::new();

        if let Some(camera) = &self.camera {
            parts.push(camera.clone());
        }
        if let Some(lens) = &self.lens {
            parts.push(lens.clone());
        }

        let mut settings = Vec::new();
        if let Some(fl) = &self.focal_length {
            settings.push(fl.clone());
        }
        if let Some(ap) = &self.aperture {
            settings.push(format!("\u{192}/{}", ap));
        }
        if let Some(ex) = &self.exposure {
            settings.push(format!("{}s", ex));
        }
        if let Some(iso) = &self.iso {
            settings.push(format!("ISO {}", iso));
        }
        if !settings.is_empty() {
            parts.push(settings.join("  "));
        }

        parts.join(" · ")
    }
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    albums: Vec<Album>,
}

#[derive(Template)]
#[template(path = "album.html")]
struct AlbumTemplate {
    album: Album,
    photos: Vec<Photo>,
}

#[derive(Template)]
#[template(path = "photo.html")]
struct PhotoTemplate {
    album: Album,
    photo: Photo,
    prev: Option<Photo>,
    next: Option<Photo>,
    exif: ExifInfo,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let photos_dir = Arc::new(PathBuf::from(PHOTOS_DIR));
    std::fs::create_dir_all(photos_dir.as_ref()).ok();

    let state = AppState { photos_dir };

    let app = Router::new()
        .route("/", get(index))
        .route("/album/{slug}", get(album))
        .route("/album/{slug}/{filename}", get(photo))
        .route("/photos/{album}/{filename}", get(serve_photo))
        .route("/thumbs/{album}/{size}/{filename}", get(serve_thumb))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    tracing::info!("listening on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}

async fn index(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let albums = scan_albums(&state.photos_dir);
    Ok(Html((IndexTemplate { albums }).render()?))
}

async fn album(
    State(state): State<AppState>,
    extract::Path(slug): extract::Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let album_path = state.photos_dir.join(&slug);
    if !album_path.is_dir() {
        return Err(AppError::NotFound);
    }

    let meta = load_meta(&album_path);
    let photos = list_photos(&album_path);
    let cover = photos.first().map(|p| p.filename.clone());

    let album = Album {
        title: meta.title.unwrap_or_else(|| slug_to_title(&slug)),
        description: meta.description.unwrap_or_default(),
        timespan: meta
            .timespan
            .unwrap_or_else(|| derive_timespan(&album_path, &photos)),
        slug,
        cover,
    };

    Ok(Html((AlbumTemplate { album, photos }).render()?))
}

async fn photo(
    State(state): State<AppState>,
    extract::Path((slug, filename)): extract::Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let album_path = state.photos_dir.join(&slug);
    if !album_path.is_dir() {
        return Err(AppError::NotFound);
    }

    let meta = load_meta(&album_path);
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

    let cover = photos.first().map(|p| p.filename.clone());
    let album = Album {
        title: meta.title.unwrap_or_else(|| slug_to_title(&slug)),
        description: meta.description.unwrap_or_default(),
        timespan: meta
            .timespan
            .unwrap_or_else(|| derive_timespan(&album_path, &photos)),
        slug,
        cover,
    };

    let photo_path = album_path.join(&filename);
    let exif = read_exif_info(&photo_path);

    let photo = Photo {
        filename: filename.clone(),
    };

    Ok(Html(
        (PhotoTemplate {
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
    let path = state.photos_dir.join(&album).join(&filename);
    serve_file(&path).await
}

async fn serve_thumb(
    State(state): State<AppState>,
    extract::Path((album, size, filename)): extract::Path<(String, String, String)>,
) -> Result<impl IntoResponse, StatusCode> {
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

    let thumb_dir = album_path.join(THUMB_DIR).join(&size);
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

    Ok(([(axum::http::header::CONTENT_TYPE, content_type)], body))
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
        let meta = load_meta(&path);
        let photos = list_photos(&path);
        let cover = photos.first().map(|p| p.filename.clone());

        albums.push(Album {
            title: meta.title.unwrap_or_else(|| slug_to_title(&slug)),
            description: meta.description.unwrap_or_default(),
            timespan: meta
                .timespan
                .unwrap_or_else(|| derive_timespan(&path, &photos)),
            slug,
            cover,
        });
    }

    albums.sort_by(|a, b| a.title.cmp(&b.title));
    albums
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
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn derive_timespan(album_path: &Path, photos: &[Photo]) -> String {
    let mut dates: Vec<String> = Vec::new();

    for photo in photos {
        if let Some(date) = read_exif_date(&album_path.join(&photo.filename)) {
            dates.push(date);
        }
    }

    if dates.is_empty() {
        return String::new();
    }

    dates.sort();
    let first = &dates[0];
    let last = &dates[dates.len() - 1];

    let first_month = format_year_month(first);
    let last_month = format_year_month(last);

    if first_month == last_month {
        first_month
    } else {
        format!("{} – {}", first_month, last_month)
    }
}

fn read_exif_info(path: &Path) -> ExifInfo {
    let get_info = || -> Option<ExifInfo> {
        let file = std::fs::File::open(path).ok()?;
        let mut bufreader = std::io::BufReader::new(file);
        let exif = exif::Reader::new()
            .read_from_container(&mut bufreader)
            .ok()?;

        let get = |tag: exif::Tag| -> Option<String> {
            let field = exif.get_field(tag, exif::In::PRIMARY)?;
            let val = field.display_value().to_string().trim_matches('"').to_string();
            if val.is_empty() { None } else { Some(val) }
        };

        let camera = match (get(exif::Tag::Make), get(exif::Tag::Model)) {
            (Some(make), Some(model)) => {
                if model.starts_with(&make) {
                    Some(model)
                } else {
                    Some(format!("{} {}", make, model))
                }
            }
            (None, Some(model)) => Some(model),
            (Some(make), None) => Some(make),
            (None, None) => None,
        };

        Some(ExifInfo {
            camera,
            lens: get(exif::Tag::LensModel),
            focal_length: get(exif::Tag::FocalLength),
            aperture: get(exif::Tag::FNumber),
            exposure: get(exif::Tag::ExposureTime),
            iso: get(exif::Tag::PhotographicSensitivity),
        })
    };

    get_info().unwrap_or(ExifInfo {
        camera: None,
        lens: None,
        focal_length: None,
        aperture: None,
        exposure: None,
        iso: None,
    })
}

fn read_exif_date(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let mut bufreader = std::io::BufReader::new(file);
    let exif = exif::Reader::new()
        .read_from_container(&mut bufreader)
        .ok()?;
    let field = exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)?;
    Some(field.display_value().to_string())
}

fn format_year_month(datetime_str: &str) -> String {
    // EXIF date format: "2024-06-15 12:00:00" or "2024:06:15 12:00:00"
    let parts: Vec<&str> = datetime_str
        .split(|c| c == '-' || c == ':' || c == ' ')
        .collect();
    if parts.len() >= 2 {
        let year = parts[0];
        let month_num: u32 = parts[1].parse().unwrap_or(0);
        let month_name = match month_num {
            1 => "January",
            2 => "February",
            3 => "March",
            4 => "April",
            5 => "May",
            6 => "June",
            7 => "July",
            8 => "August",
            9 => "September",
            10 => "October",
            11 => "November",
            12 => "December",
            _ => return datetime_str.to_string(),
        };
        format!("{} {}", month_name, year)
    } else {
        datetime_str.to_string()
    }
}
