#!/bin/bash
set -e

echo "Saikuro Development Environment Setup"

# Rust toolchain (stable)
echo "Setting up Rust..."
rustup default stable
rustup component add rustfmt clippy
rustup target add wasm32-unknown-unknown

# Python with uv
echo "Setting up Python..."
curl -LsSf https://astral.sh/uv/install.sh | sh
export PATH="$HOME/.cargo/bin:$PATH"

cd Build/adapters/python
uv venv
source .venv/bin/activate
uv pip install -e ".[dev,websocket]"

# Node.js/TypeScript
echo "Setting up TypeScript..."
cd ../typescript
npm install

# Build Rust workspace
echo "Building Rust workspace..."
cd ../../Build
cargo build --workspace

echo "Setup complete"
