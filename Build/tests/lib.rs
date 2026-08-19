mod common;

#[path = "saikuro-codegen"]
mod saikuro_codegen {
    mod c_cpp_codegen;
    mod codegen_output;
}

#[path = "saikuro-core"]
mod saikuro_core {
    mod cross_language_wire;
    mod envelope_roundtrip;
    mod error_propagation;
    mod invocation;
    mod resource;
    mod value;
}

#[path = "saikuro-exec"]
mod saikuro_exec {
    mod embassy_cancellation;
    mod embassy_executor;
    mod exec_channels;
    mod exec_concurrency;
    mod exec_select;
}

#[path = "saikuro-net"]
mod saikuro_net {
    mod embassy_net_loopback;
}

#[path = "saikuro-random"]
mod saikuro_random {
    mod drbg;
    mod drbg_unseeded;
    mod os_backend;
}

#[path = "saikuro-router"]
mod saikuro_router {
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
mod saikuro_runtime {
    mod config_capacity;
    mod schema_registration;
}

#[path = "saikuro-schema"]
mod saikuro_schema {
    mod capability_enforcement;
    mod registry;
    mod schema_validation;
    mod validator;
}

#[path = "saikuro-storage"]
mod saikuro_storage {
    mod flash;
    mod inmemory;
    mod util;
}

#[path = "saikuro-transport"]
mod saikuro_transport {
    mod embedded_io;
    mod transport_compliance;
    mod transport_framing;
    mod transport_memory_stress;
    mod transport_wasm_host;
}
