# SPDX-FileCopyrightText: 2026 Miikka Koskinen
#
# SPDX-License-Identifier: MIT

default: run

docker-build:
    docker build -t kuvasivu .

docker-run: docker-build
    docker run --rm -p 3000:3000 --init \
      -v ./site.toml:/data/site.toml \
      -v ./photos:/data/photos \
      kuvasivu

# CI also runs the slow ignored tests (e.g. AVIF encoding).
ci: (test "--run-ignored" "all") lint

lint:
    cargo fmt --check
    cargo clippy
    shellcheck tests/docker_test.sh

run:
    cargo run

test *args:
    cargo llvm-cov nextest --show-missing-lines {{args}}
    ./tests/docker_test.sh

# AVIF encoding tests are slow in debug builds, so they're skipped by default.
test-avif:
    cargo nextest run --run-ignored only avif

test-cov:
    cargo llvm-cov --html

open-cov:
    open target/llvm-cov/html/index.html
