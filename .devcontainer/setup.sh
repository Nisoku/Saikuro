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

# Project setup via just
just setup

# Build Rust workspace
echo "Building Rust workspace..."
cargo build --manifest-path Build/Cargo.toml --workspace

echo "Setup complete"
