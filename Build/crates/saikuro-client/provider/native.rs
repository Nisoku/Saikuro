//! Native (tokio) engine handler boxing: closures and futures must be
//! `Send + Sync` so a provider can be driven from a multi-threaded executor.

use core::{future::Future, pin::Pin};

use alloc::{boxed::Box, string::String};

use saikuro_event::Result;

use super::handler::{HandlerArgs, HandlerEntry, RegisterOptions};
use crate::shared::types::Arc;
use crate::Value;

/// A boxed future returned by handler closures.
pub(super) type HandlerFuture = Pin<Box<dyn Future<Output = Result<Value>> + Send>>;

/// A boxed handler that accepts args and returns a result.
pub(super) type BoxedHandler = Arc<dyn Fn(HandlerArgs) -> HandlerFuture + Send + Sync>;

/// Wrap a user-facing handler closure into a boxed handler.
pub(super) fn box_handler<F, Fut>(handler: F) -> BoxedHandler
where
    F: Fn(HandlerArgs) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<Value>> + Send + 'static,
{
    let handler = move |args| Box::pin(handler(args)) as HandlerFuture;
    Arc::from(Box::new(handler) as Box<dyn Fn(HandlerArgs) -> HandlerFuture + Send + Sync>)
}

impl super::Provider {
    /// Register a function handler under the given name.
    pub fn register<F, Fut>(&mut self, name: impl Into<String>, handler: F)
    where
        F: Fn(HandlerArgs) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value>> + Send + 'static,
    {
        self.register_with_options(name, handler, RegisterOptions::default());
    }

    /// Register a function handler with options controlling its schema and
    /// runtime behavior.
    pub fn register_with_options<F, Fut>(
        &mut self,
        name: impl Into<String>,
        handler: F,
        options: RegisterOptions,
    ) where
        F: Fn(HandlerArgs) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value>> + Send + 'static,
    {
        let name = name.into();
        let handler = box_handler(handler);
        self.handlers.insert(
            name,
            HandlerEntry {
                handler,
                schema: options.schema,
            },
        );
    }
}
