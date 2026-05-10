#!/usr/bin/env python3
"""Run all language checks.  Equivalent to `just check`."""

import subprocess
import sys
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent
LANGUAGES = ["rust", "python", "typescript", "csharp", "c", "cpp"]


def main() -> int:
    failures = 0
    for lang in LANGUAGES:
        label = f"{lang.capitalize()} check"
        print(f"=== {label} ===", flush=True)
        r = subprocess.run([sys.executable, str(SCRIPTS / f"{lang}.py"), "check"])
        if r.returncode:
            print(f"[FAIL] {label} (exit {r.returncode})", flush=True)
            failures += 1
        else:
            print(f"[PASS] {label}", flush=True)
        print(flush=True)

    if failures:
        print(f"{failures} language(s) failed", flush=True)
        return 1
    print("All checks passed!", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
