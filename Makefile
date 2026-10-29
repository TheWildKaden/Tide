.PHONY: help doctor test lint fmt build build-iso clean

help:
	@printf '%s\n' \
		'Tide OS development targets:' \
		'  make doctor    Check the build environment' \
		'  make test      Run Python and Rust tests' \
		'  make lint      Run fmt --check and clippy' \
		'  make fmt       Format Rust code' \
		'  make build     Bootstrap, configure, build ISO, write manifest (requires root/network)' \
		'  make build-iso Build only the live ISO' \
		'  make clean     Remove local build artifacts'

doctor:
	./build.sh doctor

test:
	./scripts/validate.sh

lint:
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets -- -D warnings

fmt:
	cargo fmt --all

build:
	./build.sh all

build-iso:
	./build.sh build-iso

clean:
	./build.sh clean
