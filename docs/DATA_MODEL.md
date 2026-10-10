# Data model / Модель данных

## RU

### Identity

Стабильная внешняя identity — Telegram user ID (`INTEGER`/`i64`).
`username` и display name — только presentation.

### Основные таблицы

- `users` — минимальная identity + consent metadata;
- `helper_profiles` — настройки помощника;
- `areas` — data-driven иерархия территории;
- `helper_areas`, `helper_help_kinds` — matching preferences;
- `availability_windows` — необязательное расписание;
- `help_requests` — авторитетное состояние заявки;
- `request_problem_tags` — структурированные признаки;
- `dispatches` — fan-out и состояние уведомления конкретному helper.

UUID в SQLite хранятся как canonical text на storage boundary.

Время хранится как Unix seconds (`INTEGER`) в UTC.

### Exact location

Точные координаты purpose-limited. После закрытия заявки они должны быть
обнулены по retention policy. История перемещений пользователя не создаётся.

### Constraints

DB должна обеспечивать как минимум:
- allowed enum-like status values через `CHECK`;
- FK;
- невозможность `matched_helper_id == requester_id`;
- `matched` требует helper;
- валидные lat/lon;
- `expires_at > created_at`.

## EN

### Identity

The stable external identity is the Telegram user ID (`INTEGER`/`i64`).
`username` and display name are presentation only.

### Main tables

- `users` — minimal identity + consent metadata;
- `helper_profiles` — helper settings;
- `areas` — data-driven territory hierarchy;
- `helper_areas`, `helper_help_kinds` — matching preferences;
- `availability_windows` — optional schedule;
- `help_requests` — authoritative request state;
- `request_problem_tags` — structured traits;
- `dispatches` — fan-out and per-helper notification state.

UUIDs in SQLite are stored as canonical text at the storage boundary.

Time is stored as Unix seconds (`INTEGER`) in UTC.

### Exact location

Exact coordinates are purpose-limited. After the request is closed they MUST be
cleared per the retention policy. No user movement history is created.

### Constraints

The DB MUST enforce at least:
- allowed enum-like status values via `CHECK`;
- FK;
- impossibility of `matched_helper_id == requester_id`;
- `matched` requires a helper;
- valid lat/lon;
- `expires_at > created_at`.
