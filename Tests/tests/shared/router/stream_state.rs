//! `StreamStateStore` lifecycle: insert, lookup, conditional removal, and
//! receiver extraction for streams and channels.

use crate::check_test;
use crate::shared_test;
use crate::TestSuite;
use saikuro_core::Arc;
use saikuro_core::{invocation::InvocationId, ResponseEnvelope};
use saikuro_exec::mpsc;
use saikuro_router::stream_state::{ChannelState, StreamState, StreamStateStore};

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "router::stream_store_insert_and_get",
        stream_store_insert_and_get,
    );
    shared_test!(
        suite,
        "router::stream_store_remove_returns_state",
        stream_store_remove_returns_state,
    );
    shared_test!(
        suite,
        "router::stream_store_remove_if_matching_state",
        stream_store_remove_if_matching_state,
    );
    shared_test!(
        suite,
        "router::stream_store_remove_if_mismatch_keeps",
        stream_store_remove_if_mismatch_keeps,
    );
    shared_test!(
        suite,
        "router::stream_store_take_receiver_once",
        stream_store_take_receiver_once,
    );
    shared_test!(
        suite,
        "router::channel_store_insert_get_remove",
        channel_store_insert_get_remove,
    );
    shared_test!(
        suite,
        "router::channel_store_remove_if_matching_state",
        channel_store_remove_if_matching_state,
    );
    shared_test!(
        suite,
        "router::channel_store_take_inbound_outbound",
        channel_store_take_inbound_outbound,
    );
    shared_test!(
        suite,
        "router::store_get_missing_returns_none",
        store_get_missing_returns_none,
    );
}

fn chan_capacity() -> saikuro_exec::ChannelCapacity {
    saikuro_exec::ChannelCapacity::try_from(16).expect("16 is a valid capacity")
}

fn stream_pair() -> (Arc<StreamState>, mpsc::Receiver<ResponseEnvelope>) {
    let (tx, rx) = mpsc::channel::<ResponseEnvelope>(chan_capacity());
    (StreamState::new(tx), rx)
}

fn channel_pair() -> (
    Arc<ChannelState>,
    mpsc::Receiver<ResponseEnvelope>,
    mpsc::Receiver<ResponseEnvelope>,
) {
    let (in_tx, in_rx) = mpsc::channel::<ResponseEnvelope>(chan_capacity());
    let (out_tx, out_rx) = mpsc::channel::<ResponseEnvelope>(chan_capacity());
    (ChannelState::new(in_tx, out_tx), in_rx, out_rx)
}

fn fresh_id() -> InvocationId {
    InvocationId::new().expect("invocation id")
}

fn stream_store_insert_and_get() -> Result<(), &'static str> {
    crate::block_on(async {
        let store = StreamStateStore::new();
        let (state, receiver) = stream_pair();
        let id = fresh_id();
        store.insert_stream(id, state.clone(), receiver).await;

        let fetched = store.get_stream(&id).await;
        check_test!(
            fetched.is_some_and(|got| Arc::ptr_eq(&got, &state)),
            "get_stream must return the stored state"
        );
        Ok(())
    })
}

fn stream_store_remove_returns_state() -> Result<(), &'static str> {
    crate::block_on(async {
        let store = StreamStateStore::new();
        let (state, receiver) = stream_pair();
        let id = fresh_id();
        store.insert_stream(id, state.clone(), receiver).await;

        let removed = store.remove_stream(&id).await;
        check_test!(
            removed.is_some_and(|got| Arc::ptr_eq(&got, &state)),
            "remove_stream must return the removed state"
        );
        check_test!(
            store.get_stream(&id).await.is_none(),
            "removed stream must be gone"
        );
        Ok(())
    })
}

fn stream_store_remove_if_matching_state() -> Result<(), &'static str> {
    crate::block_on(async {
        let store = StreamStateStore::new();
        let (state, receiver) = stream_pair();
        let id = fresh_id();
        store.insert_stream(id, state.clone(), receiver).await;

        check_test!(
            store.remove_stream_if(&id, &state).await,
            "matching state must be removed"
        );
        check_test!(
            store.get_stream(&id).await.is_none(),
            "state gone after remove_if"
        );
        Ok(())
    })
}

fn stream_store_remove_if_mismatch_keeps() -> Result<(), &'static str> {
    crate::block_on(async {
        let store = StreamStateStore::new();
        let (state, receiver) = stream_pair();
        let (other, _other_rx) = stream_pair();
        let id = fresh_id();
        store.insert_stream(id, state.clone(), receiver).await;

        check_test!(
            !store.remove_stream_if(&id, &other).await,
            "a different state must not be removed"
        );
        check_test!(
            store.get_stream(&id).await.is_some(),
            "stream must survive a mismatched remove_if"
        );
        Ok(())
    })
}

fn stream_store_take_receiver_once() -> Result<(), &'static str> {
    crate::block_on(async {
        let store = StreamStateStore::new();
        let (state, receiver) = stream_pair();
        let id = fresh_id();
        store.insert_stream(id, state.clone(), receiver).await;

        let taken = store.take_stream_receiver(&id).await;
        check_test!(taken.is_some(), "receiver must be extractable once");
        check_test!(
            store.take_stream_receiver(&id).await.is_none(),
            "receiver must not be extractable twice"
        );
        Ok(())
    })
}

fn channel_store_insert_get_remove() -> Result<(), &'static str> {
    crate::block_on(async {
        let store = StreamStateStore::new();
        let (state, in_rx, out_rx) = channel_pair();
        let id = fresh_id();
        store.insert_channel(id, state.clone(), in_rx, out_rx).await;

        check_test!(
            store
                .get_channel(&id)
                .await
                .is_some_and(|got| Arc::ptr_eq(&got, &state)),
            "get_channel must return the stored state"
        );
        check_test!(
            store
                .remove_channel(&id)
                .await
                .is_some_and(|got| Arc::ptr_eq(&got, &state)),
            "remove_channel must return the stored state"
        );
        check_test!(
            store.get_channel(&id).await.is_none(),
            "removed channel must be gone"
        );
        Ok(())
    })
}

fn channel_store_remove_if_matching_state() -> Result<(), &'static str> {
    crate::block_on(async {
        let store = StreamStateStore::new();
        let (state, in_rx, out_rx) = channel_pair();
        let (other, _a, _b) = channel_pair();
        let id = fresh_id();
        store.insert_channel(id, state.clone(), in_rx, out_rx).await;

        check_test!(
            !store.remove_channel_if(&id, &other).await,
            "mismatched state must not be removed"
        );
        check_test!(
            store.remove_channel_if(&id, &state).await,
            "matching state must be removed"
        );
        check_test!(
            store.get_channel(&id).await.is_none(),
            "channel gone after remove_if"
        );
        Ok(())
    })
}

fn channel_store_take_inbound_outbound() -> Result<(), &'static str> {
    crate::block_on(async {
        let store = StreamStateStore::new();
        let (state, in_rx, out_rx) = channel_pair();
        let id = fresh_id();
        store.insert_channel(id, state.clone(), in_rx, out_rx).await;

        check_test!(
            store.take_channel_inbound_receiver(&id).await.is_some(),
            "inbound receiver must be extractable"
        );
        check_test!(
            store.take_channel_outbound_receiver(&id).await.is_some(),
            "outbound receiver must be extractable"
        );
        check_test!(
            store.take_channel_inbound_receiver(&id).await.is_none()
                && store.take_channel_outbound_receiver(&id).await.is_none(),
            "receivers must not be extractable twice"
        );
        Ok(())
    })
}

fn store_get_missing_returns_none() -> Result<(), &'static str> {
    crate::block_on(async {
        let store = StreamStateStore::new();
        let id = fresh_id();
        check_test!(
            store.get_stream(&id).await.is_none(),
            "absent stream must yield None"
        );
        check_test!(
            store.get_channel(&id).await.is_none(),
            "absent channel must yield None"
        );
        check_test!(
            store.remove_stream(&id).await.is_none(),
            "absent stream remove must yield None"
        );
        check_test!(
            store.remove_channel(&id).await.is_none(),
            "absent channel remove must yield None"
        );
        Ok(())
    })
}
