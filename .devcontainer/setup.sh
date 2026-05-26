#!/bin/bash
set -e

echo "=== Saikuro Development Environment Setup ==="

# System dependencies
echo "Installing system packages..."
sudo apt-get update -qq && sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq \
    cmake \
    clang-format \
    >/dev/null 2>&1

# Rust toolchain
echo "Setting up Rust..."
rustup default stable >/dev/null 2>&1
rustup component add rustfmt clippy >/dev/null 2>&1
rustup target add wasm32-unknown-unknown >/dev/null 2>&1

# Install just (task runner)
if ! command -v just >/dev/null 2>&1; then
    echo "Installing just..."
    cargo install just >/dev/null 2>&1
fi

# uv (Python package manager)
if ! command -v uv >/dev/null 2>&1; then
    echo "Installing uv..."
    curl -LsSf https://astral.sh/uv/install.sh | sh >/dev/null 2>&1
    # Source it for the remainder of this script
    export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"
fi

# .NET SDK (C# adapter)
if ! command -v dotnet >/dev/null 2>&1; then
    echo "Installing .NET SDK 8.0..."
    curl -sSL https://dot.net/v1/dotnet-install.sh -o /tmp/dotnet-install.sh
    chmod +x /tmp/dotnet-install.sh
    /tmp/dotnet-install.sh --channel 8.0 >/dev/null 2>&1
    rm /tmp/dotnet-install.sh
    export PATH="$HOME/.dotnet:$PATH"
fi

# Per-language project setup
echo "Running language setup scripts..."
just setup

# Build everything
echo "Building Rust workspace (dev)..."
cargo build --manifest-path Build/Cargo.toml --workspace

echo "Building Rust workspace (release for tests)..."
cargo build --manifest-path Build/Cargo.toml --workspace --release

echo "Verifying wasm32 compilation..."
cargo check --target wasm32-unknown-unknown --no-default-features \
    --features wasm,wasm-storage -p saikuro \
    --manifest-path Build/Cargo.toml

echo ""
echo "=== Setup complete ==="
echo "Run 'just check' to run all language checks."
echo "Run 'just test' to run all language tests."
