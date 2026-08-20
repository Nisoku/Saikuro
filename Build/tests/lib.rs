#![allow(unused_imports, dead_code)]

mod common;

#[path = "saikuro-codegen"]
mod codegen_tests {
    mod c_cpp_codegen;
    mod codegen_output;
}

#[path = "saikuro-core"]
mod core_tests {
    mod cross_language_wire;
    mod envelope_roundtrip;
    mod error_propagation;
    mod invocation;
    mod resource;
    mod value;
}

#[path = "saikuro-exec"]
mod exec_tests {
    mod embassy_cancellation;
    mod embassy_executor;
    mod exec_channels;
    mod exec_concurrency;
    mod exec_select;
}

#[path = "saikuro-net"]
mod net_tests {
    mod embassy_net_loopback;
}

#[path = "saikuro-random"]
mod random_tests {
    mod drbg;
    mod drbg_unseeded;
    mod os_backend;
}

#[path = "saikuro-router"]
mod router_tests {
    mod announce_dispatch;
    mod batch_dispatch;
    mod call_dispatch;
    mod channel_dispatch;
    mod log_dispatch;
    mod provider_registry;
    mod resource_dispatch;
    mod sandbox_dispatch;
    mod stream_dispatch;
}

#[path = "saikuro-runtime"]
mod runtime_tests {
    mod config_capacity;
    mod schema_registration;
}

#[path = "saikuro-schema"]
mod schema_tests {
    mod capability_enforcement;
    mod registry;
    mod schema_validation;
    mod validator;
}

#[path = "saikuro-storage"]
mod storage_tests {
    #[cfg(feature = "flash")]
    mod flash;
    mod inmemory;
    mod util;
}

#[path = "saikuro-transport"]
mod transport_tests {
    #[cfg(feature = "embedded-io")]
    mod embedded_io;
    mod transport_compliance;
    mod transport_memory_stress;
    #[cfg(target_arch = "wasm32")]
    mod transport_wasm_host;
}
