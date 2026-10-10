# teloxide-ui dogfood / План dogfood

## RU

Bootstrap pin:

```text
https://github.com/Mar2ianen/teloxide-ui
70da4d21c327d70e18425eb97503e6b97e94a30e
```

Бот должен вытянуть в библиотеку только общие механики:

1. callback dispatcher + automatic ACK;
2. per-action actor/stale/TTL policy;
3. external input events (location/text), не притворяющиеся callback;
4. `RenderAt` / `Send` / `Delete` / toast / alert;
5. persisted/recoverable UI runtime;
6. invalidation capabilities между несколькими `ViewId`;
7. reusable `MultiSelect`, `Wizard`, `Confirm`, `ActionBar`, `StatusBadge`;
8. стабильный icon/custom-emoji component contract.

### Правило extraction

```text
реальная боль в bike-aid
→ минимальная app implementation
→ выделение общего invariant
→ чистый MIT primitive в teloxide-ui
→ миграция bike-aid на primitive
```

Generic API не должен содержать Bike/Bicycle-specific types.

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

## EN

Bootstrap pin:

```text
https://github.com/Mar2ianen/teloxide-ui
70da4d21c327d70e18425eb97503e6b97e94a30e
```

The bot MUST pull only generic mechanics into the library:

1. callback dispatcher + automatic ACK;
2. per-action actor/stale/TTL policy;
3. external input events (location/text) that never pretend to be callbacks;
4. `RenderAt` / `Send` / `Delete` / toast / alert;
5. persisted/recoverable UI runtime;
6. invalidation capabilities across multiple `ViewId`;
7. reusable `MultiSelect`, `Wizard`, `Confirm`, `ActionBar`, `StatusBadge`;
8. stable icon/custom-emoji component contract.

### Extraction rule

```text
real pain in bike-aid
→ minimal app implementation
→ isolate the generic invariant
→ clean MIT primitive in teloxide-ui
→ migrate bike-aid to the primitive
```

Generic APIs MUST NOT contain Bike/Bicycle-specific types.

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
