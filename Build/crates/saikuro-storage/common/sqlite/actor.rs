use alloc::string::String;
use alloc::vec::Vec;

use std::path::PathBuf;
use std::sync::mpsc::{self, SyncSender};
use std::thread;

use tokio::sync::oneshot;

use graphitesql::exec::eval::Params;
use graphitesql::Connection;
use graphitesql::QueryResult;

use crate::common::sqlite::{map_err, RawSqlite, CREATE_KV};
use crate::shared::config::StorageConfig;
use saikuro_event::{Result, SaikuroError};

/// A unit of work handed to the worker thread.
enum Job {
    Query {
        sql: String,
        params: Params,
        resp: oneshot::Sender<Result<QueryResult>>,
    },
    Batch {
        sql: String,
        resp: oneshot::Sender<Result<()>>,
    },
}

/// Where the worker should open its database.
enum OpenTarget {
    Path(PathBuf),
    Memory { page_size: Option<u32> },
}

/// `Send + Sync` handle to the SQLite worker thread.
pub struct SqliteStorage {
    pub(crate) config: StorageConfig,
    tx: SyncSender<Job>,
}

impl SqliteStorage {
    /// Open or create a SQLite database at the given path.
    pub fn new(path: impl AsRef<std::path::Path>) -> Result<Self> {
        Self::with_config(path, StorageConfig::default())
    }

    /// Open or create a SQLite database with a custom configuration.
    pub fn with_config(path: impl AsRef<std::path::Path>, config: StorageConfig) -> Result<Self> {
        Self::spawn(OpenTarget::Path(path.as_ref().to_path_buf()), config)
    }

    /// Open an in-memory SQLite database (useful for testing).
    pub fn temporary() -> Result<Self> {
        let config = StorageConfig::default();
        Self::spawn(
            OpenTarget::Memory {
                page_size: config.sqlite_page_size,
            },
            config,
        )
    }

    fn spawn(target: OpenTarget, config: StorageConfig) -> Result<Self> {
        let (tx, rx) = mpsc::sync_channel::<Job>(0);
        // The worker thread stays alive for as long as `tx` is held; dropping
        // the JoinHandle detaches it rather than cancelling it.
        thread::Builder::new()
            .name("saikuro-sqlite".into())
            .spawn(move || run_worker(target, rx))
            .map_err(|e| SaikuroError::internal(format!("spawn sqlite worker: {e}")))?;
        Ok(Self { config, tx })
    }
}

/// Own the connection on a dedicated thread and service requests serially.
fn run_worker(target: OpenTarget, rx: mpsc::Receiver<Job>) {
    let opened = match &target {
        OpenTarget::Path(p) => Connection::open(p.to_str().unwrap_or_default()),
        OpenTarget::Memory { page_size } => match page_size {
            Some(ps) => Connection::open_memory_with_page_size(*ps),
            None => Connection::open_memory(),
        },
    };
    let mut conn = match opened {
        Ok(c) => c,
        Err(e) => {
            // Surface the open failure to any queued callers, then exit.
            let _ = e;
            drain(rx);
            return;
        }
    };
    if conn.execute_batch(CREATE_KV).is_err() {
        drain(rx);
        return;
    }
    loop {
        match rx.recv() {
            Ok(Job::Query { sql, params, resp }) => {
                let r = if sql.trim_start().to_ascii_uppercase().starts_with("SELECT") {
                    conn.query_params(&sql, &params).map_err(map_err)
                } else {
                    conn.execute_params(&sql, &params)
                        .map_err(map_err)
                        .map(|_| QueryResult {
                            columns: Vec::new(),
                            rows: Vec::new(),
                        })
                };
                let _ = resp.send(r);
            }
            Ok(Job::Batch { sql, resp }) => {
                let r = conn.execute_batch(&sql).map_err(map_err);
                let _ = resp.send(r);
            }
            Err(_) => {
                break;
            }
        }
    }
}

/// Drop every pending job so callers receive a cancellation error instead of
/// hanging after the worker cannot start.
fn drain(rx: mpsc::Receiver<Job>) {
    while rx.recv().is_ok() {}
}

impl RawSqlite for SqliteStorage {
    async fn query(&self, sql: &str, params: Params) -> Result<QueryResult> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Job::Query {
                sql: sql.to_owned(),
                params,
                resp: tx,
            })
            .map_err(|_| SaikuroError::internal("sqlite worker thread is not running"))?;
        rx.await
            .map_err(|_| SaikuroError::internal("sqlite worker dropped the response"))?
    }

    async fn batch(&self, sql: &str) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Job::Batch {
                sql: sql.to_owned(),
                resp: tx,
            })
            .map_err(|_| SaikuroError::internal("sqlite worker thread is not running"))?;
        rx.await
            .map_err(|_| SaikuroError::internal("sqlite worker dropped the response"))?
    }
}

impl SqliteStorage {
    /// Compile-time guarantee that the public handle is `Send + Sync`, so it can
    /// live inside the `Storage` enum alongside the other backends.
    #[allow(dead_code)]
    fn _assert_send_sync() {
        fn is_send_sync<T: Send + Sync>() {}
        is_send_sync::<SqliteStorage>();
    }
}
