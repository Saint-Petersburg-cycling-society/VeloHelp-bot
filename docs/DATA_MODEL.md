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

### Проверенное поведение пилота (2026-10-10)

Миграция 0003 добавляет `dispatches.request_version`; 0004 наполняет каталог
18 районов Петербурга. Район заявки определяется локально по отправленной
точке и OSM-границам из `assets/areas/`; помощник выбирает районы явно. Новая wave переиспользует `(request_id, helper_id)`,
но увеличивает request version и сбрасывает metadata доставки только для новой версии.
Доменная заявка и её tags вставляются одной транзакцией. Renewed consent обновляет
и версию, и время подтверждения. Удаление requester удаляет его заявки; удаление
matched helper отменяет пару, обнуляет ссылку и координаты, затем удаляет identity.

### Оценка прибытия: миграция 0005

`help_requests.arrival_from_at` и `arrival_until_at` — nullable Unix seconds UTC.
CHECK требует обе границы или обе NULL и `from <= until`. На приёме границы
вычисляются из успешного `matched_at` и явно выбранных 1–240 минут, в той же
транзакции что winner. Purpose: координация активной помощи. Retention: только
до resolve/cancel/expire/withdraw/deletion; startup очищает остатки закрытых заявок.

Requester получает эти поля со своей заявкой в JSON export; helper получает
только собственные оценки в `arrival_estimates`, без чужой точной точки.
Имена и вычисленные адреса в доменной БД не сохраняются. Адрес — публичный
ориентир из встроенного ODbL snapshot, а не история геокодирования пользователей.
Upgrade с 0004 оставляет существующие пары и точки, заполняя ETA значением NULL;
повторное применение миграций проверено.

### Примерное место в приглашении

ApproximateLocation — производная проекция, не новое persisted поле. Координаты
округляются к центру фиксированной сетки: широта с scale 111195 м/градус, долгота
с фиксированным scale 55597.5 м/градус (опорная широта Петербурга 60°), размер ячейки
1000 м. Scale не вычисляется из точной широты: это могло бы раскрыть лишние данные.
Для invalid point карта не создаётся. Статус/TTL/участник проверяются при проекции.
Результат не зависит от helper ID, request version, времени или случайных чисел.
Миграция не требуется; исходная точка сохраняет прежний purpose и retention.

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

### Verified pilot behavior (2026-10-10)

Migration 0003 adds `dispatches.request_version`; 0004 seeds 18 Saint Petersburg
districts. Request districts are resolved locally from the submitted point and
OSM boundaries in `assets/areas/`; helpers explicitly choose their service areas.
A new wave reuses `(request_id, helper_id)` but advances request version and resets
delivery metadata only for the newer version. Requests and tags are inserted in one
transaction. Renewed consent updates both version and acceptance time. Requester
deletion removes their requests; deleting a matched helper cancels the match, clears
its identity reference and exact point, then removes the user.

### Arrival estimate: migration 0005

`help_requests.arrival_from_at` and `arrival_until_at` are nullable UTC Unix seconds.
A CHECK requires both bounds or both NULL and `from <= until`. Acceptance derives
bounds from the successful `matched_at` and the explicitly selected 1–240 minutes,
in the same transaction as the winner. Purpose: coordinate active assistance.
Retention: until resolve/cancel/expire/withdraw/deletion; startup clears residues
on closed requests.

Requesters export these fields with their own requests; helpers export only their
own `arrival_estimates`, without another user’s exact point. Names and computed
addresses are not stored in the domain DB. An address is a public landmark from
the embedded ODbL snapshot, not a user geocoding history. Upgrade from 0004 retains
existing pairs and points with NULL ETA; migration replay is tested.

### Approximate invitation location

ApproximateLocation is a derived projection, not a new persisted field. Coordinates
are quantized to a fixed cell center: latitude uses 111195 m/degree, longitude uses
a fixed 55597.5 m/degree (Saint Petersburg reference latitude 60°), with 1000 m cells.
The scale is never derived from the exact latitude, which could disclose additional
data. Invalid points yield no map. Projection checks status/TTL/viewer. The result
does not depend on helper ID, request version, time or randomness. No migration is
needed; the source point retains its existing purpose and retention.
