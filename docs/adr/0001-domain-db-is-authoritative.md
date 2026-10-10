# ADR-0001: Domain DB is authoritative / Доменная БД авторитетна

Status: accepted / принято

## RU

SQLite хранит авторитетное состояние requests/helpers/matching.
`teloxide-ui::UiStore` никогда не решает, открыта ли заявка и кто выиграл race.

Следствия:
- domain commit остаётся истиной при падении Telegram projection;
- projection можно retry без повторения бизнес-transition;
- несколько surfaces могут независимо отставать;
- UI persistence и domain persistence развиваются отдельно.

## EN

SQLite is authoritative for request/helper/matching state.
`teloxide-ui::UiStore` never decides whether a request is open or who won the
accept race. Domain commits survive projection failures, projections can retry
without replaying transitions, and UI/domain persistence remain separate.
