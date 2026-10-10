# Design system / Дизайн-система

## RU

### Принцип

UI не собирается из случайных emoji. Для стабильных сущностей используется
единый набор собственных иконок, затем загружаемый/связываемый с Telegram
custom emoji там, где API это позволяет.

Нельзя использовать `🚲`, `🚴` и варианты как конечную иконографику.

### Обязательные semantic icon IDs

```text
nav.home
nav.back
nav.next
nav.settings
action.accept
action.decline
action.cancel
action.resolve
action.edit
action.source
request.help
helper.profile
location.point
location.area
problem.puncture
problem.drivetrain
problem.brakes
problem.wheel
problem.tools
problem.transport
problem.accident
problem.other
status.open
status.matched
status.resolved
status.expired
privacy.data
privacy.delete
```

### Pipeline

```text
assets/icons/src/*.svg
    ↓ deterministic export
assets/icons/generated/*.png
    ↓ Telegram custom emoji upload/manual registration
assets/icons/manifest.toml
    ↓ application mapping
ButtonLabel / Rich UI
```

Исходник является source of truth. Generated assets воспроизводимы.

Иконка не должна быть единственным носителем смысла: у action есть текстовая
подпись/accessible fallback.

### Reuse

`IconId` и rendering component — generic. Значение `problem.puncture` —
application semantics. Не создавать `BikeIconButton` в generic layer.

### Проверенное поведение пилота (2026-10-10)

Набор v2 готов: 46 SVG + PNG/WEBP 100×100. Стиль референса bikes_spb: круг,
чёрный контур, белый диск и синий #137BB2. `assets/icons/export.py --check`
проверяет уникальность ID, соответствие исходникам, размеры, прозрачность и лимит файла.
Декоративный circle остаётся белым для читаемости в тёмной теме.
Telegram registration завершена; все 46 custom IDs заполнены. Text fallback отображает
понятное действие, не `[semantic.id]`. Back/Next/toggle не идемпотентны и отклоняют
старую UI revision. Helper withdrawal имеет отдельное действие и не вызывает cancel.

Короткий flow: одна проблема за нажатие с автоматическим переходом.
`SelectProblem` использует соответствующие `problem.*`, редактирование —
`action.edit`, «Ещё» и настройки помощника — `nav.settings`, районы — `location.area`. Все эти 46 иконок
уже зарегистрированы; новые custom-emoji ID не требуются.

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

### Иконки новых действий

Запрос места использует `location.point`. Варианты времени и приём используют
`action.accept`, точные минуты — `nav.settings`, возврат — `nav.back`, отказ —
`action.decline`. Все IDs уже есть в manifest. При отказе Telegram на custom emoji
кнопка места и остальные подписи сохраняются обычным текстом.

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

Перед приёмом подпись «Примерное место · зона около 1 × 1 км» поясняет смысл
маркера. Карта использует zoom 13, размер 512 × 256, постоянный радиус 800 м.
Matched-карта использует zoom 16 и точную точку без искусственного радиуса.
В выборе времени примерная карта остаётся, новые сообщения/кнопки не создаются.
Text fallback custom emoji сохраняет карту. Новые semantic icons не требуются.

## EN

### Principle

UI is not assembled from random emoji. Stable entities use a single set of
project-owned icons, then loaded/linked to Telegram custom emoji where the API
allows it.

MUST NOT use `🚲`, `🚴` and variants as final iconography.

### Required semantic icon IDs

```text
nav.home
nav.back
nav.next
nav.settings
action.accept
action.decline
action.cancel
action.resolve
action.edit
action.source
request.help
helper.profile
location.point
location.area
problem.puncture
problem.drivetrain
problem.brakes
problem.wheel
problem.tools
problem.transport
problem.accident
problem.other
status.open
status.matched
status.resolved
status.expired
privacy.data
privacy.delete
```

### Pipeline

```text
assets/icons/src/*.svg
    ↓ deterministic export
assets/icons/generated/*.png
    ↓ Telegram custom emoji upload/manual registration
assets/icons/manifest.toml
    ↓ application mapping
ButtonLabel / Rich UI
```

Sources are the source of truth. Generated assets are reproducible.

An icon MUST NOT be the sole carrier of meaning: an action has a text
label/accessible fallback.

### Reuse

`IconId` and the rendering component are generic. A value such as
`problem.puncture` is application semantics. Do not create `BikeIconButton`
in the generic layer.

### Verified pilot behavior (2026-10-10)

Pack v2 is prepared: 46 SVG + PNG/WEBP 100×100. The bikes_spb reference informs
circular badges, black contours, white discs and blue #137BB2.
`assets/icons/export.py --check` validates unique IDs, source/export parity, dimensions,
transparency and file limits. Discs remain white for readability in dark themes.
Telegram registration is complete; all 46 custom IDs are populated. Text fallback
shows understandable actions, not `[semantic.id]`. Back/Next/toggles are not idempotent
and reject stale UI revisions. Helper withdrawal is a separate action from cancel.

## Регистрация 2026-10-10 / Registration 2026-10-10

RU: Все 46 custom emoji ID получены от Telegram и связаны с manifest/mapping.
Набор: https://t.me/addemoji/VeloHelp_bikes_spb_by_bike_help_bot. Отказ Telegram
в отображении эмодзи переключает runtime на текстовые подписи; смысл кнопок сохранён.

EN: All 46 Telegram-issued IDs are bound in the manifest/mapping.
Pack: https://t.me/addemoji/VeloHelp_bikes_spb_by_bike_help_bot. A Telegram emoji
denial switches the runtime to text labels, preserving every action’s meaning.

The short flow selects one problem per click and advances automatically.
`SelectProblem` uses the matching `problem.*` icons; editing uses `action.edit`,
and More/helper settings use `nav.settings`; districts use `location.area`. These icons belong to the already
registered pack of 46; no new custom-emoji IDs are needed.

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

### New action icons

Location sharing uses `location.point`. Arrival choices and acceptance use
`action.accept`, exact-minute input uses `nav.settings`, back uses `nav.back`
and decline uses `action.decline`. All IDs already exist in the manifest.
If Telegram denies custom emoji, the location button and other labels use plain text.

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

Before acceptance, “Примерное место · зона около 1 × 1 км” explains the marker.
The map uses zoom 13, 512 × 256 size and fixed 800 m uncertainty. Matched maps
use zoom 16 and the exact point without an artificial radius. The coarse map stays
visible in the arrival picker; no new messages/buttons are created. Custom-emoji
text fallback preserves the map. No new semantic icons are needed.
