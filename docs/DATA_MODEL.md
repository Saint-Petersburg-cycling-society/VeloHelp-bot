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

Telegram user ID is the stable external identity; username/display name are
presentation only.

The main tables are users, helper profiles, areas, availability, requests,
problem tags and per-helper dispatches. UUIDs are stored as canonical text at
the SQLite boundary and timestamps as UTC Unix seconds.

Exact coordinates are purpose-limited and must be removed according to the
retention policy. The schema enforces status/FK/range and basic match
invariants.
