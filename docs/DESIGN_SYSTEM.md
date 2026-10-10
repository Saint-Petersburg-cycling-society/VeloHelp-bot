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

## EN

Stable UI concepts use a project-owned icon family instead of random system
emoji. Bicycle/cyclist Unicode emoji are not final product icons.

Source SVGs are authoritative, exports are deterministic, Telegram custom-emoji
IDs are recorded in the manifest, and every action keeps an accessible text
meaning.

Icon rendering is generic; application semantic icon IDs remain application
data.
