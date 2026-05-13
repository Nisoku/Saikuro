"""
saikuro-schema

CLI tool for extracting Saikuro schema from Python source files.

Usage::

    saikuro-schema --namespace my-service path/to/module.py
    saikuro-schema --namespace my-service --output schema.json path/to/module.py
    saikuro-schema --namespace my-service --pretty path/to/module.py

The tool imports the specified Python file as a module and reflects on all
top-level functions that are exported (not prefixed with ``_``).  It uses the
same ``SchemaBuilder`` machinery used by the provider at runtime, so the
schema produced here matches exactly what the provider would announce.

Exit codes:
    0  Success
    1  Usage error (missing required args, no matching files)
    2  Extraction error (import failure, introspection error, etc.)
"""

from __future__ import annotations

import argparse
import importlib.util
import inspect
import json
import re
import sys
from pathlib import Path
from typing import Callable

from .schema import SchemaBuilder


def _load_module_from_path(path: Path):
    """Import a Python file as a module.  Returns the module object."""
    spec = importlib.util.spec_from_file_location("_saikuro_schema_target", path)
    if spec is None or spec.loader is None:
        raise ImportError(f"Cannot load module from {path}")
    module = importlib.util.module_from_spec(spec)
    # Register in sys.modules before exec so that dataclasses / typing helpers
    # that look up the module by __name__ can find it.
    sys.modules["_saikuro_schema_target"] = module
    # Add the module's directory to sys.path so relative imports inside the
    # target file can resolve.
    module_dir = str(path.parent.resolve())
    added = False
    if module_dir not in sys.path:
        sys.path.insert(0, module_dir)
        added = True
    try:
        spec.loader.exec_module(module)  # type: ignore[union-attr]
    finally:
        if added:
            sys.path.remove(module_dir)
        sys.modules.pop("_saikuro_schema_target", None)
    return module


def _extract_functions(module) -> list[tuple[str, Callable]]:
    """Return all public top-level callables from *module*."""
    results = []
    for name, obj in inspect.getmembers(module, inspect.isfunction):
        if name.startswith("_"):
            continue
        # Only include functions actually defined in this module (not imported
        # from elsewhere).
        if getattr(obj, "__module__", None) != module.__name__:
            continue
        results.append((name, obj))
    return results


def extract_schema(source_path: Path, namespace: str) -> dict:
    """Load *source_path* as a Python module and build a Saikuro schema dict."""
    module = _load_module_from_path(source_path)
    builder = SchemaBuilder(namespace)
    functions = _extract_functions(module)
    for name, fn in functions:
        doc = inspect.getdoc(fn) or ""
        # Capabilities can be annotated via a ``__saikuro_capabilities__``
        # attribute set by the provider decorator, or via a docstring tag
        # ``@capability <token>``.  Support both.
        capabilities: list[str] = list(getattr(fn, "__saikuro_capabilities__", []))
        if not capabilities:
            for m in re.finditer(r"@capability\s+(\S+)", doc):
                capabilities.append(m.group(1))
        builder.add_function(name, fn, capabilities, doc)
    return builder.build()


def main(argv: list[str] | None = None) -> int:
    """Entry point for the ``saikuro-schema`` CLI."""
    parser = argparse.ArgumentParser(
        prog="saikuro-schema",
        description="Extract Saikuro schema from a Python source file.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  saikuro-schema --namespace math-service src/functions.py
  saikuro-schema --namespace api --pretty --output schema.json src/handlers.py
""",
    )
    parser.add_argument(
        "--namespace",
        "-n",
        required=True,
        metavar="NAME",
        help="Namespace to assign in the schema",
    )
    parser.add_argument(
        "--output",
        "-o",
        metavar="FILE",
        default=None,
        help="Write JSON to this file instead of stdout",
    )
    parser.add_argument(
        "--pretty",
        "-p",
        action="store_true",
        help="Pretty-print the JSON output (2-space indent)",
    )
    parser.add_argument(
        "file",
        metavar="FILE",
        help="Python source file to analyze",
    )

    args = parser.parse_args(argv)

    source_path = Path(args.file).resolve()
    if not source_path.exists():
        print(f"Error: file not found: {args.file}", file=sys.stderr)
        return 1
    if not source_path.is_file():
        print(f"Error: not a file: {args.file}", file=sys.stderr)
        return 1

    try:
        schema = extract_schema(source_path, args.namespace)
    except Exception as exc:
        print(f"Extraction error: {exc}", file=sys.stderr)
        return 2

    indent = 2 if args.pretty else None
    text = json.dumps(schema, indent=indent)

    if args.output:
        out_path = Path(args.output).resolve()
        try:
            out_path.write_text(text + "\n", encoding="utf-8")
            rel = (
                out_path.relative_to(Path.cwd())
                if out_path.is_relative_to(Path.cwd())
                else out_path
            )
            print(f"Schema written to {rel}", file=sys.stderr)
        except Exception as exc:
            print(f"Error writing output file: {exc}", file=sys.stderr)
            return 2
    else:
        print(text)

    return 0


if __name__ == "__main__":
    sys.exit(main())
