//! Result cell for `block_on`.

use alloc::boxed::Box;
use core::any::Any;
use core::cell::RefCell;
use core::future::Future;

use embassy_sync::blocking_mutex::CriticalSectionMutex;

type OutputCell = Option<Box<dyn Any>>;

struct CellWrapper(CriticalSectionMutex<RefCell<OutputCell>>);

// SAFETY: `Box<dyn Any>` is not `Send`, but access is serialized through
// CriticalSectionRawMutex and wasm is single threaded, so the value never
// crosses a thread boundary.
unsafe impl Sync for CellWrapper {}

/// The single in-flight `block_on` output slot. Sound per the module docs.
static CELL: CellWrapper = CellWrapper(CriticalSectionMutex::new(RefCell::new(None)));

/// Pointer to the output cell, for handing to an [`OutputCapture`] on the
/// executor's heap.
///
/// # Safety
///
/// The pointer stays valid for the module's lifetime. It must not be used to
/// touch the cell outside a `block_on` call, and the [`OutputCapture`] holding
/// it must be dropped once `block_on` has claimed the value. Reentrancy, the
/// only way two owners could overlap, panics at `block_on` entry.
pub(crate) fn cell_ptr() -> *mut OutputCell {
    CELL.0.lock(core::cell::RefCell::as_ptr)
}

/// Drop any value left in the cell by an earlier `block_on`.
pub(crate) fn clear() {
    CELL.0.lock(|cell| {
        cell.borrow_mut().take();
    });
}

/// Wrapper future that boxes the inner future's output into the shared cell.
pub(crate) struct OutputCapture<T> {
    inner: T,
    out: *mut OutputCell,
}

impl<T> OutputCapture<T> {
    pub(crate) fn new(inner: T, out: *mut OutputCell) -> Self {
        Self { inner, out }
    }
}

impl<T> Future for OutputCapture<T>
where
    T: Future,
    T::Output: Any,
{
    type Output = ();

    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<()> {
        match unsafe { self.as_mut().map_unchecked_mut(|s| &mut s.inner) }.poll(cx) {
            core::task::Poll::Ready(val) => {
                // SAFETY: `self.out` points at the static cell, which outlives
                // every `block_on` frame, and reentrancy is impossible per the
                // module docs, so this is the only writer. wasm is
                // single-threaded, so no other writer races this store.
                unsafe {
                    (*self.out) = Some(Box::new(val));
                }
                core::task::Poll::Ready(())
            }
            core::task::Poll::Pending => core::task::Poll::Pending,
        }
    }
}

/// Take the output of a completed `block_on` and downcast it to `F`.
pub(crate) fn claim<F: 'static>() -> F {
    let boxed = CELL
        .0
        .lock(|cell| cell.borrow_mut().take())
        .expect("block_on: suspended without an output; the executor lost the future");
    match boxed.downcast::<F>() {
        Ok(val) => *val,
        Err(other) => panic!(
            "block_on: output type mismatch in result cell (expected {} [{:?}], stored [{:?}])",
            core::any::type_name::<F>(),
            core::any::TypeId::of::<F>(),
            other.as_ref().type_id(),
        ),
    }
}

/// Read the cell without consuming it. For the suspend loop's completion check.
pub(crate) fn is_ready() -> bool {
    CELL.0.lock(|cell| cell.borrow().is_some())
}
