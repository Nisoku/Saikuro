# Saikuro Development Commands
#
#   just              List available commands
#   just rust check   Run all Rust checks
#   just check        Run all language checks

# Language-specific commands
rust *args:
    cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- lang rust {{ args }}

python *args:
    cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- lang python {{ args }}

typescript *args:
    cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- lang typescript {{ args }}

csharp *args:
    cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- lang csharp {{ args }}

c *args:
    cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- lang c {{ args }}

cpp *args:
    cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- lang cpp {{ args }}

web_demo *args:
    cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- demo {{ args }}

qemu *args:
    cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- qemu {{ args }}

# Meta commands
setup:
    cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- setup
    cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- lang rust setup
    cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- lang python setup
    cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- lang typescript setup
    cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- lang csharp setup
    cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- lang cpp setup
    cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- qemu setup

lint:
    cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- lint

alias fmt := format
format:
    cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- format

test:
    cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- test
    cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- lang python test
    cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- lang typescript test
    cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- lang csharp test
    cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- lang c test
    cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- lang cpp test

check:
    cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- check

clean:
    cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- clean

# Demo recipes
wasm-c:
    @cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- demo build-c

wasm-cpp:
    @cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- demo build-cpp

wasm-csharp:
    @cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- demo build-csharp

wasm-rust-runtime:
    @cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- demo build-rust-runtime

wasm-rust-provider:
    @cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- demo build-rust-provider

wasm-python:
    @cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- demo build-python

wasm-rust:
    @cargo run --quiet --manifest-path tools/xtask/Cargo.toml -- demo build-rust

wasm-all: wasm-rust wasm-c wasm-cpp wasm-csharp wasm-python wasm-rust-runtime wasm-rust-provider

all: setup check wasm-all qemu check
