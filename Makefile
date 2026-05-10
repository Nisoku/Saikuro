.PHONY: setup build test wasm-check wasm-test fmt clippy python-setup python-test ts-setup ts-test check dotnet-install rust-setup

export DOTNET_ROOT := $(HOME)/.dotnet
export PATH := $(DOTNET_ROOT):$(PATH)
export PYTHON := python3

# Rust

rust-setup:
	rustup target add wasm32-unknown-unknown

build:
	cd Build && cargo build --workspace

test:
	cd Build && cargo test --workspace

wasm-check:
	cd Build && cargo check --target wasm32-unknown-unknown -p saikuro-tests

wasm-test:
	cd Build && wasm-pack test --headless --firefox -p saikuro-tests

fmt:
	cd Build && cargo fmt --all

clippy:
	cd Build && cargo clippy --workspace -- -D warnings

# dotnet

dotnet-install:
	@if ! command -v dotnet >/dev/null 2>&1; then \
		echo "Installing dotnet SDK..."; \
		curl -sSL https://dot.net/v1/dotnet-install.sh | bash -s -- --channel 8.0; \
		echo "dotnet installed."; \
	else \
		echo "dotnet already installed"; \
	fi
	@if ! command -v dotnet >/dev/null 2>&1; then \
		echo "Add ~/.dotnet to your PATH or run: export PATH=\"$$HOME/.dotnet:\$$PATH\""; \
		exit 1; \
	fi

# Python (using uv)

python-setup:
	cd Build/adapters/python && uv sync --extra dev --extra websocket

python-test:
	cd Build/adapters/python && uv run pytest

python-lint:
	cd Build/adapters/python && uv run ruff check .

# TypeScript

ts-setup:
	cd Build/adapters/typescript && npm install

ts-test:
	cd Build/adapters/typescript && npm test

ts-lint:
	cd Build/adapters/typescript && npm run lint

ts-typecheck:
	cd Build/adapters/typescript && npm run typecheck

# All

setup: rust-setup python-setup ts-setup dotnet-install
	cd Build && cargo build --workspace

check: fmt clippy test wasm-check python-test ts-test
	@echo "All checks passed!"
