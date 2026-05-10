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
```

Each script supports subcommands:

```bash
python3 rust.py fmt_check   # check + auto-fix formatting
python3 python.py lint       # check + auto-fix lint
python3 typescript.py test   # just run tests
```

The orchestrator at `saikuro_build.py` runs all language checks
and is equivalent to `just check`.
