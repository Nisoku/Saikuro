//! Semihosting console output for the embedded QEMU runners.

use core::fmt;

#[cfg(target_arch = "riscv32")]
const SYS_WRITE: usize = 0x05;
#[cfg(target_arch = "riscv32")]
const SYS_EXIT: usize = 0x18;
#[cfg(target_arch = "riscv32")]
const STDOUT_FD: usize = 1;
#[cfg(target_arch = "riscv32")]
const ADP_STOPPED_APPLICATION_EXIT: usize = 0x20026;

/// Console device writing to the QEMU semihosting debug channel.
pub struct Console;

impl Console {
    /// Open the semihosting console.
    pub fn new() -> Self {
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
        let block = [STDOUT_FD, bytes.as_ptr() as usize, bytes.len()];
        semihost(SYS_WRITE, block.as_ptr() as usize);
    }
}

impl Default for Console {
    fn default() -> Self {
        Self::new()
    }
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
