#!/usr/bin/env python3

from __future__ import annotations

import argparse
import importlib
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

try:
    module = importlib.import_module("tuiro")
    TUI = module.TUI
except (ImportError, AttributeError):
    TUI = None


ROOT = Path(__file__).resolve().parents[1]


@dataclass(frozen=True)
class CommandSpec:
    label: str
    cmd: list[str]
    cwd: Path
    optional: bool = False


@dataclass
class CommandResult:
    label: str
    ok: bool
    skipped: bool
    command: str
    cwd: Path
    returncode: int
    output_tail: str


def _tail(text: str, max_lines: int = 20) -> str:
    lines = text.strip().splitlines()
    if not lines:
        return ""
    return "\n".join(lines[-max_lines:])


class Ui:
    def __init__(self) -> None:
        self._tui = TUI(ci_mode=not sys.stdout.isatty()) if TUI else None

    def banner(self, title: str) -> None:
        if self._tui:
            self._tui.banner(title)
        else:
            print(f"\n=== {title} ===")

    def section(self, title: str) -> None:
        if self._tui:
            self._tui.section(title)
        else:
            print(f"\n-- {title} --")

    def step(self, text: str) -> None:
        if self._tui:
            self._tui.step(text)
        else:
            print(f"  * {text}")

    def command(self, cmd: list[str]) -> None:
        if self._tui:
            self._tui.command(cmd)
        else:
            print("    $", " ".join(cmd))

    def info(self, text: str) -> None:
        if self._tui:
            self._tui.info(text)
        else:
            print(f"[INFO] {text}")

    def success(self, text: str) -> None:
        if self._tui:
            self._tui.success(text)
        else:
            print(f"[OK] {text}")

    def warning(self, text: str) -> None:
        if self._tui:
            self._tui.warning(text)
        else:
            print(f"[WARN] {text}")

    def error(self, text: str) -> None:
        if self._tui:
            self._tui.error(text)
        else:
            print(f"[ERROR] {text}")

    def table(self, rows: list[tuple[str, str]]) -> None:
        if self._tui:
            self._tui.table(rows)
        else:
            print("\nSummary:")
            for key, value in rows:
                print(f"  - {key}: {value}")


def _missing_binary(cmd: list[str]) -> bool:
    return shutil.which(cmd[0]) is None


def _has_dotnet_runtime_8() -> bool:
    if shutil.which("dotnet") is None:
        return False
    try:
        proc = subprocess.run(
            ["dotnet", "--list-runtimes"],
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            check=False,
            timeout=5,
        )
    except subprocess.TimeoutExpired:
        return False
    if proc.returncode != 0:
        return False
    return any(line.startswith("Microsoft.NETCore.App 8.") for line in proc.stdout.splitlines())


def run_command(ui: Ui, spec: CommandSpec) -> CommandResult:
    ui.step(spec.label)
    ui.command(spec.cmd)

    if _missing_binary(spec.cmd):
        msg = f"missing binary '{spec.cmd[0]}'"
        if spec.optional:
            ui.warning(f"{spec.label}: {msg}; skipping")
            return CommandResult(spec.label, True, True, " ".join(spec.cmd), spec.cwd, 0, msg)
        ui.error(f"{spec.label}: {msg}")
        return CommandResult(spec.label, False, False, " ".join(spec.cmd), spec.cwd, 127, msg)

    try:
        proc = subprocess.run(
            spec.cmd,
            cwd=spec.cwd,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            check=False,
            timeout=600,
        )
        ok = proc.returncode == 0
        out = _tail(proc.stdout)
    except subprocess.TimeoutExpired as e:
        if spec.optional:
            ui.warning(f"{spec.label} timed out after {e.timeout} seconds; marked optional")
            return CommandResult(
                label=spec.label,
                ok=True,
                skipped=True,
                command=" ".join(spec.cmd),
                cwd=spec.cwd,
                returncode=124,
                output_tail="<timeout>\n" + (e.output or ""),
            )

        ui.error(f"{spec.label} timed out after {e.timeout} seconds")
        return CommandResult(
            label=spec.label,
            ok=False,
            skipped=False,
            command=" ".join(spec.cmd),
            cwd=spec.cwd,
            returncode=124,
            output_tail="<timeout>\n" + (e.output or ""),
        )

    if not ok and spec.optional:
        ui.warning(f"{spec.label} failed (exit {proc.returncode}); marked optional")
        return CommandResult(
            label=spec.label,
            ok=True,
            skipped=True,
            command=" ".join(spec.cmd),
            cwd=spec.cwd,
            returncode=proc.returncode,
            output_tail=out,
        )

    if ok:
        ui.success(f"{spec.label} passed")
    else:
        ui.error(f"{spec.label} failed (exit {proc.returncode})")

    return CommandResult(
        label=spec.label,
        ok=ok,
        skipped=False,
        command=" ".join(spec.cmd),
        cwd=spec.cwd,
        returncode=proc.returncode,
        output_tail=out,
    )


def quality_checks() -> list[CommandSpec]:
    return [
        CommandSpec("Rust fmt", ["cargo", "fmt", "--all", "--", "--check"], ROOT),
        CommandSpec(
            "Rust clippy",
            ["cargo", "clippy", "--workspace", "--all-targets", "--all-features", "--", "-D", "warnings"],
            ROOT,
        ),
        CommandSpec("Rust tests", ["cargo", "test", "--workspace", "--all-features"], ROOT),
    ]


def adapter_checks(name: str) -> list[CommandSpec]:
    ts_dir = ROOT / "adapters" / "typescript"

    adapters = {
        "rust": [
            CommandSpec("Rust adapter tests", ["cargo", "test", "-p", "saikuro"], ROOT),
        ],
        "c": [
            CommandSpec("C adapter build", ["cargo", "build", "-p", "saikuro-c"], ROOT),
            CommandSpec("C adapter tests", ["cargo", "test", "-p", "saikuro-c"], ROOT),
        ],
        "cpp": [
            CommandSpec(
                "C++ configure",
                ["cmake", "-S", ".", "-B", "build"],
                ROOT / "adapters" / "cpp",
                optional=True,
            ),
            CommandSpec(
                "C++ header compile test",
                ["cmake", "--build", "build", "--target", "saikuro_cpp_header_compile_test"],
                ROOT / "adapters" / "cpp",
                optional=True,
            ),
        ],
        "typescript": lambda: [
            CommandSpec("TypeScript deps", ["npm", "install"], ts_dir),
            CommandSpec("TypeScript lint", ["npm", "run", "lint"], ts_dir),
            CommandSpec("TypeScript typecheck", ["npm", "run", "typecheck"], ts_dir),
            *([
                CommandSpec("TypeScript tests", ["npm", "test"], ts_dir)
            ] if _has_dotnet_runtime_8() else [
                CommandSpec("TypeScript tests (skip dotnet8 parity test)", ["npm", "run", "test", "--", "--exclude", "tests/parity_ts_py.test.ts"], ts_dir)
            ]),
            CommandSpec("TypeScript build", ["npm", "run", "build"], ts_dir),
        ],
        "python": [
            CommandSpec(
                "Python adapter deps",
                [sys.executable, "-m", "pip", "install", "-e", ".[dev,websocket]"],
                ROOT / "adapters" / "python",
            ),
            CommandSpec("Python tests", ["pytest"], ROOT / "adapters" / "python"),
        ],
        "csharp": [
            CommandSpec("C# restore", ["dotnet", "restore"], ROOT / "adapters" / "csharp" / "Saikuro" / "src", optional=True),
            CommandSpec(
                "C# build",
                ["dotnet", "build", "src/Saikuro.csproj", "-c", "Release"],
                ROOT / "adapters" / "csharp" / "Saikuro",
                optional=True,
            ),
            CommandSpec(
                "C# tests",
                ["dotnet", "test", "tests/Saikuro.Tests.csproj", "-c", "Release"],
                ROOT / "adapters" / "csharp" / "Saikuro",
                optional=True,
            ),
        ],
    }

    if name == "all":
        ordered = ["rust", "c", "cpp", "typescript", "python", "csharp"]
        merged: list[CommandSpec] = []
        for adapter in ordered:
            item = adapters[adapter]
            items = item() if callable(item) else item
            merged.extend(items)
        return merged

    selected = adapters[name]
    return selected() if callable(selected) else selected


def run_specs(ui: Ui, specs: Iterable[CommandSpec]) -> list[CommandResult]:
    specs_list = list(specs)
    results: list[CommandResult] = []
    for idx, spec in enumerate(specs_list):
        result = run_command(ui, spec)
        results.append(result)
        if not result.ok and not spec.optional:
            skipped = len(specs_list) - (idx + 1)
            ui.error(f"Stopping on first required failure; {skipped} specs skipped")
            break
    return results


def summarize(ui: Ui, results: list[CommandResult]) -> int:
    passed = sum(1 for r in results if r.ok and not r.skipped)
    skipped = sum(1 for r in results if r.skipped)
    failed = sum(1 for r in results if not r.ok)

    rows = [
        ("passed", str(passed)),
        ("failed", str(failed)),
        ("skipped", str(skipped)),
    ]
    ui.table(rows)

    for result in results:
        if result.ok:
            continue
        ui.section(f"Failure details: {result.label}")
        ui.info(f"cwd: {result.cwd}")
        ui.info(f"cmd: {result.command}")
        if result.output_tail:
            print(result.output_tail)

    return 0 if failed == 0 else 1


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Saikuro workspace quality runner")
    subparsers = parser.add_subparsers(dest="command", required=True)

    subparsers.add_parser("quality", help="Run global fmt/clippy/test checks")

    adapter = subparsers.add_parser("adapter", help="Run checks for one adapter")
    adapter.add_argument(
        "name",
        choices=["rust", "c", "cpp", "typescript", "python", "csharp", "all"],
        help="Adapter name",
    )

    subparsers.add_parser("all", help="Run quality checks + all adapter checks")

    return parser.parse_args()


def main() -> int:
    args = parse_args()
    ui = Ui()
    ui.banner("Saikuro Buildscripts")

    if args.command == "quality":
        ui.section("Workspace quality checks")
        return summarize(ui, run_specs(ui, quality_checks()))

    if args.command == "adapter":
        ui.section(f"Adapter checks: {args.name}")
        return summarize(ui, run_specs(ui, adapter_checks(args.name)))

    ui.section("Workspace quality checks")
    quality = run_specs(ui, quality_checks())
    if any(not r.ok for r in quality):
        return summarize(ui, quality)

    ui.section("Adapter checks: all")
    adapter = run_specs(ui, adapter_checks("all"))
    return summarize(ui, quality + adapter)


if __name__ == "__main__":
    raise SystemExit(main())
