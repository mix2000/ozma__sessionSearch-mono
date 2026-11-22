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
  продолжить в своём терминале. Уже открытый разговор отмечен «открыт в
  терминале (pid …)».

## Безопасность

В разговорах бывает всё, что вставляли в чат, в том числе секреты; теперь это
есть и в индексе.

- Сервер слушает только `127.0.0.1`. `--host` для другого адреса открывает
  разговоры сети — только вместе с `--token`.
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
cd web && npm ci && npm run build && cd ..   # интерфейс → web/dist (встраивается в бинарь)
cargo build --release                         # target/release/session-search
cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check
cd web && npm run dev                         # интерфейс с прокси /api → 127.0.0.1:4477
```

- `crates/core` — источники (`parse/claude|codex|grok`), индекс SQLite, поиск,
  открытые процессы, запуск терминала.
- `crates/app` — сервер axum, встроенный интерфейс, командная строка.
- `web/` — React + antd.
- CI (`.github/workflows/check.yml`) — только проверки на macOS и Windows.

## Релиз

Собирается локально на Mac — macOS arm64, x64 и Windows x64:

```bash
scripts/release.sh             # собрать в dist/
scripts/release.sh --publish   # + тег v<версия из Cargo.toml> и загрузка в GitHub Release
```

Нужны: `rustup target add aarch64-apple-darwin x86_64-apple-darwin
x86_64-pc-windows-msvc`, `cargo install cargo-xwin`, `brew install llvm lld`, `gh`.
