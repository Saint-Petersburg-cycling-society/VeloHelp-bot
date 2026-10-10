# AGENTS.md / Правила для агентов

This file is normative. RU and EN sections describe the same contract.
If they diverge, fix both in the same change.

---

## RU

### 1. Главный архитектурный инвариант

Проект разделён на условный **backend** и **frontend**.

Backend:
- `domain/`;
- `application/`;
- `storage/`;
- matching, транзакции, авторитетное состояние.

Frontend:
- `ui/`;
- `telegram/`;
- `teloxide-ui`;
- rendering, sessions, input adaptation, Telegram surfaces.

Запрещено:
- UI напрямую читает/пишет SQL;
- backend импортирует Telegram-типы ради бизнес-логики;
- `teloxide-ui::UiStore` решает, кто выиграл заявку;
- Telegram request выполняется внутри DB transaction.

`Open -> Matched` определяется только авторитетной доменной БД.

### 2. Переиспользование — цель по умолчанию

При проектировании UI сначала ищи **общий reusable component**, а не одноразовый
экран.

Плохо:

```text
BicycleDistrictSelector
BicycleProblemToggleRow
BikeAidConfirmationButtons
```

Хорошо:

```text
MultiSelect
HierarchicalMultiSelect
Wizard
Confirm
RequestCard
StatusBadge
ActionBar
```

Domain-specific composition остаётся в `bike-aid`, но механика, применимая к
другим Telegram-приложениям, должна становиться кандидатом на перенос в MIT
`teloxide-ui`.

Не выноси в `teloxide-ui` AGPL-код копированием. Общий primitive реализуется
отдельно и чисто.

### 3. Запрет на “велосипеды” в generic UI

Не создавай generic/frontend primitive, в API которого зашита сущность
велосипеда. Никаких `Bike*`, `Bicycle*`, `WheelSizeSelector` и подобных типов в
универсальном UI-слое, если это можно выразить общим компонентом.

Также **не используй Unicode-эмодзи велосипеда/велосипедиста (`🚲`, `🚴` и
варианты) как финальные UI-иконки**. Вообще не подменяй дизайн случайными
системными emoji.

Текст интерфейса может содержать слово «велосипед», когда оно нужно по смыслу.

### 4. Кастомные иконки обязательны

Для всех устойчивых UI-сущностей делается собственный согласованный набор
иконок:
- home/navigation;
- request/help;
- helper;
- location/area;
- technical problem categories;
- accident;
- accept/decline/cancel/resolve;
- status;
- privacy/data;
- source/about/settings.

Source assets хранятся в `assets/icons/src/`.
Экспорт/Telegram custom-emoji mapping — в `assets/icons/generated/` и
`assets/icons/manifest.toml`.

Новая кнопка с семантической иконкой без записи в icon manifest — незавершённая
работа.

Нельзя тянуть случайные иконки из внешнего набора только ради скорости.
Fallback text/alternative text должен быть доступным и не нести единственный
смысл действия.

### 5. Docs == code

**Запрещено рассинхронизировать документацию и код.**

Если изменение меняет:
- state machine;
- SQL schema;
- invariant;
- config/env;
- UI flow;
- deployment;
- privacy/retention;
- public API;
- feature status,

то соответствующая документация меняется **в том же PR/коммите**.

Запрещены фразы в коде/PR вида:
- “docs later”;
- “TODO update spec” после merge;
- “implementation is source of truth” как оправдание устаревшей спеки.

Если код и docs противоречат друг другу — изменение не готово.

### 6. Документация двуязычная

Пользовательская и архитектурная документация ведётся **RU + EN**.

Предпочтительный формат одного `.md`:

```text
## RU
...

## EN
...
```

или два явно связанных файла `*.ru.md` / `*.en.md`, если документ большой.

При смысловом изменении обновляются обе версии одновременно.
Не допускается машинно выглядящая EN/RU версия, которая теряет инварианты,
ограничения или MUST/MUST NOT.

Исключение: юридический исходный текст/цитата может оставаться на языке
оригинала, но рядом должно быть двуязычное инженерное резюме.

### 7. Domain vs UI state

Domain:
- `HelpRequest`;
- `HelperProfile`;
- `Dispatch`;
- winner;
- request status;
- coarse area/exact point policy.

UI/session:
- current wizard step;
- temporary selections;
- `ViewId`;
- `Revision`;
- `Surface`;
- capability tokens.

Нельзя хранить доменную истину только в callback data или UI store.

### 8. Concurrency

- `Accept` — atomic conditional write.
- Не делай `SELECT status` → network/logic → `UPDATE status`.
- Write transaction должна быть короткой.
- Никаких Telegram/API calls под DB lock.
- Старый callback не может обратить новый domain state.
- Проигравший гонку helper получает нормальный domain outcome, а не panic.

### 9. SQLite contract

MVP storage: SQLite.

Required:
- WAL;
- `synchronous=FULL`;
- `busy_timeout >= 5s`;
- foreign keys enabled;
- DB file only on local filesystem;
- no NFS/SMB/network filesystem for live DB;
- no raw `cp` backup of an open WAL database.

Начальный pool — один connection. Увеличивать только после измерений и
concurrency tests.

Если появляется несколько application hosts, SQLite не “растягивается” по
сети: оформляется ADR о миграции на PostgreSQL или другой client/server DB.


### 10. Production runtime

Production packaging MUST be container-first:

```text
Alpine/musl builder -> FROM scratch -> rootless Podman
```

Required:
- final image без shell/package manager/general userspace;
- numeric non-root process;
- rootless Podman;
- read-only rootfs;
- drop all Linux capabilities;
- no-new-privileges;
- никаких published ports при long polling;
- writable только `/data`;
- CA roots compiled into binary, не системный trust store;
- application binary не устанавливается непосредственно в host rootfs.

Не добавляй Alpine/Debian runtime layer “для удобства отладки”.
Отладка выполняется отдельным debug image/toolbox, production image остаётся
`scratch`.

### 10. Privacy and safety

- exact requester point не попадает в fan-out notification;
- до match helper видит coarse area;
- нет постоянного helper tracking в MVP;
- не логируй точные координаты по умолчанию;
- Telegram ID — identity, username — presentation;
- каждый новый persisted personal-data field требует purpose + retention;
- emergency/medical/legal flows не изображают профессиональную службу.

### 11. License boundary

`bike-aid`: AGPL-3.0-only.
`teloxide-ui`: MIT.

Не переносить AGPL application/domain code в MIT repo копипастой.
Generic idea/primitive реализуется заново в правильной границе.

Публичная модифицированная инстанция должна иметь видимый путь к Corresponding
Source согласно AGPL.

### 12. Quality gate

Перед merge:
- `cargo fmt --all -- --check`;
- `cargo clippy --all-targets --all-features -- -D warnings`;
- `cargo test --all-features`;
- миграции применяются на пустой DB;
- migration/upgrade path проверен;
- docs RU/EN синхронны;
- новые UI semantics имеют custom icon или явно документированное исключение;
- race tests для затронутых transitions;
- никаких новых `unsafe`.

---

## EN

### 1. Primary architecture invariant

Treat the system as a **backend/frontend** split.

Backend owns domain/application/storage, matching, transactions and
authoritative state. Frontend owns Telegram adaptation, `teloxide-ui`, rendering,
sessions and surfaces.

UI must not access SQL directly. Backend business logic must not depend on
Telegram types. `teloxide-ui::UiStore` must never decide the winner of a help
request. Never hold a database transaction across a Telegram/network request.

### 2. Reuse is a first-class goal

Prefer reusable interaction components over bicycle-specific one-offs.
Generic mechanics that are useful outside this application are candidates for
the MIT `teloxide-ui` project.

Do not copy AGPL code into the MIT repository. Extract the invariant and
implement a clean generic primitive there.

### 3. No bicycle-shaped generic primitives

Do not put bicycle domain concepts into generic UI APIs when a generic
component can express the same interaction. Avoid `Bike*`/`Bicycle*` generic
component names.

Do not use Unicode bicycle/cyclist emoji (`🚲`, `🚴`, variants) as final UI
icons. Text may of course mention bicycles when semantically required.

### 4. Custom icon system

Stable UI concepts require project-owned custom icons. Sources, generated
exports and Telegram custom-emoji IDs must be tracked by the icon manifest.
A semantic button without an icon-manifest decision is incomplete.

### 5. Documentation and code are atomic

A behavior/schema/config/API/UI/deployment/privacy change must update the
affected documentation in the same PR/commit. “Docs later” is not acceptable.
If code and docs disagree, the change is incomplete.

### 6. Documentation is bilingual

Maintain public and architectural documentation in Russian and English.
Semantic changes update both versions together.

### 7. State boundaries

Domain truth lives in domain storage. Wizard/view/callback state lives in the
UI/session layer. Callback data is never authoritative application state.

### 8. Concurrency

Use short atomic conditional writes. Do not read a status and later blindly
write it. Do not hold DB locks across network I/O. Stale callbacks must not
reverse newer domain state.

### 9. SQLite contract

The MVP uses SQLite on a local filesystem with WAL, FULL synchronous mode,
foreign keys and a busy timeout. Start with one pooled connection. Do not place
the live DB on a network filesystem and do not raw-copy an open WAL database.

Multiple application hosts require a database architecture ADR, normally
migration to PostgreSQL/client-server storage.


### 10. Production runtime

Production packaging MUST be container-first:

```text
Alpine/musl builder -> FROM scratch -> rootless Podman
```

Required:
- final image без shell/package manager/general userspace;
- numeric non-root process;
- rootless Podman;
- read-only rootfs;
- drop all Linux capabilities;
- no-new-privileges;
- никаких published ports при long polling;
- writable только `/data`;
- CA roots compiled into binary, не системный trust store;
- application binary не устанавливается непосредственно в host rootfs.

Не добавляй Alpine/Debian runtime layer “для удобства отладки”.
Отладка выполняется отдельным debug image/toolbox, production image остаётся
`scratch`.


### 10. Production runtime

Production packaging is Alpine/musl **builder only**, followed by a `FROM
scratch` image running under rootless Podman.

The final image has no shell/package manager/general userspace. Run as a
numeric non-root user with read-only rootfs, all capabilities dropped,
no-new-privileges, no published ports for long polling, and only `/data`
writable. TLS roots are compiled into the executable.

Do not add an Alpine/Debian production runtime merely for debugging
convenience; use a separate debug image/toolbox.

### 10. Privacy and safety

Minimize personal data, keep exact requester coordinates out of fan-out
notifications, avoid continuous tracking and keep retention explicit.

### 11. License boundary

`bike-aid` is AGPL-3.0-only; `teloxide-ui` is MIT. Preserve the boundary and
provide a visible Corresponding Source path for public modified deployments.

### 12. Quality gate

Formatting, clippy, tests, migrations, bilingual docs, icon-manifest coverage,
race tests and the no-unsafe rule all gate merge.
