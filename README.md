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

Thumbnails are generated on-demand and cached in `.thumbs/` inside each album directory.

## Configuration

Site-wide settings live in `site.toml`:

```toml
title = "My Portfolio"
```

## License

See [LICENSE](LICENSE).
