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

Threats include exact-location harvesting, malicious accepts, callback replay,
spam, log/backup leakage and accidental movement-history retention.

Mitigations include coarse pre-match location, actor-bound short-lived
capabilities, stale rejection, bounded retention, no continuous tracking,
minimal logging and SQLite-aware backups.

Every new persistent personal-data field requires an explicit purpose,
legal-basis review, retention period and deletion path.
