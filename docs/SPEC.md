# Product specification / Продуктовая спецификация v0.2

## RU

### 1. Цель

Telegram-сервис добровольной взаимопомощи:
`helper setup → request → matching → notification → atomic accept → match → close`.

Проект обсуждается с Павлом Хусу, директором АНО «Петербургское
велообщество». Юридический статус production-инстанса и оператор ПДн
фиксируются отдельно до запуска.

### 2. Роли не храним

Нет enum `Helper | Requester | Both`.

У пользователя независимо существуют:
- optional `HelperProfile`;
- 0..N `HelpRequest`.

### 3. Frontend/backend

Frontend = `telegram/ + ui/ + teloxide-ui`.
Backend = `domain/ + application/ + storage/`.

Frontend не решает бизнес-состояние. Backend не знает о Telegram rendering.

### 4. Home

```text
[ Нужна помощь ]
[ Я могу помочь / Настроить помощь ]
[ Активная заявка ]   // conditional
[ Мои данные ]
[ Настройки ]
[ О проекте ]
[ Исходный код ]
```

Все устойчивые действия получают project-owned custom icon согласно
`docs/DESIGN_SYSTEM.md`.

### 5. Helper setup

- multi-select территорий;
- multi-select типов помощи;
- enabled toggle;
- optional availability schedule;
- preview + save.

Территории — data-driven `AreaId`, без домашнего адреса.

### 6. Request wizard

```text
kind
→ problem tags
→ description
→ optional bike description
→ exact explicit location
→ preview
→ publish
```

До match exact location не рассылается helpers.

### 7. Matching

Candidate:
- enabled;
- не requester;
- area intersection;
- capability intersection;
- availability matches.

Нет ranking “лучшего” helper в MVP.

### 8. Atomic accept

Первый atomic conditional write `Open -> Matched` выигрывает.
Остальные получают `AlreadyTaken`; их dispatches → `Superseded`.

UI store не участвует в выборе winner.

### 9. TTL

Default: 60 min, configurable.
`Open -> Expired` инвалидирует outstanding dispatches.

### 10. Accident flow

Peer assistance + статическая safety-информация. Бот не экстренная,
медицинская или юридическая служба и не должен задерживать обращение в
экстренные службы.

### 11. Storage

MVP: SQLite/WAL local filesystem. См. ADR-0003.

### 12. Non-goals v0.2

- continuous helper GPS;
- paid vendor marketplace;
- sponsor points engine;
- AI matching;
- AI medical/legal advice;
- multiple accepted helpers;
- multi-host DB.

## EN

### 1. Goal

Voluntary peer-assistance Telegram service:
`helper setup → request → matching → notification → atomic accept → match → close`.

The project is discussed with Pavel Khusu, director of ANO «Petersburg
Velo Society». The legal status of the production instance and the personal-data
operator MUST be fixed separately before launch.

### 2. No stored roles

There is no `Helper | Requester | Both` enum.

A user independently has:
- optional `HelperProfile`;
- 0..N `HelpRequest`.

### 3. Frontend/backend

Frontend = `telegram/ + ui/ + teloxide-ui`.
Backend = `domain/ + application/ + storage/`.

Frontend MUST NOT decide business state. Backend MUST NOT know about Telegram
rendering.

### 4. Home

```text
[ Need help ]
[ I can help / Configure help ]
[ Active request ]   // conditional
[ My data ]
[ Settings ]
[ About ]
[ Source code ]
```

Every stable action gets a project-owned custom icon per
`docs/DESIGN_SYSTEM.md`.

### 5. Helper setup

- multi-select of territories;
- multi-select of help types;
- enabled toggle;
- optional availability schedule;
- preview + save.

Territories are data-driven `AreaId`, without a home address.

### 6. Request wizard

```text
kind
→ problem tags
→ description
→ optional bike description
→ exact explicit location
→ preview
→ publish
```

Before match the exact location MUST NOT be fanned out to helpers.

### 7. Matching

Candidate:
- enabled;
- not the requester;
- area intersection;
- capability intersection;
- availability matches.

There is no “best helper” ranking in the MVP.

### 8. Atomic accept

The first atomic conditional write `Open -> Matched` wins.
The rest get `AlreadyTaken`; their dispatches go to `Superseded`.

The UI store does not participate in winner selection.

### 9. TTL

Default: 60 min, configurable.
`Open -> Expired` invalidates outstanding dispatches.

### 10. Accident flow

Peer assistance + static safety information. The bot is not an emergency,
medical, or legal service and MUST NOT delay contacting emergency services.

### 11. Storage

MVP: SQLite/WAL on a local filesystem. See ADR-0003.

### 12. Non-goals v0.2

- continuous helper GPS;
- paid vendor marketplace;
- sponsor points engine;
- AI matching;
- AI medical/legal advice;
- multiple accepted helpers;
- multi-host DB.
