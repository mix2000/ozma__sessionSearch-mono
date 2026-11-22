# Обёртка над командами проекта. Порты для e2e задаются в web/playwright.config.ts
# (E2E_MOCK_PORT, E2E_WEB_PORT).
export PATH := /opt/homebrew/opt/rustup/bin:$(HOME)/.cargo/bin:$(PATH)

.PHONY: install check test e2e guards fmt build release coverage

install:            ## зависимости и git-хуки
	npm install
	cd web && npm ci

check:              ## всё, что проверяет CI (без e2e)
	bash scripts/check.sh

test:               ## юнит- и интеграционные тесты обеих частей
	cd web && npm test
	cargo test

e2e:                ## Playwright на моке API (сам поднимает и гасит серверы)
	cd web && npm run test:e2e

guards:             ## сторожа: длина файлов, cfg только в platform/
	bash scripts/guards.sh

fmt:                ## автоформат
	cd web && npm run format
	cargo fmt --all

coverage:           ## покрытие: vitest + cargo llvm-cov (если установлен)
	cd web && npm run test:coverage
	cargo llvm-cov --workspace --summary-only

build:              ## интерфейс + release-бинарь
	cd web && npm run build
	cargo build --release

release:            ## релизные сборки в dist/ (scripts/release.sh)
	bash scripts/release.sh
