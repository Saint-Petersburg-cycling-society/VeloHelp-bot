# bike-aid

Telegram-бот взаимопомощи велосипедистов и реальный dogfood-проект для
[`teloxide-ui`](https://github.com/Mar2ianen/teloxide-ui).

**Статус:** design/bootstrap. API и схема данных пока не считаются стабильными.

## Главный пользовательский цикл

```text
helper setup
    ↓
request draft → publish
    ↓
matching
    ↓
fan-out notifications
    ↓
atomic accept
    ↓
matched pair
    ↓
resolve / cancel / expire
```

Основной инвариант: **победитель заявки определяется доменной БД, а не UI runtime**.
`teloxide-ui` отвечает за capabilities, actor/stale policy и консистентную
проекцию состояния в Telegram surfaces.

## Лицензия

Проект распространяется под **GNU AGPL-3.0-only**. Полный текст — `LICENSE`.

Для публично работающей модифицированной инстанции должен оставаться заметный
способ получить Corresponding Source. В продуктовой спеке предусмотрен пункт
`Исходный код` в главном меню и `SOURCE_URL` в конфиге.

`teloxide-ui` — отдельный MIT-проект и остаётся отдельной dependency.

## Что читать

- `docs/SPEC.md` — продуктовая спека MVP.
- `docs/ARCHITECTURE.md` — границы домена, UI и Telegram.
- `docs/STATE_MACHINES.md` — state machines и гонки.
- `docs/DATA_MODEL.md` — модель данных и SQL-инварианты.
- `docs/TELOXIDE_UI_DOGFOOD.md` — что этот бот должен вытащить в библиотеку.
- `docs/SECURITY_PRIVACY.md` — threat model, минимизация данных, retention.
- `docs/legal/PDN_CHECKLIST_RU.md` — инженерный checklist по ПДн РФ.
- `docs/ROADMAP.md` — порядок реализации.

## Быстрый старт

```bash
cp .env.example .env
$EDITOR .env
cargo fmt --all
cargo test
cargo run
```

Для БД:

```bash
sqlx database create
sqlx migrate run
```

## Структура

```text
src/domain/       чистая доменная модель и инварианты
src/application/  use-cases / orchestration / транзакционные команды
src/storage/      SQLx adapters
src/telegram/     transport/update handling
src/ui/           teloxide-ui views/actions/sessions
```

Domain code не импортирует Telegram и `teloxide-ui`.
