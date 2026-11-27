
.PHONY: requirements develop build
requirements:  ## install required dev dependencies
	rustup component add rustfmt
	rustup component add clippy
	cargo install --locked cargo-nextest
	cargo install --locked cargo-llvm-cov
	cargo install --locked wasm-bindgen-cli --version 0.2.130
	rustup target add wasm32-unknown-unknown

develop: requirements  ## install required dev dependencies

build:  ## build release
	cargo build --release --all-features

.PHONY: lint lints fix format
lint:  ## run Clippy for linting, rustfmt for autoformat checks
	cargo clippy --all-features
	cargo clippy -p fsspec_opfs -p fsspec_opfs_js --target wasm32-unknown-unknown -- -D warnings
	cargo fmt --all -- --check
# alias
lints: lint

fix:  ## fix code with rustfmt
	cargo fmt --all
#alias
format: fix

.PHONY: check checks
check:
	cargo check --all-features
	cargo check -p fsspec_opfs -p fsspec_opfs_js --target wasm32-unknown-unknown
# alias
checks: check

.PHONY: test tests test-ci tests-ci
test:  ## run the tests
	cargo test

# alias
tests: test

coverage: test  ## Browser behavior is exercised by the JavaScript tests

.PHONY: dist publish
dist:  ## create dist
	cargo publish --dry-run --allow-dirty

publish:  ## publish to cargo
	cargo publish

# Thanks to Francoise at marmelab.com for this
.DEFAULT_GOAL := help
help:
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

print-%:
	@echo '$*=$($*)'
