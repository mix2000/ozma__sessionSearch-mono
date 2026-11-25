#!/usr/bin/env bash
# Local release build: macOS arm64 + x64, Windows x64 (cargo-xwin).
#   scripts/release.sh             — build into dist/
#   scripts/release.sh --publish   — build and upload to GitHub release v<version>
#                                    (HEAD must be green in CI; a release is never overwritten)
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
  fi
  COMMIT=$(git rev-parse HEAD)
  DRAFT=$(gh release view "$TAG" --json isDraft -q .isDraft 2>/dev/null || true)
  case "$DRAFT" in
    "") ;;
    true)
      echo "Остался черновик $TAG от прерванной загрузки — удалите его (gh release delete $TAG) и повторите" >&2
      exit 1 ;;
    *) echo "Релиз $TAG уже есть — поднимите версию" >&2; exit 1 ;;
  esac
  # CI проверяет то, чего здесь нет: Windows и e2e.
  CI=$(gh run list --workflow check.yml --commit "$COMMIT" --limit 1 --json conclusion -q '.[0].conclusion // ""')
  [[ "$CI" == success ]] ||
    { echo "CI на $COMMIT не зелёный (${CI:-нет завершённого прогона}) — запушьте и дождитесь" >&2; exit 1; }
fi

echo "== web"
(cd web && npm ci --silent && npm run build)

echo "== checks"
bash scripts/check.sh

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
  # Тег — только после удачной сборки: упавшая не оставляет его на сломанном коммите.
  [[ "$(git rev-parse HEAD)" == "$COMMIT" && -z "$(git status --porcelain)" ]] ||
    { echo "Репозиторий изменился во время сборки — соберите заново" >&2; exit 1; }
  git rev-parse -q --verify "refs/tags/$TAG" >/dev/null || git tag -a "$TAG" -m "$TAG" "$COMMIT"
  git push origin "$TAG"
  gh release create "$TAG" dist/* --title "$TAG" --generate-notes
fi
