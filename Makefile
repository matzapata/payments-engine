.PHONY: fmt fmt-check clippy lint test check

fmt:
	cargo fmt

fmt-check:
	cargo fmt --check

clippy:
	cargo clippy --all-targets -- -D warnings

lint: fmt-check clippy

test:
	cargo test

check: lint test
