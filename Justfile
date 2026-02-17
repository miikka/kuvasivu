default: run

docker-build:
    docker build -t kuvasivu .

docker-run: docker-build
    docker run --rm -p 3000:3000 \
      -v ./site.toml:/data/site.toml \
      -v ./photos:/data/photos \
      kuvasivu

lint:
    cargo clippy
    shellcheck tests/docker_test.sh

run:
    cargo run

test:
    cargo llvm-cov --show-missing-lines
    ./tests/docker_test.sh

test-cov:
    cargo llvm-cov --html

open-cov:
    open target/llvm-cov/html/index.html
