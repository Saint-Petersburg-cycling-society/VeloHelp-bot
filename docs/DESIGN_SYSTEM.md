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
