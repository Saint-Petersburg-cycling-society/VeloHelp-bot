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

### Проверенное поведение пилота (2026-10-10)

Закрывающие транзакции немедленно удаляют exact_lat/exact_lon. Startup удаляет
остаточные точки ранее закрытых заявок. Повторное чтение matched surface проверяет
status и Telegram ID viewer. Token скрыт в `Config::Debug`; транспорт не логирует
полные Update и URL ошибок сети. Export — валидный JSON всех строк пользователя,
не содержит чужих exact points. Удаление в БД не удаляет уже доставленные сообщения
Telegram или старые backup-файлы. Backup по умолчанию хранится 14 дней; восстановление
из старого backup требует повторного применения удалений. Drafts не пишутся на диск
рабочим polling adapter и удаляются из памяти через час. Отдельный SnapshotStore
использует уникальные временные файлы, mode 0600 и fsync/rename; он пока не
подключён к рабочему polling flow.
После удаления пользователя его черновики и capabilities немедленно удаляются
из памяти до обновления сообщений других участников. Кликабельное имя использует
уже хранимый Telegram ID только для связи участников текущей пары; это описано
на экране согласия. Имя кешируется в памяти до часа, не хранится в БД и при
восстановлении запрашивается через getChat. Закрытие убирает карту, имя-ссылку
и время из обновляемой карточки, но ранее увиденные данные отозвать нельзя.
`IGNORED_TELEGRAM_USER_IDS` исключает указанные аккаунты до создания сессии,
consent screen или любых записей в БД. Числовые ID остаются только в защищённой
конфигурации для фильтрации; они не входят в экспорт исходников и публичный manifest.

Район заявки вычисляется локально по OSM-полигонам из `assets/areas/`.
Координаты не уходят во внешний геокодер. За границами Петербурга точка не
записывается в черновик. Датасет границ отдельно лицензирован ODbL-1.0.

Точность GPS используется только для временного предупреждения в preview
и не записывается в доменную БД, экспорт или уведомление помощнику.

После принятия корректной геоточки в черновик runtime пытается удалить входящее
геосообщение пользователя в личном чате. Ошибка Telegram не отменяет черновик
и не блокирует дальнейшее оформление. Это не гарантия удаления скриншотов
или ранее сохранённых копий.

### Адрес и оценка прибытия

Адрес определяется только локально по встроенным адресам и улицам OSM.
Точка не передаётся внешнему геокодеру. Предпросмотр видит только requester;
после приёма карту, адрес и имя-ссылку получают только участники пары. Домовой
номер показывается только при однозначном контуре/адресной точке до 20 м;
улица — до 80 м. GPS-радиус больше 25 м или конкурирующие дома убирают номер.
GPS quality остаётся в сессии до TTL и не сохраняется в БД.
Ручной `exact_address` нужен для координации: requester явно вводит его,
видит в preview и экспортирует со своей заявкой. Адрес не попадает в fan-out,
логи или экспорт помощника; виден только текущей паре. SQL trigger очищает его
при закрытии или удалении точки. Фильтры `helper_problem_tags` нужны для выбора
подходящих уведомлений; хранятся с профилем до изменения/удаления аккаунта.
Диагностика пишет только фиксированное имя операции и категорию ошибки;
URL, сырой ответ Telegram, описание, адрес, ID и координаты не логируются.

Новое сохраняемое поле — границы заявленного времени прибытия. Purpose:
координация помощи; пользователь явно выбирает время при приёме, а экран
согласия описывает его хранение. Поле не используется для слежения и удаляется
при закрытии/отказе/удалении пары. JSON export содержит только свои оценки.
Политика backup 14 дней и ограничения старых backup остаются применимыми;
перед публичным запуском оператор должен завершить review правового основания.

### Примерное место в приглашении

До match помощник видит район и центр ячейки около 1 × 1 км вместо точного места.
Публичная проекция не включает ближайший адрес, профиль requester, точный остаток
округления, направление, GPS accuracy или live location. horizontal_accuracy=800 м —
постоянная оценка неопределённости ячейки, не точность устройства. Случайный jitter
не используется: несколько получателей/отправок/рестартов не дают набор точек для
усреднения. Пояснение входит в consent и preview перед явной публикацией. Это
снижение точности, не гарантия анонимности: район, окружение и описание могут помочь
сопоставить человека. Название дома от геокодера не выводится до match. Ранее
доставленные сообщения/скриншоты отозвать невозможно; закрытие очищает БД и текущие
карточки по прежней retention policy.

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

### Verified pilot behavior (2026-10-10)

Closing transactions clear exact_lat/exact_lon immediately. Startup clears residual
points on previously closed requests. Matched-surface disclosure checks status and
viewer Telegram ID. `Config::Debug` redacts the token; transport logs no full Updates
or network-error URLs. Export is valid JSON of the user's rows, excluding other users'
exact points. Database deletion does not remove already delivered Telegram messages
or historical backups. Default backup retention is 14 days; restoring an old backup
requires reapplying deletions. The polling adapter keeps drafts in memory only and
expires them after one hour. The separate SnapshotStore uses unique temporary files,
mode 0600 and fsync/rename, but is not connected to the live polling flow.
User deletion immediately clears their drafts and capabilities from memory before
refreshing other participants' messages. The clickable name uses the existing
Telegram ID solely for communication within the current pair, as described by
the consent screen. Names are cached in memory for up to one hour, never stored
in the DB, and recovered through getChat. Closure removes the map, profile link
and arrival time from refreshed cards; previously viewed data cannot be revoked.
`IGNORED_TELEGRAM_USER_IDS` excludes configured accounts before session creation,
consent rendering or database writes. Numeric IDs remain only in protected runtime
configuration for filtering; source archives and public manifests exclude them.

Request districts are computed locally using OSM polygons in `assets/areas/`.
Coordinates never go to a remote geocoder. Points outside Saint Petersburg are
not recorded in the draft. Boundary data is separately licensed ODbL-1.0.

GPS accuracy is used only for a temporary preview warning; it is not written
to the domain database, export or helper notification.

After accepting a valid location into the draft, the runtime attempts to delete
the incoming user location message in the private chat. A Telegram deletion error
does not discard the draft or block the flow. This does not remove screenshots
or copies already saved elsewhere.

### Address and arrival estimate

Address lookup runs entirely locally against embedded OSM houses and streets.
No point goes to a remote geocoder. Only the requester sees preview; after matching,
only pair members receive the map, address and profile link. House centers are
approximate: house numbers require an unambiguous outline/address node within
20 m; streets are bounded to 80 m. GPS radii over 25 m or competing buildings
suppress house numbers. GPS quality stays in the session until TTL and never in DB.
Manual `exact_address` coordinates assistance: the requester explicitly enters it,
reviews it and exports it with their own requests. It never enters fan-out, logs
or helper exports; only current pair members see it. A SQL trigger clears it
on closure or point removal. `helper_problem_tags` filter relevant notifications
and remain with the profile until changed or the account is deleted.
Diagnostics log only fixed operation/error categories, never URLs, raw Telegram
responses, descriptions, addresses, IDs or coordinates.

The new persisted personal fields are declared arrival-time bounds. Purpose:
coordinate assistance; users explicitly select an estimate on acceptance and the
consent screen describes retention. The fields are not used for tracking and are
cleared on closure/withdrawal/deletion. JSON export contains only one’s own estimates.
The 14-day backup policy and historical-backup limitations still apply; before public
launch the operator must complete review of the legal basis.

### Approximate invitation location

Before matching, helpers see the district and an approximately 1 × 1 km cell center
instead of the exact point. Public projection contains no nearby address, requester
profile, exact rounding residual, heading, GPS accuracy or live location. The fixed
800 m horizontal_accuracy expresses cell uncertainty, not device precision. There is
no random jitter: recipients, repeat sends and restarts do not produce samples to
average. Consent and preview explain disclosure before explicit publication. This
reduces precision rather than guaranteeing anonymity: district/context/descriptions
may allow identification. No geocoded house label appears before matching. Previously
delivered messages/screenshots cannot be revoked; closure clears the DB and current
cards under the existing retention policy.
