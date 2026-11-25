#!/usr/bin/env bash
# Все проверки разом; CI запускает этот же скрипт (и ещё e2e: `make e2e`).
set -euo pipefail
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
cd "$(dirname "$0")/.."

step() { printf '\n== %s\n' "$*"; }

step "web: format:check, lint, typecheck, test:coverage"
(cd web && npm run check)

step "rust: fmt"
cargo fmt --all --check
step "rust: clippy"
cargo clippy --all-targets --locked -- -D warnings
step "rust: test"
cargo test --locked

step "guards"
bash scripts/guards.sh

# Релиз собирается только на Mac.
if [ "$(uname)" = Darwin ]; then
  step "release.sh"
  bash scripts/release-test.sh
fi

printf '\nВсе проверки пройдены.\n'
