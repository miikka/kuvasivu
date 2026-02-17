default: run

docker-build:
    docker build -t kuvasivu .

docker-run: docker-build
    docker run --rm -p 3000:3000 \
      -v ./site.toml:/data/site.toml \
      -v ./photos:/data/photos \
      kuvasivu

run:
    cargo run

test-cov:
    cargo llvm-cov --html

open-cov:
    open target/llvm-cov/html/index.html
