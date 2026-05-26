#!/bin/bash
set -e

echo "Saikuro Development Environment Setup"

# Rust toolchain
echo "Setting up Rust..."
rustup default stable
rustup component add rustfmt clippy
rustup target add wasm32-unknown-unknown

# Install just
echo "Installing just..."
cargo install just

# Install uv (Python package manager used by the Python adapter)
if ! command -v uv >/dev/null 2>&1; then
    echo "Installing uv..."
    pip install uv
fi

# Project setup via just
just setup

# Build Rust workspace
echo "Building Rust workspace..."
cargo build --manifest-path Build/Cargo.toml --workspace

echo "Setup complete"
