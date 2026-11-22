#!/usr/bin/env bash
# Сторожа проекта — тесты с меткой @guard: обходят дерево и падают на КЛАССЕ
# нарушения (файл длиннее бюджета, обход транспорта, кириллица вне consts,
# глубокий импорт, inline style, длинный компонент/функция, cfg вне platform/,
# слои ядра). Отвечают за секунды: «не нарушил ли я правило проекта».
#
# Общий текст для ozma Session Search и ozma AI VPN Closer
# (scratchpad/shared-guards). Папка фронта — ui/ или web/, какая есть.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
FAILED=0

# Тест, который перечисляет файлы дерева, обязан быть помечен @guard — иначе
# он не попадёт в этот прогон.
WALKS='git ls-files\|globSync\|readdirSync\|read_dir'
MIN_FRONT=1
MIN_RUST=1

FRONT=""
for d in ui web; do
  [ -f "$d/package.json" ] && grep -q '"vitest"' "$d/package.json" && FRONT=$d && break
done

echo "Сторожа проекта"

# --- фронт (vitest) -----------------------------------------------------------
if [ -z "$FRONT" ]; then
  echo "-- фронт: не найден (ui/ или web/)"; FAILED=1
else
  # В pathspec git `*` захватывает и `/`: 'src/*.test.ts' — все тесты дерева,
  # включая src/guards.test.ts (а 'src/**/…' верхний уровень пропускает).
  ALL=$(cd "$FRONT" && git ls-files --cached --others --exclude-standard 'src/*.test.ts' 'src/*.test.tsx' | sort)
  GUARDS=$(printf '%s\n' "$ALL" | (cd "$FRONT" && xargs grep -l '@guard' 2>/dev/null) | sort)
  WALKERS=$(printf '%s\n' "$ALL" | (cd "$FRONT" && xargs grep -l "$WALKS" 2>/dev/null) | sort)
  UNTAGGED=$(comm -23 <(printf '%s\n' "$WALKERS") <(printf '%s\n' "$GUARDS") | grep . || true)
  COUNT=$(printf '%s\n' "$GUARDS" | grep -c . || true)
  echo "-- $FRONT: $COUNT сторож(ей): $(printf '%s ' $GUARDS)"
  if [ "$COUNT" -lt "$MIN_FRONT" ]; then
    echo "   ожидалось не меньше $MIN_FRONT — поиск сторожей сломан"; FAILED=1
  elif [ -n "$UNTAGGED" ]; then
    echo "   обходят дерево без метки @guard:"; printf '      %s\n' $UNTAGGED; FAILED=1
  else
    # Пути уже относительно $FRONT — запускаем там же, без второго cd.
    # shellcheck disable=SC2086
    (cd "$FRONT" && npx --no-install vitest run --reporter=dot $GUARDS) || FAILED=1
  fi
fi

# --- Rust (cargo test) --------------------------------------------------------
RUST_GUARDS=$(grep -l '@guard' crates/*/tests/*.rs 2>/dev/null || true)
RUST_WALKERS=$(grep -l "$WALKS" crates/*/tests/*.rs 2>/dev/null || true)
RUST_UNTAGGED=$(comm -23 <(printf '%s\n' "$RUST_WALKERS" | sort) <(printf '%s\n' "$RUST_GUARDS" | sort) | grep . || true)
RUST_COUNT=$(printf '%s\n' "$RUST_GUARDS" | grep -c . || true)
echo "-- rust: $RUST_COUNT сторож(ей): $(printf '%s ' $RUST_GUARDS)"
if [ "$RUST_COUNT" -lt "$MIN_RUST" ]; then
  echo "   ожидалось не меньше $MIN_RUST — поиск сторожей сломан"; FAILED=1
elif [ -n "$RUST_UNTAGGED" ]; then
  echo "   обходят дерево без метки @guard:"; printf '      %s\n' $RUST_UNTAGGED; FAILED=1
else
  for f in $RUST_GUARDS; do
    crate_dir=${f%/tests/*}
    pkg=$(sed -n 's/^name = "\(.*\)"/\1/p' "$crate_dir/Cargo.toml" | head -1)
    cargo test --locked -q -p "$pkg" --test "$(basename "$f" .rs)" || FAILED=1
  done
fi

if [ "$FAILED" -ne 0 ]; then
  echo "Сторожа нашли нарушения"; exit 1
fi
echo "Сторожа: чисто"
