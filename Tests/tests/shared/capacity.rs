//! Compile-time per-target test memory budget.

/// Sentinel `Err` payload that the runner maps to a SKIP instead of a FAIL.
///
/// Namespaced under a control byte so no real test error can collide with it.
pub const SKIPPED: &str = "\u{1}skip";

/// Per-target test memory budget in bytes, derived from the chip class.
///
/// Every capacity sits well below its chip's total SRAM because the stack and the
/// static `.bss`/`.data` regions consume memory before the heap gets any of it;
/// the budget therefore reflects what the allocator can realistically hand out.
pub const TEST_CAPACITY: usize = {
    #[cfg(saikuro_test_capacity = "small")]
    {
        64 * 1024
    }
    #[cfg(saikuro_test_capacity = "rpx-class")]
    {
        128 * 1024
    }
    #[cfg(saikuro_test_capacity = "m33-class")]
    {
        256 * 1024
    }
    #[cfg(not(any(
        saikuro_test_capacity = "small",
        saikuro_test_capacity = "rpx-class",
        saikuro_test_capacity = "m33-class"
    )))]
    {
        16 * 1024 * 1024
    }
};

/// True when this build targets a sub-host capacity (i.e. an embedded chip).
pub const fn is_embedded_capacity() -> bool {
    TEST_CAPACITY < 16 * 1024 * 1024
}

/// Return `Err(SKIPPED)` when `need` exceeds the per-target budget, so the caller
/// can bail before allocating a buffer the chip cannot hold.
pub const fn require_capacity(need: usize) -> Result<(), &'static str> {
    if need <= TEST_CAPACITY {
        Ok(())
    } else {
        Err(SKIPPED)
    }
}
