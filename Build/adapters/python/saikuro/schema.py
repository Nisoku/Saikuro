"""
Python-side schema introspection and builder.

In development mode the provider announces its schema to the runtime by
reflecting on registered functions using `inspect`.  This module handles
that reflection and builds the schema dict expected by the runtime.
"""

from __future__ import annotations

import inspect
import types
import typing
import logging
from dataclasses import dataclass, field
from typing import Any, Callable, Dict, List, Optional, get_type_hints

logger = logging.getLogger(__name__)


# Descriptors


@dataclass
class ArgDef:
    """A single argument descriptor."""

    name: str
    type_str: str = "any"
    optional: bool = False
    doc: Optional[str] = None


@dataclass
class FunctionDef:
    """A single function descriptor."""

    name: str
    args: List[ArgDef] = field(default_factory=list)
    returns: str = "any"
    capabilities: List[str] = field(default_factory=list)
    visibility: str = "public"
    doc: Optional[str] = None


# Type annotation -> Saikuro type string mapping

_PY_TYPE_MAP: Dict[Any, str] = {
    int: "i64",
    float: "f64",
    bool: "bool",
    str: "string",
    bytes: "bytes",
    type(None): "unit",
}


def _annotation_to_type_str(annotation: Any) -> str:
    """Convert a Python type annotation to a Saikuro type name string.

    Handles common cases; complex generics fall back to "any".
    """
    if annotation is inspect.Parameter.empty or annotation is None:
        return "any"

    if annotation in _PY_TYPE_MAP:
        return _PY_TYPE_MAP[annotation]

    origin = getattr(annotation, "__origin__", None)
    args = getattr(annotation, "__args__", ())

    # Handle PEP 604 union types (X | Y) which are `types.UnionType` on Python 3.10+
    if isinstance(annotation, types.UnionType) or origin is typing.Union:
        # Optional[X] has args (X, NoneType)
        non_none = [a for a in args if a is not type(None)]
        if len(non_none) == 1:
            return _annotation_to_type_str(non_none[0])
        return "any"

    if origin is list:
        inner = _annotation_to_type_str(args[0]) if args else "any"
        return f"array<{inner}>"

    if origin is dict:
        val = _annotation_to_type_str(args[1]) if len(args) >= 2 else "any"
        return f"map<{val}>"

    # Named type: use the class name.
    if hasattr(annotation, "__name__"):
        return annotation.__name__

    return "any"


# Convert a type string (or already-shaped descriptor) to a structural descriptor
def _type_str_to_desc(tstr: Any):
    prims = (
        "bool",
        "i8",
        "i16",
        "i32",
        "i64",
        "u8",
        "u16",
        "u32",
        "u64",
        "f32",
        "f64",
        "string",
        "bytes",
        "any",
        "unit",
    )

    if isinstance(tstr, dict):
        return tstr

    if not isinstance(tstr, str):
        return {"kind": "primitive", "type": "any"}

    if tstr.startswith("array<") and tstr.endswith(">"):
        inner = tstr[len("array<") : -1]
        return {"kind": "list", "item": _type_str_to_desc(inner)}
    if tstr.startswith("map<") and tstr.endswith(">"):
        val = tstr[len("map<") : -1]
        return {
            "kind": "map",
            "key": {"kind": "primitive", "type": "string"},
            "value": _type_str_to_desc(val),
        }
    if tstr.startswith("stream<") and tstr.endswith(">"):
        inner = tstr[len("stream<") : -1]
        return {"kind": "stream", "item": _type_str_to_desc(inner)}
    if tstr in prims:
        return {"kind": "primitive", "type": tstr}
    return {"kind": "named", "name": tstr}


# Schema builder


class SchemaBuilder:
    """Accumulates function definitions and emits a schema announcement dict."""

    def __init__(self, namespace: str) -> None:
        self._namespace = namespace
        self._functions: Dict[str, FunctionDef] = {}

    def add_function(
        self,
        name: str,
        fn: Callable,
        capabilities: List[str],
        doc: str = "",
    ) -> None:
        """Introspect `fn` and add it to the schema."""
        try:
            hints = get_type_hints(fn)
        except Exception as exc:
            # `get_type_hints` can fail for built-in functions or functions
            # with forward references that can't be resolved.  Log the
            # failure so it's visible and fall back to un-typed args.
            logger.warning(
                "schema introspection: could not resolve type hints for %r: %s",
                fn,
                exc,
            )
            hints = {}

        sig = inspect.signature(fn)
        args: List[ArgDef] = []

        for param_name, param in sig.parameters.items():
            if param_name in ("self", "cls"):
                continue
            ann = hints.get(param_name, inspect.Parameter.empty)
            type_str = _annotation_to_type_str(ann)
            optional = param.default is not inspect.Parameter.empty
            args.append(ArgDef(name=param_name, type_str=type_str, optional=optional))

        return_ann = hints.get("return", inspect.Parameter.empty)
        returns = _annotation_to_type_str(return_ann)

        # If the function is a generator (sync or async), encode as a stream
        # descriptor using the inner yielded type when available.
        if inspect.isgeneratorfunction(fn) or inspect.isasyncgenfunction(fn):
            inner = None
            if return_ann is not inspect.Parameter.empty:
                inner_args = getattr(return_ann, "__args__", None)
                if inner_args and len(inner_args) >= 1:
                    inner = inner_args[0]
            inner_str = _annotation_to_type_str(inner) if inner is not None else "any"
            # Represent stream as a special type string stream<...> which the
            # builder will convert into a structural descriptor.
            returns = f"stream<{inner_str}>"

        self._functions[name] = FunctionDef(
            name=name,
            args=args,
            returns=returns,
            capabilities=capabilities,
            visibility="public",
            doc=doc or None,
        )

    def build(self) -> dict:
        """Return the schema announcement dict for this namespace."""
        functions = {}
        for fn_name, fn_def in self._functions.items():
            arg_list = []

            for arg in fn_def.args:
                arg_list.append(
                    {
                        "name": arg.name,
                        "type": _type_str_to_desc(arg.type_str),
                        "optional": arg.optional,
                    }
                )
            # Convert return type string to structural descriptor using the
            # shared top-level helper.
            returns_desc = _type_str_to_desc(fn_def.returns)

            functions[fn_name] = {
                "args": arg_list,
                "returns": returns_desc,
                "visibility": fn_def.visibility,
                "capabilities": fn_def.capabilities,
            }
            if fn_def.doc:
                functions[fn_name]["doc"] = fn_def.doc

        return {
            "version": 1,
            "namespaces": {
                self._namespace: {
                    "functions": functions,
                }
            },
            "types": {},
        }
