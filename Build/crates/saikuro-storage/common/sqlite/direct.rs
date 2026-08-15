use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use spin::Mutex;

use graphitesql::exec::eval::Params;
use graphitesql::Connection;
use graphitesql::QueryResult;

use crate::common::sqlite::{map_err, RawSqlite, CREATE_KV};
use crate::shared::config::StorageConfig;
use saikuro_event::{Result, SaikuroError};

/// Owns the SQLite connection on the current (single) thread.
pub(crate) struct SqliteStorage {
    config: StorageConfig,
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStorage {
    /// Open or create a SQLite database at the given path.
    #[cfg(feature = "std")]
    pub fn new(path: impl AsRef<std::path::Path>) -> Result<Self> {
        Self::with_config(path, StorageConfig::default())
    }

    /// Open or create a SQLite database with a custom configuration.
    #[cfg(feature = "std")]
    pub fn with_config(path: impl AsRef<std::path::Path>, config: StorageConfig) -> Result<Self> {
        let conn = Connection::open(path).map_err(map_err)?;
        Self::from_conn(conn, config)
    }

    /// Open an in-memory SQLite database (wasm / no_std / embedded / testing).
    pub fn temporary() -> Result<Self> {
        let conn = Connection::open_memory().map_err(map_err)?;
        Self::from_conn(conn, StorageConfig::default())
    }

    fn from_conn(conn: Connection, config: StorageConfig) -> Result<Self> {
        conn.execute_batch(CREATE_KV).map_err(map_err)?;
        Ok(Self {
            config,
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

impl RawSqlite for SqliteStorage {
    async fn query(&self, sql: &str, params: Params) -> Result<QueryResult> {
        let conn = self.conn.lock();
        conn.query_params(sql, &params).map_err(map_err)
    }

    async fn batch(&self, sql: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch(sql).map_err(map_err)
    }
}
