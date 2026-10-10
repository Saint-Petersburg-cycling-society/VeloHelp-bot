# syntax=docker/dockerfile:1

FROM docker.io/library/rust:1.88-alpine AS build

RUN apk add --no-cache \
    build-base \
    git \
    musl-dev

WORKDIR /src

COPY Cargo.toml Cargo.lock rust-toolchain.toml build.rs ./
COPY src ./src
COPY migrations ./migrations
COPY assets/icons/generated/mapping.json ./assets/icons/generated/mapping.json
COPY assets/areas/spb-districts.json ./assets/areas/spb-districts.json
COPY assets/areas/spb-addresses.json ./assets/areas/spb-addresses.json

# Cargo.lock is committed; release builds MUST be reproducible.
RUN cargo build --release --locked

RUN strip /src/target/release/bike-aid

FROM scratch

COPY --from=build /src/target/release/bike-aid /bike-aid

# Numeric identity; scratch intentionally contains no /etc/passwd.
USER 65532:65532

ENTRYPOINT ["/bike-aid"]
