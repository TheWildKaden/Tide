.DEFAULT_GOAL := help
.PHONY: help setup doctor test lint fmt tools install build build-iso dry-run verify publish-release clean

PREFIX ?= /usr/local

help: ## Show this help
	@printf 'Tide OS development targets:\n\n'
	@grep -E '^[a-zA-Z_-]+:.*## ' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*## "} {printf "  make %-16s %s\n", $$1, $$2}'

setup: ## Install build and development dependencies (Debian/Ubuntu, sudo)
	./scripts/install-dev-deps.sh

doctor: ## Check the build environment
	./build.sh doctor

test: ## Run all Python, Rust, shell, and asset checks
	./scripts/validate.sh

lint: ## Run rustfmt --check and clippy
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets -- -D warnings

fmt: ## Format Rust code
	cargo fmt --all

tools: ## Build the tide and tidectl binaries (target/release/)
	cargo build --release --workspace --bins

install: tools ## Install tide and tidectl into $(PREFIX)/bin (PREFIX=/usr/local)
	install -Dm755 target/release/tide $(DESTDIR)$(PREFIX)/bin/tide
	install -Dm755 target/release/tidectl $(DESTDIR)$(PREFIX)/bin/tidectl

build: ## Bootstrap, configure, build ISO, write manifest (root/network)
	./build.sh all

build-iso: ## Build only the live ISO
	./build.sh build-iso

dry-run: ## Show what a full build would do without changing anything
	./build.sh --dry-run all

verify: ## Verify checksums of artifacts in output/
	./build.sh verify

publish-release: ## Upload output/ to a GitHub release: TAG=v0.1.0-alpha.1 [CREATE=1]
	@test -n "$(TAG)" || { echo "usage: make publish-release TAG=v0.1.0-alpha.1 [CREATE=1]"; exit 2; }
	./scripts/publish-release.sh "$(TAG)" $(if $(filter 1,$(CREATE)),--create)

clean: ## Remove local build artifacts
	./build.sh clean
