# Политика обработки персональных данных — TEMPLATE

> Не публиковать как финальный документ без заполнения реквизитов оператора и
> юридической проверки фактического deployment.

## 1. Оператор

- Наименование / ФИО: `[ЗАПОЛНИТЬ]`
- Контакт: `[ЗАПОЛНИТЬ]`
- Адрес: `[ЗАПОЛНИТЬ]`

## 2. Сервис

Telegram-бот взаимопомощи велосипедистов `[BOT_USERNAME]`.

## 3. Категории данных

Планируемая минимальная модель:
- Telegram user ID;
- отображаемые Telegram-данные, если они нужны для текущего взаимодействия;
- настройки профиля помощника;
- выбранные районы;
- сведения о заявке;
- точная геопозиция, которую пользователь явно отправил для активной заявки;
- timestamps и минимальные технические журналы.

## 4. Цели

Заполнить таблицу:

| Данные | Цель | Основание | Получатели | Срок/условие удаления |
|---|---|---|---|---|
| Telegram ID | идентификация внутри бота | `[ ]` | `[ ]` | `[ ]` |
| Helper areas | matching | `[ ]` | оператор | `[ ]` |
| Exact request point | оказание помощи | `[ ]` | matched helper | `[ ]` |

## 5. Передача точной геопозиции

Точная точка заявки не должна передаваться всем кандидатам. Она раскрывается
только помощнику, который успешно принял открытую заявку, и только в объёме,
необходимом для оказания помощи.

## 6. Права субъекта

Описать доступные способы:
- получить сведения;
- исправить;
- отозвать согласие, если применимо;
- запросить прекращение обработки/удаление в предусмотренных законом пределах.

## 7. Хранение и инфраструктура

`[ОПИСАТЬ ФАКТИЧЕСКУЮ СХЕМУ И ЛОКАЦИЮ БД/БЭКАПОВ]`

## 8. Контакт

`[ЗАПОЛНИТЬ]`

---

## EN summary / Инженерное резюме

> Not a final legal document. Do not publish without filling in the operator
> details and a legal review of the actual deployment.

- Operator, contact, and address MUST be filled in before launch.
- Service: cyclist mutual-aid Telegram bot `[BOT_USERNAME]`.
- Minimal data model: Telegram user ID, displayed Telegram data as needed,
  helper settings, selected areas, request data, exact geo-point explicitly
  sent for the active request, timestamps and minimal technical logs.
- Purposes, legal bases, recipients, and retention/deletion conditions MUST be
  filled in per field; the table in §4 is normative scaffolding.
- Exact request point MUST NOT be fanned out to all candidates. It is disclosed
  only to the helper who won the atomic accept, and only as needed to provide
  help.
- Subject rights (access, correction, consent withdrawal where applicable,
  cessation/deletion within legal limits) MUST be described with working
  contact channels.
- Actual DB/backup scheme and physical location MUST be described; backups
  inherit the same access/localization/retention rules.
