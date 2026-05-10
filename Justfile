#!/usr/bin/env -S just --justfile

# Saikuro Development Commands

# Set defaults
export PYTHON := "python3"
export BUILD_DIR := "Build"
export DOTNET_ROOT := env_var("HOME") + "/.dotnet"

# Rust

rust-setup:
    rustup target add wasm32-unknown-unknown

build:
    cd {{BUILD_DIR}} && cargo build --workspace

test:
    cd {{BUILD_DIR}} && cargo test --workspace

wasm-check:
    cd {{BUILD_DIR}} && cargo check --target wasm32-unknown-unknown -p saikuro-tests

wasm-test:
    cd {{BUILD_DIR}} && wasm-pack test --headless --firefox -p saikuro-tests

fmt:
    cd {{BUILD_DIR}} && cargo fmt --all

clippy:
    cd {{BUILD_DIR}} && cargo clippy --workspace -- -D warnings

# dotnet

dotnet-install:
    if ! command -v dotnet >/dev/null 2>&1; then \
        echo "Installing dotnet SDK..."; \
        curl -sSL https://dot.net/v1/dotnet-install.sh | bash -s -- --channel 8.0; \
        echo "dotnet installed."; \
    else \
        echo "dotnet already installed"; \
    fi

# Python (using uv)

python-setup:
    cd {{BUILD_DIR}}/adapters/python && uv sync --extra dev --extra websocket

python-test:
    cd {{BUILD_DIR}}/adapters/python && uv run pytest

python-lint:
    cd {{BUILD_DIR}}/adapters/python && uv run ruff check .

# TypeScript

ts-setup:
    cd {{BUILD_DIR}}/adapters/typescript && npm install

ts-test:
    cd {{BUILD_DIR}}/adapters/typescript && npm test

ts-lint:
    cd {{BUILD_DIR}}/adapters/typescript && npm run lint

ts-typecheck:
    cd {{BUILD_DIR}}/adapters/typescript && npm run typecheck

# All

setup: rust-setup python-setup ts-setup dotnet-install
    cd {{BUILD_DIR}} && cargo build --workspace

check: fmt clippy test wasm-check python-test ts-test
    @echo "All checks passed!"
