#!/bin/bash
# PostToolUse-хук: форматирует файл сразу после того, как Claude его правил.
#
# Ставится в репозиторий как .claude/skills-independent хук (.claude/settings.json)
# при добавлении репозитория в builder.
#
# Почему не официальный однострочник `jq -r .tool_input.file_path | xargs npx
# prettier --write`:
#   * он падает, если prettier в проекте не установлен (а это норма — Go, Python);
#   * не понимает монорепо: prettier надо звать из той папки, где лежит его
#     конфиг и node_modules, иначе он либо не найдётся, либо применит чужие
#     правила;
#   * форматирует что попало — включая .md, .lock и файлы вне репозитория.
#
# Логика: нашли форматтер рядом с файлом — применили; не нашли — молча вышли.
# Хук НИКОГДА не мешает работе: любой сбой — это exit 0.
set -uo pipefail

# Payload приходит на stdin (см. hook_event_name=PostToolUse).
PAYLOAD="$(cat)"

command -v jq >/dev/null 2>&1 || exit 0
# stderr глушим: на неожиданном payload jq ругается, и его ругань всплыла бы
# в сессии Claude как ошибка хука — при том, что делать тут всё равно нечего.
FILE="$(printf '%s' "$PAYLOAD" | jq -r '.tool_input.file_path // empty' 2>/dev/null)"
[ -n "$FILE" ] && [ -f "$FILE" ] || exit 0

# Форматируем только код. Без этого prettier полез бы в .md/.json/.lock и
# устраивал шум в диффе там, где его не просили.
case "$FILE" in
  *.ts|*.tsx|*.js|*.jsx|*.mjs|*.cjs|*.css|*.scss|*.vue|*.svelte) ;;
  *.py) ;;
  *.go) ;;
  *.rs) ;;
  *) exit 0 ;;
esac

# Ищем корень пакета вверх от файла: в монорепо у backend/ и frontend/ свои
# node_modules и свой конфиг prettier. Звать форматтер из корня репозитория —
# значит применить не те правила (или не найти его вовсе).
dir="$(cd "$(dirname "$FILE")" 2>/dev/null && pwd)" || exit 0

# ВАЖНО: не выходим за пределы репозитория. Рабочие копии лежат внутри другого
# node-проекта (builder держит их в своей папке), и без этой границы поиск
# уходил бы вверх и подхватывал ЧУЖОЙ prettier с чужими правилами — проверено:
# файл в репозитории без prettier форматировался конфигом builder'а.
repo_root="$(cd "$dir" && git rev-parse --show-toplevel 2>/dev/null)" || repo_root=""

pkg_dir=""
d="$dir"
while [ -n "$d" ] && [ "$d" != "/" ]; do
  if [ -f "$d/package.json" ]; then pkg_dir="$d"; break; fi
  # дошли до корня репозитория — выше не поднимаемся
  [ -n "$repo_root" ] && [ "$d" = "$repo_root" ] && break
  d="$(dirname "$d")"
done

fmt_js() {
  [ -n "$pkg_dir" ] || return 1
  # Только локальный prettier: `npx prettier` без установки полез бы в сеть и
  # тянул случайную версию — на чужом репозитории это недопустимо.
  local bin="$pkg_dir/node_modules/.bin/prettier"
  [ -x "$bin" ] || return 1
  (cd "$pkg_dir" && "$bin" --write --log-level warn "$FILE" >/dev/null 2>&1)
}

fmt_py() {
  command -v ruff >/dev/null 2>&1 && { ruff format -q "$FILE" >/dev/null 2>&1; return 0; }
  command -v black >/dev/null 2>&1 && { black -q "$FILE" >/dev/null 2>&1; return 0; }
  return 1
}

case "$FILE" in
  *.py) fmt_py ;;
  *.go) command -v gofmt >/dev/null 2>&1 && gofmt -w "$FILE" >/dev/null 2>&1 ;;
  *.rs) command -v rustfmt >/dev/null 2>&1 && rustfmt "$FILE" >/dev/null 2>&1 ;;
  *)    fmt_js ;;
esac

# Всегда 0: форматирование — удобство, а не проверка. Ненулевой код здесь
# только зашумил бы сессию Claude ошибками там, где форматтера просто нет.
exit 0
