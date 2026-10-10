# Architecture / Архитектура

## RU

### Граница frontend/backend

```text
Telegram
   ↓
telegram/ + ui/                 FRONTEND
   ↓ typed command/event
application/ + domain/          BACKEND
   ↓
storage/ SQLite
```

Frontend отвечает за ввод, отображение, `Surface`, `ViewId`, `Revision`,
capabilities и projection. Backend отвечает за доменные инварианты, matching и
авторитетные транзакции.

`ui/` не импортирует SQLx. `domain/` не импортирует teloxide.

### Accept

```text
callback
→ resolve actor/stale capability
→ application::accept(request_id, helper_id)
→ atomic conditional UPDATE
→ commit
→ domain outcome
→ reproject requester + N dispatch views
```

Никакого Telegram I/O внутри DB transaction.

### Один доменный объект — много views

```text
HelpRequest #42
├─ requester view
├─ helper A dispatch view
├─ helper B dispatch view
└─ helper C dispatch view
```

Views имеют независимые revisions и surfaces.

### SQLite

Single-host MVP использует SQLite/WAL на локальной FS. Начальный pool — один
connection. Никаких сетевых filesystem для live DB.

PostgreSQL становится оправданным при нескольких application hosts, отдельном
сетевом DB service или измеренной write-конкуренции, которую single-writer
SQLite уже не обслуживает.

## EN

### Frontend/backend boundary

Telegram adaptation and `teloxide-ui` form the frontend. Domain, application
services and SQLite persistence form the backend. UI code never performs SQL
directly; domain code does not depend on Telegram types.

### Accept

The accept command is an atomic conditional database write followed by
post-commit projection. No Telegram I/O occurs inside the transaction.

### Multi-view model

One domain request may have many independent Telegram views. They share domain
truth but not UI revision identity.

### SQLite

The single-host MVP uses SQLite/WAL on a local filesystem with one pooled
connection initially. PostgreSQL becomes appropriate when the application
requires multiple hosts, a remote DB service or measured write concurrency
beyond SQLite's single-writer model.


## Runtime isolation / Изоляция runtime

### RU

Production boundary: rootless Podman + scratch image. Контейнер содержит только
статически собранный application executable; SQLite state приходит отдельным
writable mount. У приложения нет причины иметь shell или general-purpose
userspace.

### EN

Production uses a rootless Podman scratch image containing only the compiled
application executable. SQLite state is a separate writable mount; the
application has no runtime dependency on a shell or general-purpose userspace.
