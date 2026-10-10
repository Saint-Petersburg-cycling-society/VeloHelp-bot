#!/bin/sh
set -eu

IMAGE="${IMAGE:-localhost/bike-aid:latest}"

exec podman build \
    --file Containerfile \
    --tag "$IMAGE" \
    .
