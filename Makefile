unexport VIRTUAL_ENV CONDA_PREFIX

.PHONY: install build test lint fmt clean

install:  ## Create venv and install package + dev deps
	uv sync

build:  ## Rebuild the Rust extension into the venv
	uv run maturin develop --uv

test: build  ## Run Rust and Python tests
	cargo test
	uv run pytest

lint:  ## Check formatting and lints
	cargo fmt --check
	cargo clippy -- -D warnings

fmt:  ## Format Rust code
	cargo fmt

clean:
	cargo clean
	rm -rf .venv .pytest_cache dist
