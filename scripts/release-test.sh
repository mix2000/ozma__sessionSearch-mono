#!/usr/bin/env bash
# Условия `release.sh --publish` на копии репозитория: gh и npm — заглушки, сеть
# не нужна, ничего не собирается и не публикуется.
set -euo pipefail
cd "$(dirname "$0")/.."
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

mkdir "$TMP/bin" "$TMP/repo"
git init -q "$TMP/repo"
mkdir "$TMP/repo/scripts" "$TMP/repo/web"
cp scripts/release.sh "$TMP/repo/scripts/"
cp Cargo.toml "$TMP/repo/"
touch "$TMP/repo/web/.gitkeep"
git_t() { git -C "$TMP/repo" -c user.name=t -c user.email=t@example.com -c core.hooksPath=/dev/null "$@"; }
git_t add -A
git_t commit -qm init

# FAKE_DRAFT: "" — релиза нет, true — черновик, false — выпущен, error — сбой gh. FAKE_CI — итог прогона.
cat >"$TMP/bin/gh" <<'EOF'
#!/bin/sh
case "$1 $2" in
  "release list")
    # Предупреждение в stderr при удачном ответе не должно мешать.
    echo "warning: a new release of gh is available" >&2
    case "$FAKE_DRAFT" in
      "") ;;
      error) echo "HTTP 401: Bad credentials" >&2; exit 1 ;;
      *) echo "$FAKE_DRAFT" ;;
    esac ;;
  "run list") echo "$FAKE_CI" ;;
  *) echo "неожиданный вызов gh $*" >&2; exit 99 ;;
esac
EOF
# До сборки дошли — дальше не идём.
printf '#!/bin/sh\necho BUILD-STARTED\nexit 7\n' >"$TMP/bin/npm"
chmod +x "$TMP/bin/gh" "$TMP/bin/npm"

# expect <описание> <текст в выводе> <черновик> <CI>
expect() {
  local out
  if out=$(FAKE_DRAFT=$3 FAKE_CI=$4 PATH="$TMP/bin:$PATH" bash "$TMP/repo/scripts/release.sh" --publish 2>&1); then
    echo "release-test: $1 — скрипт не остановился"; exit 1
  fi
  case "$out" in
    *"$2"*) ;;
    *) printf 'release-test: %s — нет «%s» в выводе:\n%s\n' "$1" "$2" "$out"; exit 1 ;;
  esac
  [ -z "$(git_t tag -l)" ] || { echo "release-test: $1 — остался тег"; exit 1; }
}

expect "выпущенная версия" "поднимите версию" false success
expect "черновик" "Остался черновик" true success
expect "сбой gh" "Не удалось узнать, есть ли релиз" error success
expect "CI не было" "не зелёный (нет завершённого прогона)" "" ""
expect "CI красный" "не зелёный (failure)" "" failure
expect "CI зелёный, сборка упала" "BUILD-STARTED" "" success
touch "$TMP/repo/dirty"
expect "грязное дерево" "Дерево не чистое" "" success
echo "release-test: условия публикации в порядке"
