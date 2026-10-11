# District boundaries / Границы районов

## RU

`spb-districts.json` — 18 районов Санкт-Петербурга, подготовлены 2026-10-10
из полных OSM relations (`admin_level=5`). IDs relations и их версии сохранены
в JSON, исходный список — в `export.py`. Кольца соединены по IDs узлов без
упрощения геометрии; внутренние кольца исключаются при поиске района.
Точка на общей границе относится к первому району в стабильном порядке IDs.
За границей города ближайший район не подбирается.

Данные: **© OpenStreetMap contributors**, отдельно от лицензии приложения,
под [ODbL 1.0](https://opendatacommons.org/licenses/odbl/1-0/).
[Источник и attribution](https://www.openstreetmap.org/copyright).
Датасет допускает ошибки и устаревание OSM; это не официальный кадастр.
Код `export.py` и локального resolver — AGPL-3.0-only.

Подготовка данных (до сборки, Python standard library):

```sh
python3 assets/areas/export.py --cache-dir work/osm-districts
```

Для обновления используйте пустой cache-dir: существующие ответы намеренно
переиспользуются для воспроизводимости. После обновления прогоните Rust-тесты
и пересоберите образ. Runtime использует встроенный JSON без сети; точки
пользователей никогда не передаются OSM или внешнему геокодеру.

Для оценки точности GPS проверяется пересечение круга неопределённости
(0–1500 м) с соседними районами, включая внутренние кольца. Локальная сферическая
проекция имеет консервативный запас 5 м; эта проверка даёт предупреждение,
не меняя район точного маркера. Точность не хранится в доменной БД.

### Локальные адреса и улицы

`spb-addresses.json` получен из публичного
[Saint Petersburg PBF](https://download.openstreetmap.fr/extracts/russia/northwestern_federal_district/saint_petersburg.osm.pbf),
snapshot `2026-10-09T00:26:05Z`. SHA-256 исходного PBF:
`944d36ac8d7846dc43f72607f721e031cae442dfe03b8dcefcd704e4e00cbb86`.
9799 адресных узлов, 111869 контуров зданий и 35459 именованных дорожных геометрий.
Данные также © OpenStreetMap contributors, ODbL-1.0; код экспортёра — AGPL-3.0-only.

Подготовка требует pyosmium; runtime читает только готовый встроенный JSON:

```sh
python3 -m venv work/osm-venv
work/osm-venv/bin/pip install -r assets/areas/requirements-addresses.txt
work/osm-venv/bin/python assets/areas/export-addresses.py --pbf work/saint_petersburg.osm.pbf
```

PBF скачивается по указанному публичному URL до подготовки; для воспроизводимости
используйте snapshot с указанным hash, поскольку URL регулярно обновляется.
Контуры зданий сохраняют внутренние дворы: положение внутри двора не считается
положением внутри здания. Дом определяется по контуру или адресному узлу до 20 м,
а не ближайшему bbox-центру; для разных адресов нужен отрыв не меньше 15 м
(или двойного GPS-радиуса). GPS-радиус больше 25 м исключает номер дома.
Именованная улица ищется по сегментам до 80 м. Подпись всегда «Рядом».
Без кандидата показывается карта без выдуманного адреса. Локальный поиск понимает
улицу, дом, корпус и дробный номер; неоднозначность требует выбора человека.
Публичная точка найденного здания лежит внутри его контура и не обещает найти вход.
Пользовательские адреса и точки в snapshot не добавляются. При обновлении данных
пересоберите binary.

## EN

`spb-districts.json` contains 18 Saint Petersburg districts, prepared on
2026-10-10 from full OSM relations (`admin_level=5`). Relation IDs and versions
are recorded in JSON; `export.py` contains the source list. Rings join by node
ID without geometry simplification; inner rings are excluded during lookup.
Shared-boundary points resolve to the first district in stable ID order.
Points outside the city are never assigned a guessed nearest district.

Data: **© OpenStreetMap contributors**, separately licensed from the application
under [ODbL 1.0](https://opendatacommons.org/licenses/odbl/1-0/).
[Source and attribution](https://www.openstreetmap.org/copyright).
OSM data can contain errors or become outdated; it is not an official cadastre.
`export.py` and the local resolver code are AGPL-3.0-only.

Prepare data before building using only Python's standard library:

```sh
python3 assets/areas/export.py --cache-dir work/osm-districts
```

To refresh, use an empty cache directory: existing responses are deliberately
reused for reproducibility. Run Rust tests and rebuild the image afterwards.
Runtime uses embedded JSON without network access; user coordinates are never
sent to OSM or a remote geocoder.

GPS quality checks whether an uncertainty circle (0–1500 m) can touch nearby
districts, including inner-ring boundaries. The local spherical projection has
a conservative 5 m margin; this advisory check never reassigns the marker’s
district. Accuracy is not persisted in the domain database.

### Local houses and streets

`spb-addresses.json` is derived from the public
[Saint Petersburg PBF](https://download.openstreetmap.fr/extracts/russia/northwestern_federal_district/saint_petersburg.osm.pbf),
snapshot `2026-10-09T00:26:05Z`. Source PBF SHA-256:
`944d36ac8d7846dc43f72607f721e031cae442dfe03b8dcefcd704e4e00cbb86`.
It contains 9799 address nodes, 111869 building outlines and 35459 named road geometries.
Data is © OpenStreetMap contributors under ODbL-1.0; exporter code is AGPL-3.0-only.

Preparation uses pyosmium; runtime only reads the prebuilt embedded JSON:

```sh
python3 -m venv work/osm-venv
work/osm-venv/bin/pip install -r assets/areas/requirements-addresses.txt
work/osm-venv/bin/python assets/areas/export-addresses.py --pbf work/saint_petersburg.osm.pbf
```

Download the public PBF before preparation; reproducibility requires the snapshot
with the recorded hash because the URL is regularly updated. Outlines retain
courtyard holes: courtyard points are not classified as inside a building.
House lookup uses outlines/address nodes within 20 m, not nearest bbox centers.
Different addresses require at least 15 m separation or twice the GPS radius;
a GPS radius over 25 m suppresses the number. Named street segments are bounded
to 80 m. Labels always say “Рядом”. Without a candidate, show only the map.
Local search understands street, house, corpus and slash numbers; ambiguity needs
a human choice. The public building point lies within its outline and does not
claim to locate the entrance. User addresses and points never enter the snapshot.
Rebuild the binary after refreshing data.
