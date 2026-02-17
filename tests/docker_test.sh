#!/usr/bin/env bash
set -euo pipefail

IMAGE="kuvasivu-test"
CONTAINER="kuvasivu-test-$$"
DATA_DIR="$(mktemp -d)"

cleanup() {
    docker rm -f "$CONTAINER" 2>/dev/null || true
    rm -rf "$DATA_DIR"
}
trap cleanup EXIT

# Set up test data
mkdir -p "$DATA_DIR/photos/test-album"
cat > "$DATA_DIR/site.toml" <<'EOF'
title = "Docker Test"
EOF
cat > "$DATA_DIR/photos/test-album/album.toml" <<'EOF'
title = "Test Album"
EOF

# Build the image
echo "Building Docker image..."
docker build -t "$IMAGE" .

# Start the container
echo "Starting container..."
docker run -d --name "$CONTAINER" -p 3099:3000 -v "$DATA_DIR:/data" "$IMAGE"

# Wait for the server to be ready
echo "Waiting for server..."
for i in $(seq 1 30); do
    if curl -sf http://localhost:3099/ > /dev/null 2>&1; then
        break
    fi
    if [ "$i" -eq 30 ]; then
        echo "FAIL: server did not start in time"
        docker logs "$CONTAINER"
        exit 1
    fi
    sleep 0.5
done

# Test: index page contains the site title
echo "Testing index page..."
BODY="$(curl -sf http://localhost:3099/)"
if echo "$BODY" | grep -q "Docker Test"; then
    echo "PASS: site title found"
else
    echo "FAIL: site title not found"
    echo "$BODY"
    exit 1
fi

# Test: index page contains the album
if echo "$BODY" | grep -q "Test Album"; then
    echo "PASS: album found on index"
else
    echo "FAIL: album not found on index"
    echo "$BODY"
    exit 1
fi

# Test: album page works
STATUS="$(curl -sf -o /dev/null -w '%{http_code}' http://localhost:3099/album/test-album)"
if [ "$STATUS" = "200" ]; then
    echo "PASS: album page returns 200"
else
    echo "FAIL: album page returned $STATUS"
    exit 1
fi

echo "All Docker tests passed."
