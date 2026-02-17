FROM rust:1-bookworm AS builder

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY templates/ templates/

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates tini \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /build/target/release/kuvasivu .
COPY static/ static/

EXPOSE 3000

# Mount site.toml and photos/ at runtime:
#   -v /path/to/site.toml:/app/site.toml
#   -v /path/to/photos:/app/photos

ENTRYPOINT ["tini", "--"]
CMD ["./kuvasivu"]
