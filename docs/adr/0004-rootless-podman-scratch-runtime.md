# ADR-0004: Rootless Podman + scratch runtime / Rootless Podman + scratch

Status: accepted / принято — 2026-10-03

## RU

### Решение

Production:
- build stage: Alpine + Rust 1.88 + musl + C toolchain;
- runtime: `FROM scratch`;
- rootless Podman;
- process UID/GID `65532`;
- read-only rootfs;
- `CAP_DROP=ALL`;
- no-new-privileges;
- `/data` — единственный writable mount;
- входящие порты не публикуются при long polling.

TLS root certificates встраиваются в binary через `webpki-root-certs`, потому
что reqwest 0.13 rustls по умолчанию использует platform verifier, а у scratch
нет системного CA store.

`teloxide::Bot::with_client` получает явно сконфигурированный reqwest client с
`tls_certs_only`.

SQLx `sqlite` использует bundled SQLite, поэтому runtime `.so` для SQLite не
нужна.

### Следствия

Production image нельзя “чинить” через shell внутри контейнера — shell отсутствует.
Для диагностики используется host tooling или отдельный debug image.

Обновление CA roots = rebuild/redeploy binary.

## EN

Build in Alpine/musl and run a single executable in a rootless Podman
`FROM scratch` image. The root filesystem is read-only, the process is numeric
non-root, all capabilities are dropped, no-new-privileges is enabled and
`/data` is the only writable mount.

Reqwest 0.13 normally uses the platform verifier, which has no CA store in a
scratch image. Mozilla roots are therefore compiled into the executable using
`webpki-root-certs` and supplied through `tls_certs_only`; Teloxide receives
that client through `Bot::with_client`.

SQLx's SQLite driver bundles SQLite, so no runtime SQLite shared library or CLI
is needed.
