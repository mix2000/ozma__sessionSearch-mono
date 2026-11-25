# ozma Session Search

Поиск по тексту разговоров **Claude Code**, **Codex** и **Grok** на своём
компьютере: найти, где что-то обсуждали, увидеть это место с соседними
абзацами и продолжить тот же разговор. Одна программа для macOS и Windows:
индекс, JSON API и веб-интерфейс внутри.

Отдельная версия страницы «Поиск по сессиям» (`/transcripts`) из ozma Project
Builder. API и JSON те же (`/api/agents/transcripts/*`).

## Запуск

```bash
session-search            # индекс + интерфейс, откроется браузер (http://127.0.0.1:4477)
session-search serve --no-open --port 4477
session-search serve --skip-ports 38000,48000   # не занимать эти порты при поиске свободного (или SESSION_SEARCH_SKIP_PORTS)
session-search search "telegram \"report work\"" --json
session-search sync       # обновить индекс
session-search status
```

Готовые сборки — в GitHub Releases (`session-search-macos-arm64`,
`-macos-x64`, `-windows-x64`). На macOS при первом запуске неподписанного файла:
`xattr -d com.apple.quarantine session-search`.

## Что индексируется

| Агент | Где | Продолжить |
|---|---|---|
| Claude Code | `$CLAUDE_CONFIG_DIR/projects` или `~/.claude/projects/<папка>/<id>.jsonl` | `claude --resume <id>` |
| Codex | `$CODEX_HOME/sessions` или `~/.codex/sessions/…/rollout-…-<id>.jsonl` | `codex resume <id>` |
| Grok Build (xAI) | `$GROK_HOME` или `~/.grok/sessions/<папка>/<id>/updates.jsonl` + `summary.json` | `grok --resume <id>` |
| grok-cli (superagent) | `~/.grok/grok.db` (только чтение) | `grok -s <id>` |

На Windows `~` — это `%USERPROFILE%`. Папки можно задать явно: `--claude-root`,
`--codex-root`, `--grok-root` (или `SESSION_SEARCH_*_ROOT`).

Берутся только реплики пользователя и текстовые ответы агента. Не берутся
вывод инструментов, размышления, служебные вставки (`<system-reminder>`,
окружение Codex, AGENTS.md), пересказы после сжатия контекста, разговоры
субагентов.

Индекс — SQLite в папке данных ОС (`~/Library/Application Support/ozma-session-search`,
`%APPDATA%\ozma-session-search`; `--data-dir` / `SESSION_SEARCH_DATA_DIR`).
Файлы разговоров только дописываются, поэтому читается лишь новое: первая
загрузка ~1000 разговоров — около 4 с, дальше — миллисекунды. Обновление: через
секунду после старта, каждые 5 минут и по кнопке «Обновить индекс».

Claude сам удаляет старые транскрипты (через 30 дней по умолчанию). В индексе
текст остаётся, искать можно, продолжить — нет.

## Поиск

- Все слова или «фразы в кавычках» должны встретиться в одной реплике; регистр
  не важен (и для кириллицы), ищется подстрока.
- По умолчанию — интерактивные разговоры; «Включая автоматические» добавляет
  `claude -p`, `codex exec`, `grok -p`.
- В разговоре до трёх совпадений с абзацем до и после; «Контекст» — по три
  реплики вокруг.
- «Продолжить» открывает разговор в новом окне терминала (Terminal или iTerm
  на macOS, Windows Terminal или cmd на Windows). Рядом — команда, чтобы
  продолжить в своём терминале (на Windows — для cmd и PowerShell). Уже открытый разговор отмечен «открыт в
  терминале (pid …)».

## Безопасность

В разговорах бывает всё, что вставляли в чат, в том числе секреты; теперь это
есть и в индексе.

- Сервер слушает только `127.0.0.1`. `--host` для другого адреса открывает
  разговоры сети; без `--token` программа с таким адресом не запустится
  (с версии 0.2.1). Токен нужен длинный случайный (`openssl rand -hex 32`), и
  трафик идёт по HTTP без шифрования — надёжнее оставить `127.0.0.1` и
  подключаться через `ssh -L 4477:127.0.0.1:4477`.
- `--token` / `SESSION_SEARCH_TOKEN`: API требует `Authorization: Bearer` или
  `?token=` (интерфейс запомнит токен из ссылки).
- Запросы с чужим `Host` отклоняются (защита от DNS rebinding), CORS нет.

## API

| Адрес | Что делает |
|---|---|
| `GET /api/agents/transcripts/search?q=&provider=&includeHeadless=` | поиск |
| `GET /api/agents/transcripts/status` | сколько в индексе, источники, когда обновлялся |
| `POST /api/agents/transcripts/sync` | обновить индекс |
| `GET /api/agents/transcripts/:id/context?seq=` | реплики вокруг совпадения |
| `POST /api/agents/transcripts/:id/resume` | открыть в терминале → `{ok, command}` |
| `GET /api/health` | жив ли сервер |

`provider`: `claude` | `codex` | `grok`. Ошибки — `{statusCode, message, error}`.

## Разработка

```bash
make install                                  # npm-зависимости (корень и web/) + git-хуки (husky)
cd web && npm run build && cd ..              # интерфейс → web/dist (встраивается в бинарь)
cargo build --release                         # target/release/session-search
cd web && npm run dev                         # интерфейс с прокси /api → 127.0.0.1:4477
cd web && npm run mock                        # мок API без бэкенда (MOCK_PORT, по умолчанию 4478)
```

- `crates/core` — по слоям: `models/` (типы и DTO), `sources/` (форматы
  Claude/Codex/Grok на диске), `storage/` (индекс SQLite), `services/`
  (индексация, поиск, правила продолжения, открытые процессы), `platform/`
  (всё, что зависит от ОС: запуск терминала, процессы, пути), `engine.rs` — фасад.
- `crates/app` — библиотека + бинарь: `http/` (router, handlers, dto, error,
  middleware, встроенный интерфейс) и `cli/` (аргументы и команды).
- `web/` — React + antd по образцу CheerPortal: `common/services/{api,theme}`,
  `common/{components,hooks,utils,consts,types}`, `providers/`, `layouts/`,
  `entities/transcripts/{search,context,components,utils}` с `models.ts`
  (интерфейсы `I…`), `transcripts.api.ts` и явными реэкспортами в `index.ts`.

## Разработка и проверки

| Команда | Что делает |
|---|---|
| `make check` / `npm run check` | всё, что проверяет CI, кроме e2e: prettier, eslint, tsc, vitest с порогами покрытия, `cargo fmt`, `clippy -D warnings`, `cargo test`, сторожа, условия `release.sh` |
| `make test` / `npm test` | юнит- и интеграционные тесты web и Rust |
| `make e2e` / `npm run test:e2e` | Playwright на моке API: сам поднимает мок (4597) и vite (5197) и гасит их; порты — `E2E_MOCK_PORT`, `E2E_WEB_PORT` |
| `make guards` / `npm run guards` | сторожа `@guard`: web — файл ≤ 500 строк, нет `export *`, сеть только в `common/services/api`, кириллица только в `*.consts.ts`, нет глубоких импортов между сущностями, нет inline-`style`; Rust (`crates/core/tests/guards.rs`) — файл ≤ 500 строк, `cfg(target_os/unix/windows)` только в `platform/`, слои (`models` без I/O и часов, `services` без прямого I/O и конкретных платформ, `platform`/`storage` без `services`) |
| `make coverage` | покрытие: `vitest --coverage` и `cargo llvm-cov` (нужен `cargo install cargo-llvm-cov`) |
| `make fmt` | prettier + `cargo fmt` |
| `make release` | релизные сборки (`scripts/release.sh`) |

В `web/`: `npm run format`, `format:check`, `lint`, `typecheck`, `test`,
`test:coverage`, `test:e2e`, `check`.

Тесты Rust: модульные — рядом с кодом (`#[cfg(test)]`); интеграционные —
`crates/core/tests` (индексация и поиск на фикстурах), `crates/app/tests/http.rs`
(API через `tower::ServiceExt::oneshot`: коды, формы JSON, 403 на чужой Host,
токен), `crates/app/tests/cli.rs` (бинарь через `assert_cmd`), `crates/core/tests/guards.rs` (сторож).

Git-хуки (husky, ставятся `npm install` в корне; сеть не нужна):
- **pre-commit** — гард против `.env`/ключей в индексе, `eslint --fix` + prettier
  по staged-файлам `web/` (lint-staged), `cargo fmt --check` при изменённых `.rs`;
- **pre-push** — по затронутой части: web `lint` + `typecheck` + `test`,
  Rust `clippy -D warnings` + `cargo test` (всё с `--locked`), затем сторожа. Обойти: `git push --no-verify`.

CI (`.github/workflows/check.yml`) — `scripts/check.sh` на macOS и Windows, e2e — на macOS.

## Релиз

Собирается локально на Mac — macOS arm64, x64 и Windows x64:

```bash
scripts/release.sh             # собрать в dist/
scripts/release.sh --publish   # + тег v<версия из Cargo.toml> и загрузка в GitHub Release
```

`--publish` требует зелёный CI на текущем коммите и не перезаписывает уже
выпущенную версию.

Нужны: `rustup target add aarch64-apple-darwin x86_64-apple-darwin
x86_64-pc-windows-msvc`, `cargo install cargo-xwin`, `brew install llvm lld`, `gh`.
