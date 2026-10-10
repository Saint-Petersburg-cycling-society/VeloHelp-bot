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
[ Активные заявки ]   // только при наличии
[ Ещё ]              // данные, исходники, информация
```

Все устойчивые действия получают project-owned custom icon согласно
`docs/DESIGN_SYSTEM.md`.

### 5. Helper setup

- multi-select районов: все 18 на одном экране, две колонки;
- multi-select типов помощи, поломки включены по умолчанию;
- enabled toggle;
- save; расписания пока не подключены.

Территории — data-driven `AreaId`, без домашнего адреса.

### 6. Request wizard

```text
problem button OR free-text description
→ explicit location (automatic local district lookup)
→ preview
→ publish
```

Одна кнопка проблемы задаёт единственный признак, тип и краткое описание,
сразу открывая геопозицию без «Далее». Свой текст также сразу переводит к месту.
Отправленная геопозиция автоматически открывает предпросмотр; за пределами
Петербурга остаётся текущий шаг с пояснением. Район вручную не выбирается.
Редактирование описания сохраняет точку и сразу возвращает к предпросмотру;
место можно изменить отдельно. Публикация требует явного нажатия.
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

### Проверенное поведение пилота (2026-10-10)

Текущий transport — long polling в личных чатах. Неподтверждённые пользователи
получают consent screen; запись identity создаётся только явным подтверждением.
Район вычисляется по вложенной геопозиции локально из ODbL-границ OSM, без
передачи точки геокодеру. Доступность по расписанию пока не подключена.
Matching повторяется на polling pass для открытых заявок, учитывая новые профили;
создание dispatches идемпотентно в рамках request version. Доставка — at least once:
рестарт или ошибка записи sent_at могут дать повторное сообщение. После рестарта pending notifications
рассылаются заново, старые кнопки отклоняются; активные пары и pending invitations доступны через `/active`.
Метаданные views и черновики — in-memory; черновики и меню имеют TTL 1 час,
capabilities Open-заявок — оставшийся TTL заявки; permanent draft recovery
и гарантированная очередь projection retries пока не реализованы.
В карточке принятой пары имя другого участника становится ссылкой на Telegram-профиль.
Перед раскрытием UI проверяет актуальный статус и Telegram ID участника по доменной БД.
Возможность написать зависит от настроек Telegram. После закрытия имя-ссылка удаляется
при обновлении карточки; ранее увиденные данные Telegram отозвать нельзя.

При геопозиции рядом с границей районов или точности GPS хуже 100 м
предпросмотр просит проверить точку. Дополнительного шага выбора района нет;
маркер можно исправить через «Изменить место».

Выбор районов помощника сохраняется при переходе в настройки и при ошибке типа помощи.
Проверки отсутствующего района и типа дают разные сообщения; техническая помощь
выбрана по умолчанию. Каждый район использует свою `area.*` custom emoji.

Основной экран помощника: одна строка счётчика, все 18 кнопок районов в две
колонки на одном экране, затем «Сохранить / Главная / Настройки». Пагинации нет.
Тип помощи и переключатель уведомлений находятся в отдельном экране
«Настройки»; возврат к районам сохраняет выбор.
«Сохранить» отключено, пока включённому помощнику не выбран район или тип.
Возврат на главную и `/start` сохраняют временный черновик помощника до TTL.
`/start` обновляет текущее главное сообщение; кнопки прежней revision
инвалидируются. Новый экран создаётся только при отсутствии/недоступности старого
сообщения, а прежний по возможности удаляется.
Заголовки экранов выводятся обычным текстом без крупного Rich Heading.

### Карта и время прибытия

На шаге места есть стандартная reply-кнопка «Отправить местоположение» с
`request_location=true` ([Bot API](https://core.telegram.org/bots/api#keyboardbutton)).
Также можно отправить выбранную точку вложением Telegram. После получения точки
клавиатура убирается; район и ближайший адрес определяются локально.

«Помогу через…» в исходном приглашении открывает интервалы 5–15, 15–30,
30–60, 60–120 минут или ввод точного числа 1–240. Открытие выбора не принимает
заявку. Выбор интервала или валидное число выполняют атомарный приём вместе
с оценкой времени; проигравший помощник не перезаписывает оценку победителя.

Исходная карточка обновляется на месте: имя другого участника — кликабельная
ссылка, ниже время прибытия по Москве, ближайший адрес и нативная карта
`InputRichBlockMap`. Отдельных кнопок контакта/карты и отдельного сообщения
с геоточкой нет. До приёма рассылка содержит район и приблизительную зону; точная точка, адрес и
имя-ссылка доступны только участникам текущей пары.

### Одно основное сообщение

Предпросмотр → публикация → приём/закрытие используют одно основное Rich-сообщение
с тем же message ID. Отдельное «Заявка опубликована» не отправляется. Кнопки
главной, настроек и списка заявок редактируют сообщение, в котором нажаты.
Команды `/start`, `/help`, `/active`, `/cancel` создают новый основной экран внизу
чата; после успешной отправки старый экран удаляется, его capabilities отзываются.
Ошибка удаления повторяется в polling pass. При одной
активной заявке сразу открывается карточка; при нескольких — список с кнопками
`OpenRequest`, и выбранная карточка заменяет список. `OpenRequest` использует
существующую иконку `status.open` и проверяет доступность через backend.

Переход в меню снимает связь surface с заявкой: её background refresh не может
заменить открытые настройки/главную. Приглашения помощникам — отдельные уведомления;
смена их версии редактирует имеющуюся карточку в текущей сессии. `/active` убирает
дубли выбранной карточки после успешного отображения в основном сообщении.
Метаданные surface пока живут только в памяти: после restart первое открытие
создаёт новый основной экран, старые capabilities не работают.

[Telegram ограничивает редактирование сообщений с reply-клавиатурами](https://core.telegram.org/bots/api#updating-messages).
Поэтому request_location вынесен во временное тихое сообщение; после геоточки,
возврата или отмены оно и сообщение снятия клавиатуры удаляются. При первом
`/start` после запуска неизвестную клавиатуру не пытаются снимать видимым
служебным сообщением. При ошибке удаления
runtime повторяет cleanup в polling pass. Основная карточка всегда без reply_markup.
Новый основной экран при ошибке редактирования допустим только для
`MessageToEditNotFound`/`MessageCantBeEdited`; сетевые и прочие ошибки не создают дубль.

Регрессия проверяет не только успешный ответ mock: stub запрещает редактирование
reply-keyboard сообщений. Smoke считает успешные Send/Edit/Delete, проверяет один
оставшийся экран: кнопки сохраняют message ID через публикацию, match и resolve,
а команды создают новый экран и удаляют прежний. Проверены несколько заявок,
кнопка в отдельном приглашении и повторная попытка после временной ошибки карты.
Причина дублей локализована в адаптере VeloHelp; путь `SurfaceWorker::project`
в закреплённой версии teloxide-ui использует редактирование существующей surface.
Неподтверждённые изменения библиотеки в этот fix не включены.

### Примерное место в приглашении

Open-приглашение и экран выбора времени получают примерную карту, чтобы помощник
оценил дорогу до приёма. Маркер — центр фиксированной ячейки около 1 × 1 км;
подпись явно сообщает, что точное место и адрес будут после приёма. При Matched
у победителя карта заменяется точной в том же сообщении; у остальных помощников
карта убирается. Закрытие/TTL убирают карту из обновляемых карточек. Чужой actor
не может открыть приглашение через callback или OpenRequest. Requester получает
пояснение на consent и в preview; дополнительных шагов и кнопок не добавляется.

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
[ Active requests ]   // only when present
[ More ]              // data, source, information
```

Every stable action gets a project-owned custom icon per
`docs/DESIGN_SYSTEM.md`.

### 5. Helper setup

- multi-select districts: all 18 on one screen, in two columns;
- multi-select help types, technical assistance selected by default;
- enabled toggle;
- save; availability schedules are not integrated yet.

Territories are data-driven `AreaId`, without a home address.

### 6. Request wizard

```text
problem button OR free-text description
→ explicit location (automatic local district lookup)
→ preview
→ publish
```

A problem button sets a single tag, type and short description, immediately
opening location without Next. Free text also advances immediately. A submitted
location automatically opens preview; points outside Saint Petersburg keep the
current step with an explanation. Requesters never select a district manually.
Editing the description retains the point and returns directly to preview;
location can be edited separately. Publishing requires an explicit click.
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

### Verified pilot behavior (2026-10-10)

The current transport polls private chats. Users without current consent receive a
consent screen; identity is persisted only after explicit confirmation. Districts
are resolved locally from the submitted point using ODbL OSM boundaries; no point
is sent to a geocoder. Availability schedules are not integrated yet.
Matching runs for open requests on polling passes, including new helper profiles;
dispatch creation is idempotent per request version. Delivery is at least once:
restart or a failed sent_at write can cause duplicate messages. Pending notifications are replayed
on restart, old buttons are rejected, and matches/pending invitations recover via `/active`.
Views and drafts are in memory; drafts/menus have a one-hour TTL, while
open-request capabilities last for the remaining request TTL; durable draft recovery
and a guaranteed projection retry queue are not implemented yet.
The matched card links the other participant’s name to their Telegram profile.
Before disclosure, the UI checks current status and participant Telegram ID against
the domain database. Messaging depends on Telegram privacy settings. Closure removes
the link when refreshing the card; previously viewed Telegram data cannot be revoked.

Near district borders or when GPS accuracy is 100 m or worse, preview asks the
user to check the point. There is no extra district-selection step; the marker
can be corrected through Edit location.

Helper district selections survive visits to settings and help-type validation errors.
Missing areas and missing help types have distinct messages; technical help is
selected by default. Each district uses its own `area.*` custom emoji.

The helper's main screen has one counter line, all 18 district buttons in two
columns on one screen, then Save / Home / Settings. There is no pagination.
Help types and the notification toggle are on a separate Settings screen;
returning to districts preserves selections.
Save is disabled while an enabled helper has no area or help type selected.
Home and `/start` preserve the temporary helper draft until its TTL expires.
`/start` updates the current primary message and invalidates the previous
revision’s capabilities. A new surface is created only when the old message is
missing or uneditable; the old message is deleted when possible.
Screen titles use ordinary text rather than oversized Rich Heading blocks.

### Map and arrival estimate

The location step uses a native reply button, “Отправить местоположение”,
with `request_location=true` ([Bot API](https://core.telegram.org/bots/api#keyboardbutton)).
A manually chosen Telegram location attachment also works. Receiving the point
removes the keyboard; district and nearby address lookup are local.

“Помогу через…” opens 5–15, 15–30, 30–60 or 60–120 minute ranges, or exact
minutes from 1 to 240. Opening the picker does not accept the request. Choosing
a range or submitting a valid number atomically accepts with the estimate;
a losing helper cannot replace the winner’s estimate.

The original card updates in place: the other participant’s name is a clickable
profile link, followed by arrival time in Moscow, a nearby address and a native
`InputRichBlockMap`. There are no separate contact/map buttons or location
messages. Pre-match fan-out contains the district and an approximate zone; the exact point, address and
profile link are available only to the current matched pair.

### One primary message

Preview → publication → acceptance/closure use one primary Rich Message with
the same message ID. No separate publication confirmation is sent. Home, settings
and request-list buttons edit the message carrying the clicked button. Commands
`/start`, `/help`, `/active`, `/cancel` create a new primary screen at the bottom
of the chat. After a successful send, the previous screen is deleted and its
capabilities revoked; failed deletion is retried on polling passes. One active request
opens directly; multiple requests show an `OpenRequest` button list, replaced
by the selected card. `OpenRequest` reuses `status.open` and checks availability
through the backend.

Menu navigation detaches the surface from its request: background refresh cannot
replace settings/home. Helper invitations remain separate notifications; new waves
edit an existing card in the current session. `/active` removes duplicates of the
selected card after successful rendering in the primary message. Surface metadata
is still in memory: the first opening after restart creates a new primary screen
and old capabilities no longer work.

[Telegram restricts editing messages with reply keyboards](https://core.telegram.org/bots/api#updating-messages).
The native request_location button therefore uses a temporary silent carrier.
Receiving a point, returning or cancelling removes it and the keyboard-removal
message; failed cleanup is retried on polling passes. At a fresh `/start`, an
unknown keyboard state does not trigger a visible cleanup message. The primary card always
has no reply_markup. A new main screen after edit failure is allowed only for
`MessageToEditNotFound`/`MessageCantBeEdited`; network/other errors never create a copy.

Regression checks go beyond a successful mock response: the stub rejects edits
of reply-keyboard messages. Smoke counts successful Send/Edit/Delete and verifies
one surviving screen: buttons preserve the message ID through publication, match
and resolution; commands create a new screen and delete its predecessor. Coverage
includes multiple requests, a button in a separate invitation and transient map retry.
Duplicate creation is in VeloHelp’s adapter; the pinned teloxide-ui
`SurfaceWorker::project` edits the existing surface. No unsubstantiated library
changes are included in this fix.

### Approximate invitation location

Open invitations and arrival pickers include an approximate map so helpers can
assess travel before acceptance. The marker is the center of a fixed approximately
1 × 1 km cell; its label says the exact point/address follow acceptance. Matching
replaces the winner’s map with the exact one in the same message and removes maps
from losing helpers’ refreshed cards. Closure/TTL remove maps. Foreign actors cannot
open invitations through callbacks or OpenRequest. Consent and requester preview
explain disclosure; no extra steps or buttons are introduced.
