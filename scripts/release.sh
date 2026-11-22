#!/usr/bin/env bash
# Local release build: macOS arm64 + x64, Windows x64 (cargo-xwin).
#   scripts/release.sh             — build into dist/
#   scripts/release.sh --publish   — build and upload to GitHub release v<version>
# Needs: rustup targets aarch64/x86_64-apple-darwin, x86_64-pc-windows-msvc;
#        cargo-xwin, brew llvm + lld (clang-cl, lld-link), gh.
set -euo pipefail

PUBLISH=0
[[ "${1:-}" == "--publish" ]] && PUBLISH=1

export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:/opt/homebrew/opt/llvm/bin:/opt/homebrew/opt/lld/bin:$PATH"
cd "$(dirname "$0")/.."

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
TAG="v$VERSION"
BIN=session-search

if [[ $PUBLISH == 1 ]]; then
  [[ -z "$(git status --porcelain)" ]] || { echo "Дерево не чистое — закоммитьте изменения" >&2; exit 1; }
  if git rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
    [[ "$(git rev-list -n1 "$TAG")" == "$(git rev-parse HEAD)" ]] ||
      { echo "Тег $TAG указывает не на HEAD" >&2; exit 1; }
  else
    git tag -a "$TAG" -m "$TAG"
  fi
fi

echo "== web"
(cd web && npm ci --silent && npm run build)

echo "== checks"
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked -q

rm -rf dist && mkdir dist
for t in aarch64-apple-darwin x86_64-apple-darwin; do
  echo "== $t"
  cargo build --release --locked --target "$t"
done
cargo xwin build --release --locked --target x86_64-pc-windows-msvc

tar -czf "dist/$BIN-macos-arm64.tar.gz" -C target/aarch64-apple-darwin/release "$BIN"
tar -czf "dist/$BIN-macos-x64.tar.gz" -C target/x86_64-apple-darwin/release "$BIN"
(cd target/x86_64-pc-windows-msvc/release && zip -q -j "$OLDPWD/dist/$BIN-windows-x64.zip" "$BIN.exe")
(cd dist && shasum -a 256 ./* | sed 's#\./##' > SHA256SUMS.txt)
ls -la dist

if [[ $PUBLISH == 1 ]]; then
  git push origin "$TAG"
  if gh release view "$TAG" >/dev/null 2>&1; then
    gh release upload "$TAG" dist/* --clobber
  else
    gh release create "$TAG" dist/* --title "$TAG" --generate-notes
  fi
fi
