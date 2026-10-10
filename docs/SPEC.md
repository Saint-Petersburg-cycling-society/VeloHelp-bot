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

The product coordinates voluntary peer assistance through a Telegram
frontend and an authoritative domain backend.

There is no permanent requester role: users may independently have a helper
profile and create requests.

The frontend (`telegram/ui/teloxide-ui`) owns interaction and projection.
The backend (`domain/application/storage`) owns matching, request state and
transactions.

The MVP flow is helper configuration, request wizard, coarse-area matching,
fan-out notification, atomic single-winner accept, exact-location disclosure to
the matched helper, and resolution/cancellation/expiry.

The MVP uses local SQLite/WAL. Continuous helper tracking, AI decisions,
marketplace/rewards and multi-host database deployment are deferred.
