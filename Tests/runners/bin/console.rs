//! Semihosting console and host clock access for the embedded QEMU runners.

use core::fmt;

use portable_atomic::{AtomicU64, Ordering};

#[cfg(target_arch = "riscv32")]
const SYS_OPEN: usize = 0x01;
#[cfg(target_arch = "riscv32")]
const SYS_WRITE: usize = 0x05;
#[cfg(target_arch = "riscv32")]
const SYS_CLOCK: usize = 0x10;
#[cfg(target_arch = "riscv32")]
const SYS_EXIT: usize = 0x18;
#[cfg(target_arch = "riscv32")]
const O_WRONLY: usize = 4;
#[cfg(target_arch = "riscv32")]
const ADP_STOPPED_APPLICATION_EXIT: usize = 0x20026;

#[cfg(target_arch = "riscv32")]
use portable_atomic::AtomicUsize;

#[cfg(target_arch = "riscv32")]
static STDOUT_GUESTFD: AtomicUsize = AtomicUsize::new(0);

/// QEMU's `SYS_CLOCK` returns centiseconds; embassy-time runs at 1 MHz ticks.
const MICROS_PER_CENTISEC: u64 = 10_000;

/// Most recent monotonic timestamp in microsecond ticks, so the clock never
/// reports a smaller value than a previous call.
static LAST_NOW_US: AtomicU64 = AtomicU64::new(0);

/// Console device writing to the QEMU semihosting debug channel.
pub struct Console;

impl Console {
    /// Open the semihosting console.
    pub fn new() -> Self {
        #[cfg(target_arch = "riscv32")]
        {
            let name = b":tt\0";
            let block = [name.as_ptr() as usize, O_WRONLY, 0];
            let fd = semihost(SYS_OPEN, block.as_ptr() as usize);
            STDOUT_GUESTFD.store(fd, Ordering::Relaxed);
        }
        Console
    }

    /// Write all `bytes` to the console.
    #[cfg(target_arch = "arm")]
    pub fn write_all(bytes: &[u8]) {
        // `hstdout` opens a fresh stream each call; QEMU writes it immediately.
        if let Ok(mut stream) = cortex_m_semihosting::hio::hstdout() {
            let _ = stream.write_all(bytes);
        }
    }

    /// Write all `bytes` to the console.
    #[cfg(target_arch = "riscv32")]
    pub fn write_all(bytes: &[u8]) {
        let block = [STDOUT_GUESTFD.load(Ordering::Relaxed), bytes.as_ptr() as usize, bytes.len()];
        semihost(SYS_WRITE, block.as_ptr() as usize);
    }
}

impl Default for Console {
    fn default() -> Self {
        Self::new()
    }
}

/// Read QEMU's monotonic `SYS_CLOCK` (host CPU time, centiseconds) as
/// microsecond embassy-time ticks.
#[cfg(target_arch = "arm")]
fn clock_us() -> u64 {
    // SAFETY: SYS_CLOCK (0x10) takes no argument block; a null pointer is
    // accepted by QEMU's `arm-compat-semi.c` handler.
    unsafe {
        (cortex_m_semihosting::syscall1(cortex_m_semihosting::nr::CLOCK, 0) as u64)
            .saturating_mul(MICROS_PER_CENTISEC)
    }
}

/// Read QEMU's monotonic `SYS_CLOCK` (host CPU time, centiseconds) as
/// microsecond embassy-time ticks.
#[cfg(target_arch = "riscv32")]
fn clock_us() -> u64 {
    (semihost(SYS_CLOCK, 0) as u64).saturating_mul(MICROS_PER_CENTISEC)
}

/// Monotonic microsecond clock for the embassy-time driver.
///
/// `SYS_CLOCK` never rolls backwards, but clamp through `LAST_NOW_US` anyway so
/// the driver contract holds even if host time is adjusted mid-run.
pub fn time_now_us() -> u64 {
    LAST_NOW_US.fetch_max(clock_us(), Ordering::Relaxed)
}

#[cfg(target_arch = "riscv32")]
fn semihost(op: usize, arg: usize) -> usize {
    let result: usize;
    // SAFETY: the RISC-V semihosting ABI passes the operation in a0 and the
    // argument block pointer in a1, returning the result in a0. `.option
    // norvc` forces the `slli`/`ebreak`/`srai` marker triple to 32-bit
    // instructions so QEMU recognises the sequence.
    unsafe {
        core::arch::asm!(
            ".option push",
            ".option norvc",
            "slli x0, x0, 0x1f",
            "ebreak",
            "srai x0, x0, 7",
            ".option pop",
            inout("a0") op => result,
            in("a1") arg,
            options(nostack),
        );
    }
    result
}

/// Exit the QEMU guest with a process-style status.
///
/// On RV32 the non-extended `SYS_EXIT` path treats only
/// `ADP_Stopped_ApplicationExit` (`0x20026`) as success; anything else maps to
/// a failure status in QEMU.
#[cfg(target_arch = "riscv32")]
pub fn qemu_exit(success: bool) -> ! {
    let arg = if success {
        ADP_STOPPED_APPLICATION_EXIT
    } else {
        1
    };
    let _ = semihost(SYS_EXIT, arg);
    loop {}
}

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        Self::write_all(s.as_bytes());
        Ok(())
    }
}
