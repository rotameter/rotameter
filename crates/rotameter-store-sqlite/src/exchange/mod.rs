#![forbid(unsafe_code)]
//! Chunked export and idempotent import entrypoints on [`SqliteStore`] for portable exchange.

mod control;
mod read;
mod read_sessions;
mod write;

use rotameter_core::error::CoreError;
use rotameter_core::storage::{
    ControlPlaneSnapshot, IngestEvent, RollupDailyRow, RollupHourlyRow, RollupPathDailyRow,
    SessionUpsert,
};

use crate::store::SqliteStore;

impl SqliteStore {
    /// Exports all organizations, sites, API keys, and active rotating salts.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on query or decode failure.
    pub fn export_control_plane_sync(&self) -> Result<ControlPlaneSnapshot, CoreError> {
        self.with_reader(control::export_control_plane_rows)
    }

    /// Idempotently restores control-plane entities into this `SQLite` database.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on transaction failure.
    pub fn import_control_plane_sync(&self, snap: &ControlPlaneSnapshot) -> Result<(), CoreError> {
        self.with_writer(|state| {
            state.interner.clear();
            control::import_control_plane_rows(&mut state.conn, snap)
        })
    }

    /// Reads an ordered slice of raw `events` rows (`[offset, offset + limit)`).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on query or decode failure.
    pub fn read_events_chunk_sync(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<IngestEvent>, CoreError> {
        self.with_reader(|conn| read::read_events_chunk(conn, offset, limit))
    }

    /// Reads an ordered slice of `sessions` rows (`[offset, offset + limit)`).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on query or decode failure.
    pub fn read_sessions_chunk_sync(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SessionUpsert>, CoreError> {
        self.with_reader(|conn| read_sessions::read_sessions_chunk(conn, offset, limit))
    }

    /// Reads an ordered slice of `rollups_hourly` rows (`[offset, offset + limit)`).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on query or decode failure.
    pub fn read_rollups_hourly_chunk_sync(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<RollupHourlyRow>, CoreError> {
        self.with_reader(|conn| read::read_rollups_hourly_chunk(conn, offset, limit))
    }

    /// Reads an ordered slice of `rollups_daily` rows (`[offset, offset + limit)`).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on query or decode failure.
    pub fn read_rollups_daily_chunk_sync(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<RollupDailyRow>, CoreError> {
        self.with_reader(|conn| read::read_rollups_daily_chunk(conn, offset, limit))
    }

    /// Reads an ordered slice of `rollups_path_daily` rows (`[offset, offset + limit)`).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on query or decode failure.
    pub fn read_rollups_path_daily_chunk_sync(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<RollupPathDailyRow>, CoreError> {
        self.with_reader(|conn| read::read_rollups_path_daily_chunk(conn, offset, limit))
    }

    /// Imports an ordered slice of `events` inside a single immediate transaction.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on transaction failure.
    pub fn import_events_chunk_sync(&self, rows: &[IngestEvent]) -> Result<(), CoreError> {
        self.with_writer(|state| write::import_events_chunk(state, rows))
    }

    /// Imports an ordered slice of `sessions` (and `seen_visitors_daily` markers) idempotently.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on transaction failure.
    pub fn import_sessions_chunk_sync(&self, rows: &[SessionUpsert]) -> Result<(), CoreError> {
        self.with_writer(|state| write::import_sessions_chunk(state, rows))
    }

    /// Imports `rollups_hourly` rows idempotently (`INSERT OR REPLACE`).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on transaction failure.
    pub fn import_rollups_hourly_chunk_sync(
        &self,
        rows: &[RollupHourlyRow],
    ) -> Result<(), CoreError> {
        self.with_writer(|state| write::import_rollups_hourly_chunk(state, rows))
    }

    /// Imports `rollups_daily` rows idempotently (`INSERT OR REPLACE`).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on transaction failure.
    pub fn import_rollups_daily_chunk_sync(
        &self,
        rows: &[RollupDailyRow],
    ) -> Result<(), CoreError> {
        self.with_writer(|state| write::import_rollups_daily_chunk(state, rows))
    }

    /// Imports `rollups_path_daily` rows idempotently (`INSERT OR REPLACE`).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on transaction failure.
    pub fn import_rollups_path_daily_chunk_sync(
        &self,
        rows: &[RollupPathDailyRow],
    ) -> Result<(), CoreError> {
        self.with_writer(|state| write::import_rollups_path_daily_chunk(state, rows))
    }
}
