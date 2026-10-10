# Deployment / Деплой

## RU

### Runtime contract

Production deployment — **rootless Podman**, multi-stage build:

```text
Alpine + Rust + musl + compiler toolchain    builder only
                    ↓
             static-ish musl ELF
                    ↓
                FROM scratch                 runtime
                    ↓
          /bike-aid + mounted /data
```

В final image нет:
- shell;
- package manager;
- libc shared objects;
- coreutils;
- sqlite CLI;
- CA bundle;
- `/etc/passwd`.

Есть один executable. TLS roots встроены в binary через `webpki-root-certs`.
`teloxide::Bot` получает custom `reqwest::Client` через `Bot::with_client`.

SQLite также bundled/static через SQLx `sqlite`, поэтому системная `libsqlite3`
runtime не требуется.

### Почему не Alpine runtime

Alpine нужен только как builder. Оставлять его final layer означало бы добавить
apk/busybox/musl userspace, которым приложение не пользуется.

`scratch` уменьшает runtime attack surface и убирает класс уязвимостей в
неиспользуемых userspace packages.

Это не “виртуальная машина без ОС”: container всё ещё использует kernel host.
Изоляцию дают rootless Podman, user namespace, OCI namespaces/seccomp/cgroups и
ограничения container config.

### Build

На ARM64-хосте:

```sh
podman build -f Containerfile -t localhost/bike-aid:latest .
```

Alpine builder собирает native `aarch64-unknown-linux-musl`.
На x86_64 аналогично получится `x86_64-unknown-linux-musl`.

Для release после появления `Cargo.lock` сборка обязана использовать
`cargo build --release --locked`.

### Runtime hardening

Baseline:

```text
rootless Podman
FROM scratch
USER 65532:65532
read-only rootfs
drop ALL capabilities
no-new-privileges
no published ports
only /data writable
```

Long polling требует только исходящих HTTPS-соединений; inbound port не нужен.

### Raw run

```sh
mkdir -p ~/.config/bike-aid ~/.local/share/bike-aid
cp .env.example ~/.config/bike-aid/bike-aid.env
$EDITOR ~/.config/bike-aid/bike-aid.env

podman run \
  --rm \
  --name bike-aid \
  --read-only \
  --cap-drop=all \
  --security-opt=no-new-privileges \
  --user 65532:65532 \
  --userns=keep-id:uid=65532,gid=65532 \
  --env-file ~/.config/bike-aid/bike-aid.env \
  --volume ~/.local/share/bike-aid:/data:Z \
  localhost/bike-aid:latest
```

### Quadlet

На systemd-хосте rootless Quadlet кладётся в:

```text
~/.config/containers/systemd/bike-aid.container
```

После чего:

```sh
systemctl --user daemon-reload
systemctl --user enable --now bike-aid.service
```

Для старта без пользовательской login-session обычно включается linger для
отдельного service account.

На OpenWrt/systemd-less host контейнер запускается/рестартуется host init
системой через Podman CLI; сам application binary на host напрямую не
устанавливается.

### SQLite

Live database:

```text
/data/bike-aid.sqlite
```

Контейнеру writable только `/data`.

SQLite:
- WAL;
- synchronous FULL;
- foreign keys ON;
- busy timeout 5s;
- один connection в MVP.

### Backup

`sqlite3` не тащим в container. Это host-side maintenance tool.

Нельзя raw-copy открытый WAL DB. Использовать online backup API / `.backup`.

Таймер: `deploy/systemd/bike-aid-backup.service` + `bike-aid-backup.timer`
(daily, `Persistent=true`). Restore проверяется `tests/backup_restore.rs`
(`VACUUM INTO` + `PRAGMA integrity_check` + row count).

### Updates

```text
git pull
→ podman build new immutable image
→ run tests/smoke
→ replace container
```

Никакого `apk upgrade` внутри production image не существует.

TLS root set обновляется вместе с binary/container rebuild, поскольку
`webpki-root-certs` compiled in.

## EN

### Runtime contract

Production deployment is **rootless Podman** with a multi-stage build:

```text
Alpine + Rust + musl + compiler toolchain    builder only
                    ↓
             static-ish musl ELF
                    ↓
                FROM scratch                 runtime
                    ↓
          /bike-aid + mounted /data
```

The final image contains no shell, package manager, libc shared objects,
coreutils, sqlite CLI, CA bundle, or `/etc/passwd`.

There is a single executable. TLS roots are compiled into the binary via
`webpki-root-certs`. `teloxide::Bot` receives a custom `reqwest::Client`
through `Bot::with_client`.

SQLite is bundled/static via SQLx `sqlite`, so no system `libsqlite3`
runtime is required.

### Why not an Alpine runtime

Alpine is builder-only. Keeping it as a final layer would add
apk/busybox/musl userspace that the application never uses.

`scratch` reduces the runtime attack surface and removes the vulnerability
class in unused userspace packages.

This is not an “OS-less VM”: the container still uses the host kernel.
Isolation comes from rootless Podman, user namespaces, OCI
namespaces/seccomp/cgroups, and container config limits.

### Build

On an ARM64 host:

```sh
podman build -f Containerfile -t localhost/bike-aid:latest .
```

The Alpine builder produces a native `aarch64-unknown-linux-musl` binary;
on x86_64 it produces `x86_64-unknown-linux-musl`.

For releases, once `Cargo.lock` exists, the build MUST use
`cargo build --release --locked`.

### Runtime hardening

Baseline:

```text
rootless Podman
FROM scratch
USER 65532:65532
read-only rootfs
drop ALL capabilities
no-new-privileges
no published ports
only /data writable
```

Long polling requires only outbound HTTPS; no inbound port is needed.

### Raw run

```sh
mkdir -p ~/.config/bike-aid ~/.local/share/bike-aid
cp .env.example ~/.config/bike-aid/bike-aid.env
$EDITOR ~/.config/bike-aid/bike-aid.env

podman run \
  --rm \
  --name bike-aid \
  --read-only \
  --cap-drop=all \
  --security-opt=no-new-privileges \
  --user 65532:65532 \
  --userns=keep-id:uid=65532,gid=65532 \
  --env-file ~/.config/bike-aid/bike-aid.env \
  --volume ~/.local/share/bike-aid:/data:Z \
  localhost/bike-aid:latest
```

### Quadlet

On a systemd host the rootless Quadlet is placed at:

```text
~/.config/containers/systemd/bike-aid.container
```

Then:

```sh
systemctl --user daemon-reload
systemctl --user enable --now bike-aid.service
```

Linger is normally enabled for a dedicated service account so the service
starts without a user login session.

On OpenWrt/systemd-less hosts the container is started/restarted by the host
init system via the Podman CLI; the application binary MUST NOT be installed
directly on the host rootfs.

### SQLite

Live database:

```text
/data/bike-aid.sqlite
```

Only `/data` is writable to the container.

SQLite:
- WAL;
- synchronous FULL;
- foreign keys ON;
- busy timeout 5s;
- one connection in the MVP.

### Backup

Do not bring `sqlite3` into the container. It is a host-side maintenance tool.

Never raw-copy an open WAL DB. Use the online backup API / `.backup`.

Timer: `deploy/systemd/bike-aid-backup.service` + `bike-aid-backup.timer`
(daily, `Persistent=true`). Restore is covered by `tests/backup_restore.rs`
(`VACUUM INTO` + `PRAGMA integrity_check` + row count).

### Updates

```text
git pull
→ podman build new immutable image
→ run tests/smoke
→ replace container
```

There is no `apk upgrade` inside the production image.

The TLS root set is updated together with the binary/container rebuild,
because `webpki-root-certs` is compiled in.

Containers still use the host kernel; `scratch` removes image userspace rather
than removing the operating system kernel.
