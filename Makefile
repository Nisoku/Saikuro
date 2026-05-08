.PHONY: setup build test fmt clippy python-setup python-test ts-setup ts-test ci dotnet-install

export DOTNET_ROOT := $(HOME)/.dotnet
export PATH := $(DOTNET_ROOT):$(PATH)

# Rust

build:
	cd Build && cargo build --workspace

test:
	cd Build && cargo test --workspace

fmt:
	cd Build && cargo fmt --all

clippy:
	cd Build && cargo clippy --workspace --all-targets -- -D warnings

# dotnet

dotnet-install:
	@if ! command -v dotnet &> /dev/null; then \
		echo "Installing dotnet SDK..."; \
		curl -sSL https://dot.net/v1/dotnet-install.sh | bash -s -- --channel 8.0; \
		echo "dotnet installed."; \
	else \
		echo "dotnet already installed"; \
	fi
	@if ! command -v dotnet &> /dev/null; then \
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

setup: python-setup ts-setup dotnet-install
	cd Build && cargo build --workspace

check: fmt clippy test python-test ts-test
	@echo "All checks passed!"

# CI

ci:
	cd Build && python3 scripts/saikuro_build.py all
