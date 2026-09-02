#[cfg(not(target_has_atomic = "ptr"))]
use portable_atomic_util::Arc;
#[cfg(target_has_atomic = "ptr")]
use saikuro_core::Arc;

use spin::Mutex;

use graphitesql::exec::eval::Params;
use graphitesql::Connection;
use graphitesql::QueryResult;

use alloc::vec::Vec;

use crate::common::sqlite::{map_err, RawSqlite, CREATE_KV};
use crate::shared::config::StorageConfig;
use saikuro_event::Result;

/// Owns the SQLite connection on the current (single) thread.
pub struct SqliteStorage {
    pub(crate) config: StorageConfig,
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStorage {
    /// Open an in-memory SQLite database (wasm / no_std / embedded / testing).
    pub fn temporary() -> Result<Self> {
        let conn = Connection::open_memory().map_err(map_err)?;
        Self::from_conn(conn, StorageConfig::default())
    }

    fn from_conn(mut conn: Connection, config: StorageConfig) -> Result<Self> {
        conn.execute_batch(CREATE_KV).map_err(map_err)?;
        Ok(Self {
            config,
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

impl RawSqlite for SqliteStorage {
    async fn query(&self, sql: &str, params: Params) -> Result<QueryResult> {
        let mut conn = self.conn.lock();
        if sql.trim_start().to_ascii_uppercase().starts_with("SELECT") {
            conn.query_params(sql, &params).map_err(map_err)
        } else {
            conn.execute_params(sql, &params).map_err(map_err)?;
            Ok(QueryResult {
                columns: Vec::new(),
                rows: Vec::new(),
            })
        }
    }

    async fn batch(&self, sql: &str) -> Result<()> {
        let mut conn = self.conn.lock();
        conn.execute_batch(sql).map_err(map_err)
    }
}
