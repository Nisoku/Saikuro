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

# Meta commands
setup:
    cd {{scripts}} && python3 rust.py setup
    cd {{scripts}} && python3 python.py setup
    cd {{scripts}} && python3 typescript.py setup
    cd {{scripts}} && python3 cpp.py setup

format:
    cd {{scripts}} && python3 rust.py fmt_check
    cd {{scripts}} && python3 typescript.py fmt_check
    cd {{scripts}} && python3 python.py fmt_check
    cd {{scripts}} && python3 csharp.py fmt_check
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

# Utilities
dotnet-install:
    @if ! command -v dotnet >/dev/null 2>&1; then \
        echo "Installing dotnet SDK..."; \
        curl -sSL https://dot.net/v1/dotnet-install.sh | bash -s -- --channel 8.0; \
    else \
        echo "dotnet already installed"; \
    fi
