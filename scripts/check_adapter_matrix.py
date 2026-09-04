#!/usr/bin/env python3
"""Cross-compile adapter crates across the full engine x target matrix.

Engines and their targets / feature sets:

  native      host (current)        default features            std
  native (ws) host (current)        --features ws               std
  wasm        wasm32-unknown-unknown --no-default-features       no_std / std
  wasi p1     wasm32-wasip1         --no-default-features       no_std / std / ws
  wasi p2     wasm32-wasip2         --no-default-features       no_std / std / ws

Usage:
  python3 scripts/check_adapter_matrix.py
  python3 scripts/check_adapter_matrix.py --crate saikuro
  python3 scripts/check_adapter_matrix.py --crate saikuro-c
  python3 scripts/check_adapter_matrix.py --all-adapters
  python3 scripts/check_adapter_matrix.py --json out.json
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass, field
from typing import Optional

CARGO = shutil.which("cargo") or "/Users/neel/.cargo/bin/cargo"
ROOT = os.path.dirname(os.path.abspath(__file__))
if os.path.basename(ROOT) == "scripts":
    ROOT = os.path.dirname(ROOT)
MANIFEST = os.path.join(ROOT, "Build", "Cargo.toml")

DEFAULT_CRATE = "saikuro-c"
TIMEOUT = 600  # seconds per check

ADAPTER_CRATES = ["saikuro", "saikuro-c"]


@dataclass
class Combo:
    name: str
    target: Optional[str]
    cargo_args: list[str] = field(default_factory=list)
    notes: str = ""


MATRIX: list[Combo] = [
    Combo(
        "native (host, std)",
        None,
        [],
        "default features",
    ),
    Combo(
        "native (ws)",
        None,
        ["--features", "ws"],
        "default features + websocket transport",
    ),
    Combo(
        "wasm (std)",
        "wasm32-unknown-unknown",
        ["--no-default-features", "--features", "std,wasm"],
        "std wasm",
    ),
    Combo(
        "wasm (no_std)",
        "wasm32-unknown-unknown",
        ["--no-default-features", "--features", "wasm"],
        "no_std wasm",
    ),
    Combo(
        "embedded (no_std) (Cortex-M3 / RP2350-class)",
        "thumbv7m-none-eabi",
        ["--no-default-features", "--features", "embedded,tcp-net"],
        "no_std embedded",
    ),
    Combo(
        "embedded (no_std) (Cortex-M0+ / RP2040)",
        "thumbv6m-none-eabi",
        ["--no-default-features", "--features", "embedded,tcp-net"],
        "no_std embedded",
    ),
    Combo(
        "embedded (no_std) (Cortex-M33 / RP2350)",
        "thumbv8m.main-none-eabihf",
        ["--no-default-features", "--features", "embedded,tcp-net"],
        "no_std embedded",
    ),
    Combo(
        "embedded (no_std) (AArch64 bare-metal)",
        "aarch64-unknown-none",
        ["--no-default-features", "--features", "embedded,tcp-net"],
        "no_std embedded",
    ),
    Combo(
        "embedded (no_std) (RISC-V 32 IMAC)",
        "riscv32imac-unknown-none-elf",
        ["--no-default-features", "--features", "embedded,tcp-net"],
        "no_std embedded",
    ),
    Combo(
        "embedded (no_std) (RISC-V 32 IMC / ESP32-C3)",
        "riscv32imc-unknown-none-elf",
        ["--no-default-features", "--features", "embedded,tcp-net"],
        "no_std embedded",
    ),
    Combo(
        "wasi preview1 (no_std)",
        "wasm32-wasip1",
        [
            "--no-default-features",
            "--features",
            "no_std,wasi-tcp,wasi-host,wasi-preview1",
        ],
        "no_std wasi (preview1)",
    ),
    Combo(
        "wasi preview1 (std)",
        "wasm32-wasip1",
        [
            "--no-default-features",
            "--features",
            "std,no_std,wasi-tcp,wasi-host,wasi-preview1",
        ],
        "std wasi (preview1)",
    ),
    Combo(
        "wasi preview2 (no_std)",
        "wasm32-wasip2",
        [
            "--no-default-features",
            "--features",
            "no_std,wasi-tcp,wasi-host,wasi-preview2",
        ],
        "no_std wasi (preview2)",
    ),
    Combo(
        "wasi preview2 (std)",
        "wasm32-wasip2",
        [
            "--no-default-features",
            "--features",
            "std,no_std,wasi-tcp,wasi-host,wasi-preview2",
        ],
        "std wasi (preview2)",
    ),
    Combo(
        "wasi preview1 (ws)",
        "wasm32-wasip1",
        [
            "--no-default-features",
            "--features",
            "no_std,wasi-tcp,wasi-host,wasi-preview1,ws-wasi",
        ],
        "no_std wasi websocket client (preview1)",
    ),
    Combo(
        "wasi preview2 (ws)",
        "wasm32-wasip2",
        [
            "--no-default-features",
            "--features",
            "no_std,wasi-tcp,wasi-host,wasi-preview2,ws-wasi",
        ],
        "no_std wasi websocket client (preview2)",
    ),
    Combo(
        "wasi preview1 (std ws)",
        "wasm32-wasip1",
        [
            "--no-default-features",
            "--features",
            "std,no_std,wasi-tcp,wasi-host,wasi-preview1,ws-wasi",
        ],
        "std wasi websocket client (preview1)",
    ),
    Combo(
        "wasi preview2 (std ws)",
        "wasm32-wasip2",
        [
            "--no-default-features",
            "--features",
            "std,no_std,wasi-tcp,wasi-host,wasi-preview2,ws-wasi",
        ],
        "std wasi websocket client (preview2)",
    ),
]


def run_combo(crate: str, combo: Combo, verbose: bool, crate_features: set[str] | None = None) -> dict:
    """Run `cargo check` for one combo; return a result dict."""
    cmd = [CARGO, "check", "-p", crate, "--lib", "--manifest-path", MANIFEST]
    if combo.target:
        cmd += ["--target", combo.target]

    # Filter combo features to only those the crate actually declares.
    args = list(combo.cargo_args)
    if crate_features is not None and "--features" in args:
        fi = args.index("--features")
        raw_feats = args[fi + 1]
        feat_list = [f.strip() for f in raw_feats.split(",")]
        kept = [f for f in feat_list if f in crate_features]
        if not kept:
            return {
                "name": combo.name,
                "target": combo.target or "<host>",
                "features": " ".join(args) or "<default>",
                "notes": combo.notes + " (skipped: no matching features)",
                "passed": True,
                "returncode": 0,
                "errors": 0,
                "warnings": 0,
                "diags": [],
                "seconds": 0.0,
            }
        args[fi + 1] = ",".join(kept)

    cmd += args

    start = time.time()
    env = dict(os.environ)
    env["CARGO_TERM_COLOR"] = "never"
    proc = subprocess.run(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        env=env,
        cwd=ROOT,
        timeout=TIMEOUT,
    )
    elapsed = time.time() - start
    out = proc.stdout.decode("utf-8", errors="replace")
    lines = out.splitlines()
    err_count = sum(1 for line in lines if line.startswith("error"))
    warn_count = sum(
        1 for line in lines if line.startswith("warning") and "generated" not in line
    )
    diags = []
    in_diag = False
    for line in lines:
        s = line.strip()
        if s.startswith(("error", "warning", "note:", "help:", "-->")):
            in_diag = True
            diags.append(line)
        elif in_diag:
            if s == "":
                in_diag = False
            elif s.startswith(("|", "=", "^", "*")) or line[:1] in (" ", "\t"):
                diags.append(line)
            else:
                in_diag = False
    passed = proc.returncode == 0 and err_count == 0

    if verbose and not passed:
        print(out)

    return {
        "name": combo.name,
        "target": combo.target or "<host>",
        "features": " ".join(combo.cargo_args) or "<default>",
        "notes": combo.notes,
        "passed": passed,
        "returncode": proc.returncode,
        "errors": err_count,
        "warnings": warn_count,
        "diags": diags,
        "seconds": round(elapsed, 1),
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--crate", default=DEFAULT_CRATE, help="crate to check")
    ap.add_argument(
        "--all-adapters",
        action="store_true",
        help="check all adapter crates (saikuro, saikuro-c)",
    )
    ap.add_argument("--json", metavar="PATH", help="write results as JSON")
    ap.add_argument("--verbose", action="store_true", help="print failing output")
    args = ap.parse_args()

    crates = []
    if args.all_adapters:
        crates = ADAPTER_CRATES
    else:
        crates = [args.crate]

    print(f"cargo : {CARGO}")
    print(f"root  : {ROOT}")
    print(f"matrix: {len(MATRIX)} combos x {len(crates)} crate(s)\n")

    results: list[dict] = []
    for crate in crates:
        print(f"=== crate: {crate} ===")

        # Fetch the crate's declared features so we can filter the matrix.
        try:
            meta_raw = subprocess.run(
                [CARGO, "metadata", "--no-deps", "--format-version", "1",
                 "--manifest-path", MANIFEST],
                stdout=subprocess.PIPE,
                check=True,
            ).stdout.decode()
            import json
            meta_pkgs = json.loads(meta_raw)["packages"]
            pkg_features: set[str] = set()
            for p in meta_pkgs:
                if p["name"] == crate:
                    pkg_features = set(p["features"].keys())
                    break
        except Exception:
            pkg_features = None

        for combo in MATRIX:
            r = run_combo(crate, combo, args.verbose, pkg_features)
            status = "PASS" if r["passed"] else "FAIL"
            print(
                f"  [{status}] {r['name']:<22} target={r['target']:<20} "
                f"errs={r['errors']:<3} warns={r['warnings']:<3} {r['seconds']}s"
            )
            results.append({**r, "crate": crate})

    total = len(results)
    passed = sum(1 for r in results if r["passed"])
    warn_total = sum(r["warnings"] for r in results)
    err_total = sum(r["errors"] for r in results)
    print(
        f"\n=== SUMMARY: {passed}/{total} passed "
        f"({warn_total} warnings, {err_total} errors) ==="
    )

    diag_results = [r for r in results if r["warnings"] or r["errors"]]
    if diag_results:
        print("\n=== WARNINGS & ERRORS ===")
        for r in diag_results:
            tag = "FAIL" if not r["passed"] else "WARN"
            print(
                f"\n[{tag}] {r['crate']} :: {r['name']} "
                f"(target={r['target']}, features={r['features']})"
            )
            if r["diags"]:
                for line in r["diags"]:
                    print("  " + line)
            else:
                print("  (no diagnostic lines captured)")

    if args.json:
        import json

        with open(args.json, "w") as fh:
            json.dump(results, fh, indent=2)
        print(f"\nwrote {args.json}")

    return 0 if passed == total else 1


if __name__ == "__main__":
    sys.exit(main())
