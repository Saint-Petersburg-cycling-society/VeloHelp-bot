# Icons / Иконки

## RU

46 оригинальных иконок по референсу bikes_spb: чёрный контур #111111,
синий #137BB2, белый диск, прозрачный фон вне круга. Без надписей внутри эмодзи,
градиентов и сторонних icon packs. Это отдельные знаки интерфейса, а не копия логотипа.

- `src/` — редактируемые SVG, сетка 100×100;
- `generated/` — PNG и lossless WEBP 100×100, превью;
- `manifest.toml` — semantic ID → файл, подпись RU/EN, emoji association и будущий custom ID;
- `export.py` — воспроизводимый экспорт и проверка файлов.

```sh
python -m venv target/icon-tools
target/icon-tools/bin/pip install -r assets/icons/requirements.txt
target/icon-tools/bin/python assets/icons/export.py
target/icon-tools/bin/python assets/icons/export.py --check
```

Экспорт требует Cairo и DejaVu Sans на машине разработчика, не в scratch runtime.
Ассоциации Unicode в manifest — только метаданные Telegram, не конечные UI-иконки.
Все actions сохраняют текстовую подпись. Файлы меньше 128 KiB.
Требования формата: [Telegram static emoji](https://core.telegram.org/stickers#static-stickers-and-emoji).

Пак зарегистрирован 2026-10-10: [VeloHelp · Петербургское велообщество](https://t.me/addemoji/VeloHelp_bikes_spb_by_bike_help_bot).
46 настоящих custom IDs сохранены в manifest/mapping и `generated/telegram-pack.json`.
Если Telegram отклоняет custom emoji, runtime автоматически оставляет читаемые текстовые подписи.
Для нового набора через @Stickers выберите новый
emoji pack, отправляйте PNG/WEBP как **файлы**, указывайте association из manifest.
После регистрации получите реальные `custom_emoji_id`, заполните manifest и
перегенерируйте `generated/mapping.json` через export.py, затем пересоберите контейнер.
ID не выдумывать. Проверьте права бота на отображение
custom emoji по [Bot API](https://core.telegram.org/bots/api#formatting-options).

18 районных знаков используют местные ориентиры и сохраняют полные подписи районов
в кнопках. Отдельное превью: `generated/district-preview.png`.

## EN

46 original icons following the bikes_spb reference: black #111111 contours,
blue #137BB2, white discs and transparent exteriors. No lettering inside emoji,
gradients or third-party icon packs. These are interface marks, not a logo copy.

- `src/`: editable SVG on a 100×100 grid;
- `generated/`: PNG and lossless WEBP 100×100, preview;
- `manifest.toml`: semantic ID → asset, RU/EN label, emoji association and future custom ID;
- `export.py`: reproducible export and asset validation.

Use the virtual environment and commands shown above. Export requires Cairo and
DejaVu Sans on the developer machine, not in the scratch runtime.
Unicode associations are Telegram metadata, not final UI icons. Actions retain text
labels. Files are below 128 KiB. See [Telegram static emoji requirements](https://core.telegram.org/stickers#static-stickers-and-emoji).

Registered on 2026-10-10: [VeloHelp · Petersburg Velo Society](https://t.me/addemoji/VeloHelp_bikes_spb_by_bike_help_bot).
All 46 real IDs are in the manifest/mapping and `generated/telegram-pack.json`.
The runtime automatically keeps readable text labels if Telegram denies custom emoji.
For a new pack, in @Stickers create an emoji pack, send PNG/WEBP
as **files**, and use the manifest associations. After registration retrieve real
`custom_emoji_id` values, update the manifest, regenerate `generated/mapping.json` with export.py, and rebuild
the container. Never invent
IDs. Verify bot eligibility using [Bot API](https://core.telegram.org/bots/api#formatting-options).

The 18 district icons use local landmarks; buttons retain full district names.
A separate preview is in `generated/district-preview.png`.

| Район / District | Символ / Symbol |
| --- | --- |
| Адмиралтейский / Admiralteysky | Адмиралтейство / Admiralty |
| Василеостровский / Vasileostrovsky | Ростральные колонны / Rostral columns |
| Выборгский / Vyborgsky | Готические башни Шуваловского парка / Shuvalov Park gothic towers |
| Калининский / Kalininsky | Финляндский вокзал, паровоз / Finland Station locomotive |
| Кировский / Kirovsky | Нарвские ворота / Narva Gate |
| Колпинский / Kolpinsky | Ижорские заводы / Izhora factories |
| Красногвардейский / Krasnogvardeysky | Большеохтинский мост / Bolsheokhtinsky Bridge |
| Красносельский / Krasnoselsky | Дудергофские высоты / Duderhof Heights |
| Кронштадтский / Kronshtadtsky | Морской собор и якорь / Naval Cathedral and anchor |
| Курортный / Kurortny | Сосны и берег залива / Pines and Gulf shore |
| Московский / Moskovsky | Чесменская церковь / Chesme Church |
| Невский / Nevsky | Вантовый мост / Cable-stayed bridge |
| Петроградский / Petrogradsky | Петропавловская крепость / Peter and Paul Fortress |
| Петродворцовый / Petrodvortsovy | Фонтаны Петергофа / Peterhof fountains |
| Приморский / Primorsky | Лахта Центр / Lakhta Center |
| Пушкинский / Pushkinsky | Екатерининский дворец / Catherine Palace |
| Фрунзенский / Frunzensky | Купчино, метро / Kupchino metro |
| Центральный / Tsentralny | Спас на Крови / Church of the Savior on Spilled Blood |
