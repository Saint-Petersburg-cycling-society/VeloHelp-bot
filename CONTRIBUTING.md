# Участие / Contributing

## RU

Сообщайте об ошибках и предлагайте улучшения через [Issues](https://github.com/Saint-Petersburg-cycling-society/VeloHelp-bot/issues).
Опишите действие, ожидаемый результат и фактическое поведение. В публичные сообщения не включайте токен бота, Telegram ID, точную геопозицию или рабочую базу данных.

Для изменения кода создайте ветку и pull request в `main`. Прочитайте [AGENTS.md](AGENTS.md): состояние заявки принадлежит SQLite; Telegram отображает его. При изменении поведения обновляйте документацию на русском и английском и соответствующие проверки.

Проверки перед pull request:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-features --locked
```

Для изменений иконок выполните `python assets/icons/export.py --check` после установки зависимостей из `assets/icons/requirements.txt`.
Для тестов токен и реальные аккаунты не нужны. Бот использует локальную SQLite и long polling; инструкция запуска находится в [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md).
Код приложения распространяется под AGPL-3.0-only; границу с MIT-зависимостью `teloxide-ui` необходимо сохранять.

## EN

Report bugs and propose improvements in [Issues](https://github.com/Saint-Petersburg-cycling-society/VeloHelp-bot/issues).
Describe the action, expected outcome and observed behavior. Do not include bot tokens, Telegram IDs, exact locations or production databases in public reports.

Use a branch and a pull request targeting `main`. Read [AGENTS.md](AGENTS.md): SQLite owns request state; Telegram presents it. Behavior changes must update both Russian and English documentation and relevant checks.

Run the formatting, clippy and test commands above before submitting. For icon changes, install `assets/icons/requirements.txt` and run `python assets/icons/export.py --check`.
Tests need neither a token nor real accounts. The bot uses local SQLite and long polling; deployment instructions are in [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md).
Application code is AGPL-3.0-only; preserve the boundary with the MIT-licensed `teloxide-ui` dependency.
