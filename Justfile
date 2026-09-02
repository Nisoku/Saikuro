# Saikuro Development Commands
#
#   just              List available commands
#   just rust check   Run all Rust checks
#   just check        Run all language checks

scripts := "Build/scripts"
QEMU_DIR := "Build/tests/qemu"

# Language-specific commands
rust *args:
    cd {{scripts}} && python3 rust.py {{args}}

python *args:
    cd {{scripts}} && python3 python.py {{args}}

typescript *args:
    cd {{scripts}} && python3 typescript.py {{args}}

csharp *args:
    cd {{scripts}} && python3 csharp.py {{args}}

c *args:
    cd {{scripts}} && python3 c.py {{args}}

cpp *args:
    cd {{scripts}} && python3 cpp.py {{args}}

web_demo *args:
    cd {{scripts}} && python3 web_demo.py {{args}}

# Meta commands
setup:
    cd {{scripts}} && python3 rust.py setup
    cd {{scripts}} && python3 python.py setup
    cd {{scripts}} && python3 typescript.py setup
    cd {{scripts}} && python3 csharp.py setup
    cd {{scripts}} && python3 cpp.py setup

format:
    cd {{scripts}} && python3 rust.py fmt_check
    cd {{scripts}} && python3 typescript.py fmt_check
    cd {{scripts}} && python3 python.py fmt_check
    cd {{scripts}} && python3 csharp.py fmt_check
    cd {{scripts}} && python3 c.py fmt_check
    cd {{scripts}} && python3 cpp.py fmt_check

test:
    cd {{scripts}} && python3 rust.py test
    cd {{scripts}} && python3 python.py test
    cd {{scripts}} && python3 typescript.py test
    cd {{scripts}} && python3 csharp.py test
    cd {{scripts}} && python3 c.py test
    cd {{scripts}} && python3 cpp.py test

check:
    cd {{scripts}} && python3 rust.py check
    cd {{scripts}} && python3 python.py check
    cd {{scripts}} && python3 typescript.py check
    cd {{scripts}} && python3 csharp.py check
    cd {{scripts}} && python3 c.py check
    cd {{scripts}} && python3 cpp.py check

clean:
	cd {{scripts}} && python3 rust.py clean
	cd {{scripts}} && python3 python.py clean
	cd {{scripts}} && python3 typescript.py clean
	cd {{scripts}} && python3 csharp.py clean
	cd {{scripts}} && python3 c.py clean
	cd {{scripts}} && python3 cpp.py clean
	rm -rf Demo/public/wasm Demo/node_modules Demo/dist

# Demo recipes
wasm-c:
    @cd {{scripts}} && python3 web_demo.py build-c

wasm-cpp:
    @cd {{scripts}} && python3 web_demo.py build-cpp

wasm-csharp:
    @cd {{scripts}} && python3 web_demo.py build-csharp

wasm-rust-runtime:
    @cd {{scripts}} && python3 web_demo.py build-rust-runtime

wasm-rust-provider:
    @cd {{scripts}} && python3 web_demo.py build-rust-provider

wasm-python:
    @cd {{scripts}} && python3 web_demo.py build-python

wasm-rust:
    @cd {{scripts}} && python3 web_demo.py build-rust

wasm-all: wasm-rust wasm-c wasm-cpp wasm-csharp wasm-python wasm-rust-runtime wasm-rust-provider


# QEMU embedded test recipes

qemu-build-arm:
    cargo build --profile qemu --features arm,sqlite --target thumbv7m-none-eabi --manifest-path {{QEMU_DIR}}/Cargo.toml

qemu-build-riscv:
    cargo build --profile qemu --features riscv,sqlite --target riscv32imac-unknown-none-elf --manifest-path {{QEMU_DIR}}/Cargo.toml

qemu-build-all:
    cargo build --profile qemu --features arm,sqlite --target thumbv7m-none-eabi --manifest-path {{QEMU_DIR}}/Cargo.toml
    cargo build --profile qemu --features arm,sqlite --target thumbv6m-none-eabi --manifest-path {{QEMU_DIR}}/Cargo.toml
    cargo build --profile qemu --features arm,sqlite --target thumbv8m.main-none-eabihf --manifest-path {{QEMU_DIR}}/Cargo.toml
    cargo build --profile qemu --features riscv,sqlite --target riscv32imc-unknown-none-elf --manifest-path {{QEMU_DIR}}/Cargo.toml
    cargo build --profile qemu --features riscv,sqlite --target riscv32imac-unknown-none-elf --manifest-path {{QEMU_DIR}}/Cargo.toml

qemu-run-arm: qemu-build-arm
    qemu-system-arm -cpu cortex-m3 -machine mps2-an385 -nographic -semihosting-config enable=on,target=native -kernel {{QEMU_DIR}}/target/thumbv7m-none-eabi/qemu/thumbv7m

qemu-run-riscv: qemu-build-riscv
    qemu-system-riscv32 -machine virt -nographic -semihosting-config enable=on,target=native -kernel {{QEMU_DIR}}/target/riscv32imac-unknown-none-elf/qemu/riscv32imac

qemu-check:
    cargo check --features arm,sqlite --target thumbv7m-none-eabi --manifest-path {{QEMU_DIR}}/Cargo.toml
    cargo check --features arm,sqlite --target thumbv6m-none-eabi --manifest-path {{QEMU_DIR}}/Cargo.toml
    cargo check --features arm,sqlite --target thumbv8m.main-none-eabihf --manifest-path {{QEMU_DIR}}/Cargo.toml
    cargo check --features riscv,sqlite --target riscv32imc-unknown-none-elf --manifest-path {{QEMU_DIR}}/Cargo.toml
    cargo check --features riscv,sqlite --target riscv32imac-unknown-none-elf --manifest-path {{QEMU_DIR}}/Cargo.toml

qemu:
    cargo check --features arm,sqlite --target thumbv7m-none-eabi --manifest-path {{QEMU_DIR}}/Cargo.toml
    cargo check --features riscv,sqlite --target riscv32imac-unknown-none-elf --manifest-path {{QEMU_DIR}}/Cargo.toml

all: setup check wasm-all qemu-check
