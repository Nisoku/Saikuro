"""QEMU embedded test commands."""

import argparse
import shutil
import subprocess
import sys

from shared.constants import QEMU_DIR
from shared.run import run

ARM_TARGETS = [
    ("thumbv7m-none-eabi", "arm,sqlite"),
    ("thumbv6m-none-eabi", "arm,sqlite"),
    ("thumbv8m.main-none-eabihf", "arm,sqlite"),
]

RISCV_TARGETS = [
    ("riscv32imc-unknown-none-elf", "riscv,sqlite"),
    ("riscv32imac-unknown-none-elf", "riscv,sqlite"),
]

ALL_TARGETS = ARM_TARGETS + RISCV_TARGETS


def _cargo_qemu(target: str, features: str, subcommand: str) -> list[str]:
    return [
        "cargo", subcommand,
        "--profile", "qemu",
        "--no-default-features",
        "--features", features,
        "--target", target,
        "--manifest-path", str(QEMU_DIR / "Cargo.toml"),
    ]


def _cargo_check(target: str, features: str) -> list[str]:
    return [
        "cargo", "check",
        "--no-default-features",
        "--features", features,
        "--target", target,
        "--manifest-path", str(QEMU_DIR / "Cargo.toml"),
    ]


def setup() -> int:
    if shutil.which("qemu-system-arm") and shutil.which("qemu-system-riscv32"):
        return 0
    if sys.platform == "darwin":
        return run(["brew", "install", "qemu"])
    elif sys.platform.startswith("linux"):
        return run(["sh", "-c", "sudo apt-get update && sudo apt-get install -y qemu-system"])
    print("[ERROR] Could not determine how to install QEMU on this platform.", file=sys.stderr)
    return 1


def build(target: str, features: str) -> int:
    return run(_cargo_qemu(target, features, "build"))


def build_arm() -> int:
    target, features = ARM_TARGETS[0]
    return build(target, features)


def build_riscv() -> int:
    target, features = RISCV_TARGETS[-1]
    return build(target, features)


def build_all() -> int:
    for target, features in ALL_TARGETS:
        rc = build(target, features)
        if rc != 0:
            return rc
    return 0


def run_arm() -> int:
    target, features = ARM_TARGETS[0]
    rc = build(target, features)
    if rc != 0:
        return rc
    return run([
        "qemu-system-arm", "-cpu", "cortex-m3", "-machine", "mps2-an385",
        "-nographic", "-semihosting-config", "enable=on,target=native",
        "-kernel", str(QEMU_DIR / "target" / target / "qemu" / "thumbv7m"),
    ])


def run_riscv() -> int:
    target, features = RISCV_TARGETS[-1]
    rc = build(target, features)
    if rc != 0:
        return rc
    return run([
        "qemu-system-riscv32", "-machine", "virt",
        "-nographic", "-semihosting-config", "enable=on,target=native",
        "-kernel", str(QEMU_DIR / "target" / target / "qemu" / "riscv32imac"),
    ])


def check() -> int:
    rc = 0
    for target, features in ALL_TARGETS:
        if run(_cargo_check(target, features)) != 0:
            rc = 1
    return rc


def clean() -> int:
    return run(["cargo", "clean"], cwd=QEMU_DIR)


def main() -> None:
    commands = {
        "setup": setup,
        "build_arm": build_arm,
        "build_riscv": build_riscv,
        "build_all": build_all,
        "run_arm": run_arm,
        "run_riscv": run_riscv,
        "check": check,
        "clean": clean,
    }

    parser = argparse.ArgumentParser(description="QEMU embedded test commands")
    parser.add_argument("command", nargs="?", default="check", choices=list(commands))
    args = parser.parse_args()
    sys.exit(commands[args.command]())


if __name__ == "__main__":
    main()
