# Icons / Иконки

## RU

`src/` — editable vector sources.
`generated/` — deterministic Telegram-ready exports.
`manifest.toml` — semantic ID → source/export/custom emoji metadata.

Не коммитить иконку без source asset.
Не использовать случайные Unicode emoji как замену отсутствующей иконке.

Статус v0: `manifest.toml` резервирует 28 ID из `docs/DESIGN_SYSTEM.md`,
`custom_emoji_id` пустые, `src/*.svg` и `generated/*.png` — TODO.
SVG-генерация вынесена за пределы этого изменения (Codex с image gen).

## EN

`src/` contains editable vector sources, `generated/` deterministic Telegram
exports, and `manifest.toml` maps semantic IDs to assets/custom emoji metadata.

Never commit a generated icon without its source and do not substitute random
Unicode emoji for missing assets.

v0 status: `manifest.toml` reserves the 28 IDs from `docs/DESIGN_SYSTEM.md`
with empty `custom_emoji_id`; `src/*.svg` and `generated/*.png` are TODO.
SVG generation is out of scope for this change (Codex with image gen).
