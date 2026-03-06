SHELL := /usr/bin/env bash

.PHONY: dev test build release package package-repos ci-local fmt lint clean

dev:
	@echo "Starting ForgeISO developer workflow"
	@echo "Run GUI dev server in one terminal: cd gui && npm run dev"
	@echo "Run CLI doctor in another terminal: cargo run -p forgeiso-cli -- doctor"
	@echo "Run TUI in another terminal: cargo run -p forgeiso-tui"

test:
	cargo test --workspace
	cd agent && go test ./...

build:
	cargo build --workspace --release
	cd agent && go build -o ../target/release/forgeiso-agent ./cmd/forgeiso-agent
	@echo "GUI build: cd gui && npm ci && npm run build"

release:
	@echo "Release workflow is tag-driven via .github/workflows/release.yml"
	@echo "Use: git tag vX.Y.Z && git push origin vX.Y.Z"

package:
	@echo "Packaging requires release binaries in target/release"
	@echo "Running package scripts for tar.gz, tar.zst, RPM, DEB, and Pacman"
	scripts/release/clean-release-dir.sh
	scripts/release/package-tarball.sh
	scripts/release/package-zstd.sh
	scripts/release/package-rpm.sh
	scripts/release/package-deb.sh
	scripts/release/package-pacman.sh
	scripts/release/build-repos.sh

package-repos:
	@echo "Generating repository metadata for apt, dnf/yum, and pacman"
	scripts/release/build-repos.sh

ci-local:
	docker compose -f docker-compose.ci.yml up --build --abort-on-container-exit --exit-code-from c1

fmt:
	cargo fmt --all
	cd agent && gofmt -w $$(find . -name '*.go' -type f)

lint:
	cargo clippy --workspace --all-targets -- -D warnings
	cd agent && go vet ./...
	cd gui && npm run lint

clean:
	cargo clean
	cd agent && rm -rf bin
	cd gui && rm -rf node_modules dist src-tauri/target
	docker compose -f docker-compose.ci.yml down -v --remove-orphans || true
