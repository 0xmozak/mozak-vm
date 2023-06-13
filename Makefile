init:
	git config core.hooksPath .githooks

build:
	cargo build

ci: build
	cargo fmt --all --check
	cargo clippy --all-features --all-targets -- -D warnings
