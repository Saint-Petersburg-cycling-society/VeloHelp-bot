# ADR-0003: SQLite for the single-host MVP / SQLite для single-host MVP

Status: accepted / принято — 2026-10-03

## RU

### Решение

Использовать SQLite через SQLx:
- WAL;
- synchronous FULL;
- foreign keys ON;
- busy timeout 5 s;
- один pooled connection в MVP;
- local filesystem only.

### Почему не PostgreSQL сейчас

Нагрузка бота состоит из коротких небольших транзакций. Основная конкурентная
операция (`Accept`) — один conditional `UPDATE` плюс обновление dispatch rows.
SQLite сериализует writers, что здесь является корректной моделью, а не
ограничением продукта.

На single-host ARM deployment PostgreSQL добавил бы daemon, конфигурацию,
обновления, backup/restore и RAM overhead без нужной сейчас возможности.

### Когда пересмотреть

Пересмотр обязателен при:
- >1 application host/replica;
- удалённой DB;
- необходимости нескольких параллельных write-heavy workers;
- измеренных lock waits/latency, которые мешают SLO;
- требованиях к HA/replication, которые проще получить client/server DB.

## EN

Use SQLx + SQLite in WAL mode, FULL synchronous mode, foreign keys and a
5-second busy timeout, initially with one pooled connection on a local
filesystem.

The workload consists of short transactions and the critical accept race is an
atomic conditional update. SQLite's serialized writer model is correct for this
single-host service and avoids running an unnecessary database daemon.

Revisit the decision for multiple application hosts, remote storage,
write-heavy concurrency, measured lock latency, or HA/replication requirements.
