FROM rust:1-bookworm AS builder

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY templates/ templates/

RUN cargo build --release

FROM debian:bookworm-slim

LABEL org.opencontainers.image.source=https://github.com/miikka/kuvasivu

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates tini \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /build/target/release/kuvasivu .
COPY static/ static/

EXPOSE 3000

ENV KUVASIVU_DATA_DIR=/data
ENV KUVASIVU_CACHE_DIR=/cache

# Mount your data directory at runtime:
#   -v /path/to/data:/data
# The data directory should contain site.toml and photos/
#
# Thumbnails are written to /cache (a separate volume):
#   -v kuvasivu-cache:/cache
VOLUME /cache

ENTRYPOINT ["tini", "--"]
CMD ["./kuvasivu"]
