# Kuvasivu - Photography Portfolio

## Build & Run
- `cargo build` — compile
- `cargo run` — start server on port 3000

## Architecture
- Rust/Axum backend with askama HTML templates
- Filesystem-based album management: each album is a directory under `photos/`
- Album metadata in `album.toml` (title, description, optional timespan)
- Thumbnails generated on-demand via `image` crate, cached to `.thumbs/` inside each album dir
- EXIF date extraction via `kamadak-exif` for auto-deriving album timespans

## Conventions
- Keep it simple: no database, no JS frameworks
- Progressive enhancement: works without JS, CSS enhances layout
- Photo filenames are URL-safe (lowercase, hyphens)
- Templates use askama (Jinja2-like syntax)
