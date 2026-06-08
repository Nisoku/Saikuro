from typing import Callable


def run_checks(*steps: tuple[str, Callable[[], int]]) -> int:
    for name, fn in steps:
        code = fn()
        if code != 0:
            print(f"[FAIL] {name} (exit {code})", flush=True)
            return code
    return 0


def main_entry(aliases: dict[str, Callable[[], int]], default: str = "check") -> None:
    import argparse, sys
    parser = argparse.ArgumentParser()
    parser.add_argument("command", nargs="?", default=default, choices=list(aliases))
    args = parser.parse_args()
    sys.exit(aliases[args.command]())
