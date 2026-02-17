# Kuvasivu

A photography portfolio site: put some JPEGs into a directory, receive a gallery.


## Quick Start

```
cargo run
```

The server starts on port 3000.

## Adding Albums

Each album is a directory under `photos/`. Add a JPEG/PNG/WebP image and an `album.toml`:

```
photos/
  my-album/
    album.toml
    photo-one.jpg
    photo-two.jpg
```

`album.toml` example:

```toml
title = "My Album"
description = "A short description."
timespan = "January 2026"   # optional, auto-derived from EXIF if omitted
```

Thumbnails are generated on-demand and cached in a separate cache directory.

## Configuration

Site-wide settings live in `site.toml`:

```toml
title = "My Portfolio"
```

### Environment Variables

| Variable | Default | Description |
|---|---|---|
| `KUVASIVU_DATA_DIR` | `.` | Directory containing `site.toml` and `photos/` |
| `KUVASIVU_CACHE_DIR` | `{data_dir}/cache` | Directory for generated thumbnails |

## Docker

```
docker run -p 3000:3000 -v /path/to/data:/data:ro -v kuvasivu-cache:/cache kuvasivu
```

The data volume (`/data`) can be mounted read-only. Thumbnails are written to a separate `/cache` volume.

## License

See [LICENSE](LICENSE).
