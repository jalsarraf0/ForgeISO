SHELL := /usr/bin/env bash

.PHONY: dev test build package ci-local lint clean

dev:
	@echo "CLI: cargo run -p forgeiso-cli -- doctor"
	@echo "TUI: cargo run -p forgeiso-tui"
	@echo "GUI: cd gui && npm run build && cargo run --manifest-path src-tauri/Cargo.toml"

test:
	cargo test --workspace

build:
	cargo build --workspace --release
	@echo "GUI build: cd gui && npm run build && cargo build --manifest-path src-tauri/Cargo.toml --release"

package:
	@echo "Packaging Linux release tarball"
	scripts/release/package-tarball.sh

ci-local:
	docker compose -f docker-compose.ci.yml up --build --abort-on-container-exit --exit-code-from c1
	docker compose -f docker-compose.ci.yml down -v --remove-orphans || true

lint:
	cargo check --workspace
	cd gui && npm run lint
	cargo check --manifest-path gui/src-tauri/Cargo.toml

clean:
	cargo clean
	cd gui && rm -rf dist src-tauri/target
