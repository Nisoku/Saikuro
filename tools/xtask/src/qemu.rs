use anyhow::Context;

use crate::paths;
use crate::run;

pub const ARM_TARGETS: &[(&str, &str)] = &[
    ("thumbv7m-none-eabi", "arm"),
    ("thumbv6m-none-eabi", "arm,sqlite"),
    ("thumbv8m.main-none-eabihf", "arm,sqlite"),
];

pub const RISCV_TARGETS: &[(&str, &str)] = &[
    ("riscv32imc-unknown-none-elf", "riscv,sqlite"),
    ("riscv32imac-unknown-none-elf", "riscv,sqlite"),
];

const EMBEDDED_RUNNERS: &[(&str, &str, &[&str])] = &[
    (
        "thumbv7m-none-eabi",
        "arm",
        &[
            "qemu-system-arm",
            "-cpu",
            "cortex-m3",
            "-machine",
            "netduino2",
            "-nographic",
            "-semihosting-config",
            "enable=on,target=native",
            "-kernel",
        ],
    ),
    (
        "thumbv6m-none-eabi",
        "arm,sqlite",
        &[
            "qemu-system-arm",
            "-machine",
            "mps2-an385",
            "-nographic",
            "-semihosting-config",
            "enable=on,target=native",
            "-kernel",
        ],
    ),
    (
        "thumbv8m.main-none-eabihf",
        "arm,sqlite",
        &[
            "qemu-system-arm",
            "-cpu",
            "cortex-m33",
            "-machine",
            "mps2-an505",
            "-nographic",
            "-semihosting-config",
            "enable=on,target=native",
            "-kernel",
        ],
    ),
    (
        "riscv32imc-unknown-none-elf",
        "riscv,sqlite",
        &[
            "qemu-system-riscv32",
            "-M",
            "virt",
            "-bios",
            "none",
            "-nographic",
            "-semihosting-config",
            "enable=on,target=native",
            "-kernel",
        ],
    ),
    (
        "riscv32imac-unknown-none-elf",
        "riscv,sqlite",
        &[
            "qemu-system-riscv32",
            "-M",
            "virt",
            "-bios",
            "none",
            "-nographic",
            "-semihosting-config",
            "enable=on,target=native",
            "-kernel",
        ],
    ),
];

fn root() -> std::path::PathBuf {
    paths::repo_root()
}

fn runner_bin(target: &str) -> &'static str {
    match target {
        "thumbv7m-none-eabi" => "thumbv7m",
        "thumbv6m-none-eabi" => "thumbv6m",
        "thumbv8m.main-none-eabihf" => "thumbv8m",
        "riscv32imc-unknown-none-elf" => "riscv32imc",
        "riscv32imac-unknown-none-elf" => "riscv32imac",
        _ => unreachable!("no runner bin mapped for target {target}"),
    }
}

fn build_args(target: &str, features: &str) -> Vec<String> {
    vec![
        "build".into(),
        "--profile".into(),
        "qemu".into(),
        "--no-default-features".into(),
        "--features".into(),
        features.into(),
        "--target".into(),
        target.into(),
        "--bin".into(),
        runner_bin(target).into(),
        "--manifest-path".into(),
        paths::tests_dir().join("Cargo.toml").display().to_string(),
    ]
}

fn check_args(target: &str, features: &str) -> Vec<String> {
    vec![
        "check".into(),
        "--no-default-features".into(),
        "--features".into(),
        features.into(),
        "--target".into(),
        target.into(),
        "--bin".into(),
        runner_bin(target).into(),
        "--manifest-path".into(),
        paths::tests_dir().join("Cargo.toml").display().to_string(),
    ]
}

fn selftest_cargo(args: &[String]) -> anyhow::Result<()> {
    run::cargo(
        &root(),
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
    )
}

fn qemu_bin_path(target: &str, bin: &str) -> anyhow::Result<std::path::PathBuf> {
    let path = paths::repo_root()
        .join("target")
        .join(target)
        .join("qemu")
        .join(bin);
    if !path.is_file() {
        anyhow::bail!(
            "{} does not exist; run `cargo xtask qemu build` first",
            path.display()
        );
    }
    Ok(path)
}

fn run_runner(target: &str, bin: &str, argv: &[&str]) -> anyhow::Result<()> {
    let el = qemu_bin_path(target, bin)?;
    let (program, args) = argv.split_first().expect("runner argv is never empty");
    let mut args = args.to_vec();
    let el_str = el.display().to_string();
    args.push(el_str.as_str());
    run::run(&root(), program, &args).with_context(|| format!("{bin} under QEMU"))
}

pub fn setup() -> anyhow::Result<()> {
    for bin in ["qemu-system-arm", "qemu-system-riscv32"] {
        if !run::which(bin) {
            anyhow::bail!(
                "{bin} not found. Install QEMU via your package manager \
                 (homebrew: `brew install qemu`; apt: `apt-get install qemu-system`)"
            );
        }
    }
    println!("qemu: system-arm + system-riscv32 present");
    Ok(())
}

pub fn build_arm() -> anyhow::Result<()> {
    let (target, features) = ARM_TARGETS[0];
    selftest_cargo(&build_args(target, features))
}

pub fn build_riscv() -> anyhow::Result<()> {
    let (target, features) = RISCV_TARGETS[1];
    selftest_cargo(&build_args(target, features))
}

pub fn build_all() -> anyhow::Result<()> {
    for (target, features) in ARM_TARGETS.iter().chain(RISCV_TARGETS) {
        selftest_cargo(&build_args(target, features))?;
    }
    Ok(())
}

pub fn check() -> anyhow::Result<()> {
    setup()?;
    for (target, features) in ARM_TARGETS.iter().chain(RISCV_TARGETS) {
        selftest_cargo(&check_args(target, features))?;
    }
    Ok(())
}

pub fn test_embedded() -> anyhow::Result<()> {
    setup()?;
    for (target, features, argv) in EMBEDDED_RUNNERS {
        selftest_cargo(&build_args(target, features))?;
        run_runner(target, runner_bin(target), argv)?;
    }
    Ok(())
}

pub fn run_arm() -> anyhow::Result<()> {
    setup()?;
    build_arm()?;
    let argv = &[
        "qemu-system-arm",
        "-cpu",
        "cortex-m3",
        "-machine",
        "mps2-an385",
        "-nographic",
        "-semihosting-config",
        "enable=on,target=native",
        "-kernel",
    ];
    run_runner("thumbv7m-none-eabi", "thumbv7m", argv)
}

pub fn run_riscv() -> anyhow::Result<()> {
    setup()?;
    build_riscv()?;
    let argv = &[
        "qemu-system-riscv32",
        "-M",
        "virt",
        "-nographic",
        "-semihosting-config",
        "enable=on,target=native",
        "-kernel",
    ];
    run_runner("riscv32imac-unknown-none-elf", "riscv32imac", argv)
}

pub fn clean() -> anyhow::Result<()> {
    selftest_cargo(&["clean".to_string()])
}
