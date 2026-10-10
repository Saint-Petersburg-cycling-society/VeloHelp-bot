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

Production uses a rootless Podman container built in Alpine/musl and executed
from `FROM scratch`.

The final root filesystem contains the application executable only; writable
state is a mounted `/data` directory. There is no shell, package manager,
shared libc, SQLite CLI, CA bundle or general-purpose userspace.

TLS Mozilla roots are compiled into the executable and passed to reqwest using
`tls_certs_only`; Teloxide receives that client through `Bot::with_client`.
SQLx's SQLite feature bundles and statically links SQLite.

The runtime baseline is rootless Podman, numeric non-root user, read-only
rootfs, all capabilities dropped, no-new-privileges, no published ports and a
single writable data mount.

Containers still use the host kernel; `scratch` removes image userspace rather
than removing the operating system kernel.
