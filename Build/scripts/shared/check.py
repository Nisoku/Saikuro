import sys
from typing import Callable


def run_checks(*steps: tuple[str, Callable[[], int]]) -> int:
    for name, fn in steps:
        code = fn()
        if code != 0:
            print(f"[FAIL] {name} (exit {code})", flush=True)
            return code
    return 0


def main_entry(aliases: dict[str, Callable[[], int]], default: str = "check") -> None:
    cmd = sys.argv[1] if len(sys.argv) > 1 else default
    if cmd in aliases:
        sys.exit(aliases[cmd]())
    print(f"Usage: {sys.argv[0]} <{'|'.join(aliases)}>", flush=True)
    sys.exit(1)
