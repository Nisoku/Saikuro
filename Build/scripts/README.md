# Build Scripts

Per-language build/check scripts used by the `Justfile` and CI.

## Usage

Don't call these directly — use the `Justfile` from the repo root:

```bash
just rust check       # fmt (auto-fix) + clippy + tests + wasm check
just python check     # ruff lint (auto-fix) + pytest
just typescript check # eslint (auto-fix) + tsc + vitest + tsup
just csharp check     # dotnet format (auto-fix) + build + tests
just c check          # build + test C adapter
just cpp check        # cmake config + header compile test
just check            # all of the above
just web_demo         # build + run the polyglot WASM demo
```

Each script supports subcommands:

```bash
python3 rust.py fmt_check   # check + auto-fix formatting
python3 python.py lint       # check + auto-fix lint
python3 typescript.py test   # just run tests
python3 web_demo.py dev      # build WASM and start Vite
```

The orchestrator at `saikuro_build.py` runs all language checks
and is equivalent to `just check`.

## Emscripten (emsdk)

The web demo requires Emscripten to compile the C/C++ providers. The `just setup` command attempts to install `emsdk` automatically when `emcc` is not found.

If you prefer to install `emsdk` manually, run:

```bash
git clone https://github.com/emscripten-core/emsdk.git ~/.emsdk
cd ~/.emsdk
./emsdk install latest
./emsdk activate latest
# Add to your shell rc (bash/zsh/fish):
source ~/.emsdk/emsdk_env.sh
```

After installing, verify with:

```bash
emcc --version
```

If you're on macOS you can also install via Homebrew (may be older):

```bash
brew install emscripten
```
