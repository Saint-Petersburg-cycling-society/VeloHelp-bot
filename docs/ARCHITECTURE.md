# Architecture / Архитектура

## RU

### Граница frontend/backend

```text
Telegram
   ↓
telegram/ + ui/                 FRONTEND
   ↓ typed command/event
application/ + domain/          BACKEND
   ↓
storage/ SQLite
```

Frontend отвечает за ввод, отображение, `Surface`, `ViewId`, `Revision`,
capabilities и projection. Backend отвечает за доменные инварианты, matching и
авторитетные транзакции.

`ui/` не импортирует SQLx. `domain/` не импортирует teloxide.

### Accept

```text
callback
→ resolve actor/stale capability
→ application/repository::accept(request_id, helper_id, request_version)
→ atomic conditional UPDATE
→ commit
→ domain outcome
→ reproject requester + N dispatch views
```

Никакого Telegram I/O внутри DB transaction.

### Один доменный объект — много views

```text
HelpRequest #42
├─ requester view
├─ helper A dispatch view
├─ helper B dispatch view
└─ helper C dispatch view
```

Views имеют независимые revisions и surfaces.

### SQLite

Single-host MVP использует SQLite/WAL на локальной FS. Начальный pool — один
connection. Никаких сетевых filesystem для live DB.

PostgreSQL становится оправданным при нескольких application hosts, отдельном
сетевом DB service или измеренной write-конкуренции, которую single-writer
SQLite уже не обслуживает.

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
возврата или отмены оно и сообщение снятия клавиатуры удаляются. При ошибке удаления
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

Политика округления находится в domain::ApproximateLocation и не зависит от
Telegram. Адаптер выводит производный центр через тот же native map primitive,
что и matched-карта; доменная точка не подменяется округлённой. Pure view остаётся
без SQL/API. До Matched карты используют только центр/постоянный радиус; после
авторизации текущей пары — исходную точку. Состояние приёма определяется SQLite.

## EN

### Frontend/backend boundary

```text
Telegram
   ↓
telegram/ + ui/                 FRONTEND
   ↓ typed command/event
application/ + domain/          BACKEND
   ↓
storage/ SQLite
```

Frontend owns input, rendering, `Surface`, `ViewId`, `Revision`,
capabilities and projection. Backend owns domain invariants, matching and
authoritative transactions.

`ui/` MUST NOT import SQLx. `domain/` MUST NOT import teloxide.

### Accept

```text
callback
→ resolve actor/stale capability
→ application/repository::accept(request_id, helper_id, request_version)
→ atomic conditional UPDATE
→ commit
→ domain outcome
→ reproject requester + N dispatch views
```

No Telegram I/O inside the DB transaction.

### Multi-view model

```text
HelpRequest #42
├─ requester view
├─ helper A dispatch view
├─ helper B dispatch view
└─ helper C dispatch view
```

Views have independent revisions and surfaces. They share domain
truth but not UI revision identity.

### SQLite

Single-host MVP uses SQLite/WAL on a local filesystem. The initial pool is one
connection. No network filesystem for the live DB.

PostgreSQL becomes justified with multiple application hosts, a separate
network DB service, or measured write concurrency that single-writer
SQLite no longer serves.


## Runtime isolation / Изоляция runtime

### RU

Production boundary: rootless Podman + scratch image. Контейнер содержит только
статически собранный application executable; SQLite state приходит отдельным
writable mount. У приложения нет причины иметь shell или general-purpose
userspace.

### EN

Production uses a rootless Podman scratch image containing only the compiled
application executable. SQLite state is a separate writable mount; the
application has no runtime dependency on a shell or general-purpose userspace.

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
message; failed cleanup is retried on polling passes. The primary card always
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

Quantization policy lives in domain::ApproximateLocation without Telegram types.
The adapter renders its derived center with the native map primitive also used for
matched cards; it never replaces the stored domain point. Pure views still perform
no SQL/API work. Pre-match maps use only the center/fixed radius; the current
authorized pair receives the original point. SQLite still owns acceptance.
