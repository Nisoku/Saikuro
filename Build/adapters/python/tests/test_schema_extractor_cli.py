import json
import subprocess
import sys
from pathlib import Path

from saikuro.cli import extract_schema


FIXTURE_FILE = Path(__file__).parent / "fixtures" / "service.py"


def test_extract_schema_function_direct() -> None:
    schema = extract_schema(FIXTURE_FILE, "parityns")

    assert schema["version"] == 1
    assert "parityns" in schema["namespaces"]

    functions = schema["namespaces"]["parityns"]["functions"]
    assert "add" in functions
    assert "gen_numbers" in functions
    assert "maybe" in functions


def test_schema_cli_stdout_pretty_json() -> None:
    proc = subprocess.run(
        [
            sys.executable,
            "-m",
            "saikuro.cli",
            "--namespace",
            "parityns",
            "--pretty",
            str(FIXTURE_FILE),
        ],
        capture_output=True,
        text=True,
        check=False,
        timeout=30,
    )

    assert proc.returncode == 0, proc.stderr
    out = json.loads(proc.stdout)
    assert out["version"] == 1
    assert "parityns" in out["namespaces"]
    assert "\n  " in proc.stdout


def test_schema_cli_writes_output_file(tmp_path: Path) -> None:
    out_file = tmp_path / "schema.json"

    proc = subprocess.run(
        [
            sys.executable,
            "-m",
            "saikuro.cli",
            "--namespace",
            "parityns",
            "--output",
            str(out_file),
            str(FIXTURE_FILE),
        ],
        capture_output=True,
        text=True,
        check=False,
        timeout=30,
    )

    assert proc.returncode == 0, proc.stderr
    assert out_file.exists()

    schema = json.loads(out_file.read_text(encoding="utf-8"))
    assert "parityns" in schema["namespaces"]


def test_schema_cli_missing_file_returns_usage_error(tmp_path: Path) -> None:
    missing_path = tmp_path / "nonexistent.py"
    proc = subprocess.run(
        [
            sys.executable,
            "-m",
            "saikuro.cli",
            "--namespace",
            "parityns",
            str(missing_path),
        ],
        capture_output=True,
        text=True,
        check=False,
        timeout=30,
    )

    assert proc.returncode == 1
    assert "file not found" in proc.stderr.lower()


def test_schema_cli_extract_failure_returns_error(tmp_path: Path) -> None:
    bad_fixture = tmp_path / "bad_fixture.py"
    bad_fixture.write_text("def broken(:\n    pass\n", encoding="utf-8")

    proc = subprocess.run(
        [
            sys.executable,
            "-m",
            "saikuro.cli",
            "--namespace",
            "parityns",
            str(bad_fixture),
        ],
        capture_output=True,
        text=True,
        check=False,
        timeout=30,
    )

    assert proc.returncode == 2
    assert "extraction error" in proc.stderr.lower()
