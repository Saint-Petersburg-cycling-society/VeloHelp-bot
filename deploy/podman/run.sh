#!/bin/sh
set -eu

IMAGE="${IMAGE:-localhost/bike-aid:latest}"
ENV_FILE="${ENV_FILE:-$HOME/.config/bike-aid/bike-aid.env}"
DATA_DIR="${DATA_DIR:-$HOME/.local/share/bike-aid}"

mkdir -p "$DATA_DIR"

exec podman run \
    --rm \
    --name bike-aid \
    --read-only \
    --cap-drop=all \
    --security-opt=no-new-privileges \
    --user 65532:65532 \
    --userns=keep-id:uid=65532,gid=65532 \
    --env-file "$ENV_FILE" \
    --volume "$DATA_DIR:/data:Z" \
    "$IMAGE"
