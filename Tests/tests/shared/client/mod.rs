use crate::shared_test;
use crate::TestSuite;
use core::time::Duration;
use saikuro_client::{Client, ClientOptions, Error, MemoryAdapterTransport, Provider, Value};
use saikuro_core::Arc;
use saikuro_event::LogSink;

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "client::call_roundtrip_over_memory",
        call_roundtrip_over_memory,
    );
    shared_test!(
        suite,
        "client::cast_delivers_to_provider_handler",
        cast_delivers_to_provider_handler,
    );
    shared_test!(
        suite,
        "client::is_connected_true_after_from_transport",
        is_connected_true_after_from_transport,
    );
    shared_test!(
        suite,
        "client::default_timeout_aborts_orphaned_call",
        default_timeout_aborts_orphaned_call,
    );
    shared_test!(
        suite,
        "client::channel_abort_and_close_send_frames",
        channel_abort_and_close_send_frames,
    );
    shared_test!(
        suite,
        "client::provider_namespace_and_log_sink_accessors",
        provider_namespace_and_log_sink_accessors,
    );
}

/// Boot a client and a provider on a memory transport pair, hand the client to
/// `client_fut`, then wait for the provider to exit once the future closes.
async fn with_provider<F, Fut>(client_fut: F) -> Result<(), &'static str>
where
    F: FnOnce(Client) -> Fut,
    Fut: core::future::Future<Output = Result<(), &'static str>>,
{
    let (client_side, provider_side) = MemoryAdapterTransport::pair();
    let client =
        Client::from_transport(crate::Box::new(client_side), Some(ClientOptions::default()))
            .map_err(|_| "client from transport")?;

    let mut provider = Provider::new("echo");
    provider.register("reverse", |args: saikuro_client::HandlerArgs| async move {
        let input = args
            .first()
            .and_then(|v| v.as_str())
            .ok_or(Error::InvalidArguments {
                target: crate::String::from("echo.reverse"),
                reason: crate::String::from("missing input"),
            })?;
        let reversed: crate::String = input.chars().rev().collect();
        Ok(Value::String(reversed))
    });

    let serve =
        saikuro_exec::spawn(async move { provider.serve_on(crate::Box::new(provider_side)).await });

    let result = client_fut(client).await;

    let serve_outcome = serve.await.map_err(|_| "provider task join")?;
    serve_outcome.map_err(|_| "provider serve")?;
    result
}

fn call_roundtrip_over_memory() -> Result<(), &'static str> {
    crate::block_on(async {
        with_provider(|client| async move {
            let out = client
                .call(
                    "echo.reverse",
                    crate::vec![Value::String("stressed".into())],
                )
                .await
                .map_err(|_| "client call")?;
            crate::check_test!(
                out.as_str() == Some("desserts"),
                "the provider must reverse the argument"
            );
            client.close().await.map_err(|_| "client close")?;
            Ok(())
        })
        .await
    })
}

fn cast_delivers_to_provider_handler() -> Result<(), &'static str> {
    crate::block_on(async {
        let (client_side, provider_side) = MemoryAdapterTransport::pair();
        let client = Client::from_transport(crate::Box::new(client_side), None)
            .map_err(|_| "client from transport")?;

        let hits = Arc::new(spin::Mutex::new(0usize));
        let mut provider = Provider::new("counter");
        let hits_clone = hits.clone();
        provider.register("ping", move |_args| {
            let hits_clone = hits_clone.clone();
            async move {
                *hits_clone.lock() += 1;
                Ok(Value::Null)
            }
        });

        let serve =
            saikuro_exec::spawn(
                async move { provider.serve_on(crate::Box::new(provider_side)).await },
            );

        client
            .cast("counter.ping", crate::vec![])
            .await
            .map_err(|_| "client cast")?;

        let count = client
            .call("counter.ping", crate::vec![])
            .await
            .map_err(|_| "client call")?;
        crate::check_test!(count.is_null(), "ping must return null on success");
        crate::check_test!(
            *hits.lock() >= 2,
            "cast and call must both reach the handler"
        );

        client.close().await.map_err(|_| "client close")?;
        let serve_outcome = serve.await.map_err(|_| "provider task join")?;
        serve_outcome.map_err(|_| "provider serve")?;
        Ok(())
    })
}

fn is_connected_true_after_from_transport() -> Result<(), &'static str> {
    crate::block_on(async {
        let (a, _b) = MemoryAdapterTransport::pair();
        let client = Client::from_transport(crate::Box::new(a), None)
            .map_err(|_| "client from transport")?;
        crate::check_test!(
            client.is_connected(),
            "a freshly-built client must report connected"
        );
        client.close().await.map_err(|_| "client close")?;
        Ok(())
    })
}

fn default_timeout_aborts_orphaned_call() -> Result<(), &'static str> {
    crate::block_on(async {
        let (a, _b) = MemoryAdapterTransport::pair();
        let options = ClientOptions {
            default_timeout: Some(Duration::from_millis(250)),
        };
        let client = Client::from_transport(crate::Box::new(a), Some(options))
            .map_err(|_| "client from transport")?;
        let out = client.call("ghost.no_reply", crate::vec![]).await;
        crate::check_test!(
            out.is_err(),
            "a call to an unserved namespace must not hang"
        );
        client.close().await.map_err(|_| "client close")?;
        Ok(())
    })
}

fn channel_abort_and_close_send_frames() -> Result<(), &'static str> {
    crate::block_on(async {
        let (a, _b) = MemoryAdapterTransport::pair();
        let client = Client::from_transport(crate::Box::new(a), None)
            .map_err(|_| "client from transport")?;

        let channel = client
            .channel("echo.duplex", crate::vec![])
            .await
            .map_err(|_| "open channel")?;
        channel
            .send(Value::String("hello".into()))
            .await
            .map_err(|_| "channel send")?;
        channel.abort().await.map_err(|_| "channel abort")?;

        let channel2 = client
            .channel("echo.duplex", crate::vec![])
            .await
            .map_err(|_| "open second channel")?;
        channel2.close().await.map_err(|_| "channel close")?;

        client.close().await.map_err(|_| "client close")?;
        Ok(())
    })
}

fn provider_namespace_and_log_sink_accessors() -> Result<(), &'static str> {
    let provider = Provider::new("greet");
    crate::check_test!(
        provider.namespace() == "greet",
        "a provider must report its namespace"
    );

    let sink: Arc<dyn LogSink> =
        Arc::from(crate::Box::new(saikuro_event::NullSink) as crate::Box<dyn LogSink>);
    let with_sink = provider.with_log_sink(sink);
    crate::check_test!(
        with_sink.namespace() == "greet",
        "with_log_sink must preserve the namespace"
    );
    Ok(())
}
