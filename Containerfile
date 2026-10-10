# syntax=docker/dockerfile:1

FROM docker.io/library/rust:1.88-alpine AS build

RUN apk add --no-cache \
    build-base \
    git \
    musl-dev

WORKDIR /src

COPY Cargo.toml rust-toolchain.toml build.rs ./
COPY src ./src
COPY migrations ./migrations

# Cargo.lock is intentionally expected to be committed once generated.
# Use --locked in CI/release as soon as the first lockfile exists.
RUN cargo build --release

RUN strip /src/target/release/bike-aid

FROM scratch

COPY --from=build /src/target/release/bike-aid /bike-aid

# Numeric identity; scratch intentionally contains no /etc/passwd.
USER 65532:65532

ENTRYPOINT ["/bike-aid"]
