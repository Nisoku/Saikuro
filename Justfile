# Saikuro Development Commands
#
#   just              List available commands
#   just rust check   Run all Rust checks
#   just check        Run all language checks

scripts := "Build/scripts"

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

# Demo WASM build recipes (individual, used by dev.mjs watcher)
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

all: setup check

