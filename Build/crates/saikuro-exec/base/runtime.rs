use core::future::Future;

/// Heap executor harness.
pub struct Runtime;

pub fn new_runtime() -> Runtime {
    Runtime
}

impl Runtime {
    pub fn new() -> Self {
        Runtime
    }

    pub fn new_multi_thread() -> Self {
        Runtime
    }

    pub fn new_current_thread() -> Self {
        Runtime
    }

    pub fn block_on<F: Future + Send + 'static>(&self, fut: F) -> F::Output
    where
        F::Output: Send + 'static,
    {
        crate::block_on(fut)
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

pub struct RuntimeBuilder {
    _private: (),
}

impl RuntimeBuilder {
    pub fn new_multi_thread() -> Self {
        RuntimeBuilder { _private: () }
    }

    pub fn new_current_thread() -> Self {
        RuntimeBuilder { _private: () }
    }

    pub fn enable_all(self) -> Self {
        self
    }

    pub fn worker_threads(self, _n: usize) -> Self {
        self
    }

    pub fn build(self) -> Runtime {
        Runtime::new()
    }
}
