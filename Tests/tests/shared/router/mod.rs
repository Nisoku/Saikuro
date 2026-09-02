use crate::TestSuite;

pub fn register(suite: &mut TestSuite) {
    call_dispatch::register(suite);
    provider_registry::register(suite);
    batch_dispatch::register(suite);
    announce_dispatch::register(suite);
    channel_dispatch::register(suite);
    stream_dispatch::register(suite);
    resource_dispatch::register(suite);
    log_dispatch::register(suite);
    sandbox_dispatch::register(suite);
}

pub mod announce_dispatch;
pub mod batch_dispatch;
pub mod call_dispatch;
pub mod channel_dispatch;
pub mod log_dispatch;
pub mod provider_registry;
pub mod resource_dispatch;
pub mod sandbox_dispatch;
pub mod stream_dispatch;
