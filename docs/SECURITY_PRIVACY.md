# Security & privacy / Безопасность и приватность

## RU

Основные угрозы:
- сбор точных координат злоумышленником;
- accept только ради получения location;
- callback replay/stale action;
- spam;
- утечка через логи/backup;
- случайное накопление истории перемещений.

Контрмеры:
- exact point не входит в fan-out;
- actor-bound short-lived capabilities;
- stale reject для Accept;
- rate limits;
- не логировать full Telegram Update;
- не логировать exact coordinates по умолчанию;
- конечный retention;
- no continuous tracking;
- backup через SQLite API, не raw-copy WAL;
- backup получает те же access/localization/retention rules.

Каждое новое persisted personal-data поле требует:
`purpose + legal basis review + retention + deletion path`.

## EN

Main threats:
- exact-coordinate harvesting by an attacker;
- accept only to obtain location;
- callback replay/stale action;
- spam;
- leakage via logs/backups;
- accidental accumulation of movement history.

Countermeasures:
- exact point is not included in fan-out;
- actor-bound short-lived capabilities;
- stale reject for Accept;
- rate limits;
- do not log the full Telegram Update;
- do not log exact coordinates by default;
- finite retention;
- no continuous tracking;
- backup via the SQLite API, not raw-copy of WAL;
- backups get the same access/localization/retention rules.

Every new persisted personal-data field requires:
`purpose + legal basis review + retention + deletion path`.
