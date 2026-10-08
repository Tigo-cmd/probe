//! scanstore: the local scan history and the queue the sync service will drain.
//!
//! Every scan the app makes or opens is kept in a SQLite file on the machine
//! running the app. Nothing leaves it on its own: a scan starts `local` and
//! only becomes `pending` when the user asks for it to be shared. The sync
//! service (not built yet) takes pending scans oldest first and reports back
//! with [`Store::mark_synced`] or [`Store::mark_failed`].
//!
//! Rules this crate keeps:
//! - A scan is stored exactly as captured and never modified. It is keyed by
//!   the SHA-256 of its JSON, so storing the same scan twice is a no-op and a
//!   sync retry can never create a duplicate.
//! - Verdicts are a cache. They are re-graded when the store is opened by a
//!   different grader version, so the history never shows an outdated grade.
//! - A database written by a newer schema is refused, not rewritten.

use std::fmt;
use std::path::Path;

use hwprobe::grade::{self, Verdict, GRADER_VERSION};
use hwprobe::Scan;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Bumped with every migration; stored in SQLite's `user_version`.
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug)]
pub enum Error {
    Sqlite(rusqlite::Error),
    Json(serde_json::Error),
    NotFound(i64),
    /// The file was written by a newer version of probe.
    NewerSchema(u32),
    /// The requested sync transition is not allowed from the current state.
    InvalidTransition {
        id: i64,
        from: SyncState,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Sqlite(e) => write!(f, "scan history: {e}"),
            Error::Json(e) => write!(f, "stored scan is unreadable: {e}"),
            Error::NotFound(id) => write!(f, "no stored scan with id {id}"),
            Error::NewerSchema(v) => write!(
                f,
                "the scan history was written by a newer version of probe (schema {v}, this build reads {SCHEMA_VERSION}); update probe to open it"
            ),
            Error::InvalidTransition { id, from } => {
                write!(f, "scan {id} is {from:?} and cannot change that way")
            }
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Sqlite(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Where a stored scan came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// Scanned by this app on the machine it ran on.
    Live,
    /// Opened from a saved file. It may describe a different machine.
    Imported,
}

/// A scan's place in the sync queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncState {
    /// Kept on this computer only. The default: nothing is shared unasked.
    Local,
    /// The user asked to share it; waiting for the sync service.
    Pending,
    Synced,
    /// The last attempt failed; it stays queued and is retried.
    Failed,
}

impl SyncState {
    fn as_str(self) -> &'static str {
        match self {
            SyncState::Local => "local",
            SyncState::Pending => "pending",
            SyncState::Synced => "synced",
            SyncState::Failed => "failed",
        }
    }

    fn parse(s: &str) -> SyncState {
        match s {
            "pending" => SyncState::Pending,
            "synced" => SyncState::Synced,
            "failed" => SyncState::Failed,
            _ => SyncState::Local,
        }
    }
}

/// One row of the history, without the scan body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Summary {
    pub id: i64,
    /// SHA-256 of the scan's JSON: the stable identity used for sync.
    pub content_id: String,
    pub started_at: u64,
    pub stored_at: u64,
    pub origin: Origin,
    pub os: String,
    pub machine: Option<String>,
    pub serial: Option<String>,
    pub headline: Verdict,
    pub grader_version: String,
    pub label: Option<String>,
    pub sync: SyncState,
    pub sync_attempts: u32,
    pub last_error: Option<String>,
    pub synced_at: Option<u64>,
}

/// The result of storing a scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Stored {
    pub id: i64,
    /// False when this exact scan was already in the history.
    pub new: bool,
}

pub struct Store {
    conn: Connection,
}

const SUMMARY_COLUMNS: &str =
    "id, content_id, started_at, stored_at, origin, os, machine, serial, \
     headline, grader_version, label, sync_state, sync_attempts, last_error, synced_at";

impl Store {
    /// Open or create the history at `path`, migrating and re-grading as needed.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_connection(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(conn: Connection) -> Result<Self> {
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let mut store = Store { conn };
        store.migrate()?;
        store.refresh_grades()?;
        Ok(store)
    }

    fn migrate(&mut self) -> Result<()> {
        let version: u32 = self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(Error::NewerSchema(version));
        }
        if version < 1 {
            let tx = self.conn.transaction()?;
            tx.execute_batch(
                "CREATE TABLE scans (
                    id              INTEGER PRIMARY KEY,
                    content_id      TEXT NOT NULL UNIQUE,
                    scan_json       TEXT NOT NULL,
                    started_at      INTEGER NOT NULL,
                    stored_at       INTEGER NOT NULL,
                    origin          TEXT NOT NULL CHECK (origin IN ('live', 'imported')),
                    os              TEXT NOT NULL,
                    machine         TEXT,
                    serial          TEXT,
                    board_serial    TEXT,
                    uuid            TEXT,
                    headline        TEXT NOT NULL,
                    grader_version  TEXT NOT NULL,
                    label           TEXT,
                    sync_state      TEXT NOT NULL DEFAULT 'local'
                                    CHECK (sync_state IN ('local', 'pending', 'synced', 'failed')),
                    sync_attempts   INTEGER NOT NULL DEFAULT 0,
                    last_attempt_at INTEGER,
                    last_error      TEXT,
                    synced_at       INTEGER
                );
                CREATE INDEX scans_by_time ON scans (started_at DESC, id DESC);
                CREATE INDEX scans_by_serial ON scans (serial);
                CREATE INDEX scans_by_board_serial ON scans (board_serial);
                CREATE INDEX scans_by_uuid ON scans (uuid);
                CREATE INDEX scans_queue ON scans (sync_state, sync_attempts, stored_at)
                    WHERE sync_state IN ('pending', 'failed');
                PRAGMA user_version = 1;",
            )?;
            tx.commit()?;
        }
        Ok(())
    }

    /// Re-grade every row cached by a different grader version.
    fn refresh_grades(&mut self) -> Result<()> {
        let stale: Vec<(i64, String)> = {
            let mut q = self
                .conn
                .prepare("SELECT id, scan_json FROM scans WHERE grader_version != ?1")?;
            let rows = q.query_map([GRADER_VERSION], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        if stale.is_empty() {
            return Ok(());
        }
        let tx = self.conn.transaction()?;
        for (id, json) in stale {
            let scan: Scan = serde_json::from_str(&json)?;
            let g = grade::grade(&scan);
            tx.execute(
                "UPDATE scans SET headline = ?1, grader_version = ?2 WHERE id = ?3",
                params![verdict_str(g.headline), GRADER_VERSION, id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Store a scan. Storing the same scan again returns the existing row.
    pub fn insert(&mut self, scan: &Scan, origin: Origin) -> Result<Stored> {
        let json = serde_json::to_string(scan)?;
        let content_id = content_id(&json);
        if let Some(id) = self.id_of(&content_id)? {
            return Ok(Stored { id, new: false });
        }
        let g = grade::grade(scan);
        let id = &scan.identity;
        let machine = [id.vendor.get(), id.model.get()]
            .into_iter()
            .flatten()
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        self.conn.execute(
            "INSERT INTO scans (content_id, scan_json, started_at, stored_at, origin, os, machine,
                                serial, board_serial, uuid, headline, grader_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                content_id,
                json,
                scan.started_at as i64,
                now() as i64,
                origin_str(origin),
                scan.os,
                (!machine.is_empty()).then_some(machine),
                id.serial.get(),
                id.board_serial.get(),
                id.uuid.get().map(|u| u.to_ascii_lowercase()),
                verdict_str(g.headline),
                GRADER_VERSION,
            ],
        )?;
        Ok(Stored {
            id: self.conn.last_insert_rowid(),
            new: true,
        })
    }

    fn id_of(&self, content_id: &str) -> Result<Option<i64>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id FROM scans WHERE content_id = ?1",
                [content_id],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// The whole history, newest scan first.
    pub fn list(&self) -> Result<Vec<Summary>> {
        self.summaries(
            &format!("SELECT {SUMMARY_COLUMNS} FROM scans ORDER BY started_at DESC, id DESC"),
            [],
        )
    }

    pub fn summary(&self, id: i64) -> Result<Summary> {
        self.summaries(
            &format!("SELECT {SUMMARY_COLUMNS} FROM scans WHERE id = ?1"),
            [id],
        )?
        .pop()
        .ok_or(Error::NotFound(id))
    }

    pub fn get(&self, id: i64) -> Result<(Summary, Scan)> {
        let summary = self.summary(id)?;
        let json: String =
            self.conn
                .query_row("SELECT scan_json FROM scans WHERE id = ?1", [id], |r| {
                    r.get(0)
                })?;
        Ok((summary, serde_json::from_str(&json)?))
    }

    pub fn delete(&mut self, id: i64) -> Result<()> {
        match self.conn.execute("DELETE FROM scans WHERE id = ?1", [id])? {
            0 => Err(Error::NotFound(id)),
            _ => Ok(()),
        }
    }

    /// A free-text note, e.g. the listing or seller. `None` or blank clears it.
    pub fn set_label(&mut self, id: i64, label: Option<&str>) -> Result<()> {
        let label = label.map(str::trim).filter(|l| !l.is_empty());
        match self.conn.execute(
            "UPDATE scans SET label = ?1 WHERE id = ?2",
            params![label, id],
        )? {
            0 => Err(Error::NotFound(id)),
            _ => Ok(()),
        }
    }

    /// Other stored scans that share an identifier with `scan` (SMBIOS UUID,
    /// system serial or board serial), newest first. These are the candidates
    /// for a swapped-parts comparison. Identifiers are claims, so a match
    /// means "says it is the same machine", not proof.
    pub fn same_machine(&self, scan: &Scan) -> Result<Vec<Summary>> {
        let id = &scan.identity;
        let uuid = id.uuid.get().map(|u| u.to_ascii_lowercase());
        let serial = id.serial.get();
        let board = id.board_serial.get();
        if uuid.is_none() && serial.is_none() && board.is_none() {
            return Ok(Vec::new());
        }
        let own = content_id(&serde_json::to_string(scan)?);
        self.summaries(
            &format!(
                "SELECT {SUMMARY_COLUMNS} FROM scans
                 WHERE content_id != ?1
                   AND ((?2 IS NOT NULL AND uuid = ?2)
                     OR (?3 IS NOT NULL AND serial = ?3)
                     OR (?4 IS NOT NULL AND board_serial = ?4))
                 ORDER BY started_at DESC, id DESC"
            ),
            params![own, uuid, serial, board],
        )
    }

    // ------------------------------------------------------------ sync queue

    /// Ask for a scan to be shared. Allowed from `local` and `failed`.
    pub fn queue_for_sync(&mut self, id: i64) -> Result<()> {
        self.transition(
            id,
            &[SyncState::Local, SyncState::Failed],
            SyncState::Pending,
        )
    }

    /// Take a scan back out of the queue before it is sent.
    pub fn withdraw(&mut self, id: i64) -> Result<()> {
        self.transition(
            id,
            &[SyncState::Pending, SyncState::Failed],
            SyncState::Local,
        )
    }

    /// Up to `limit` queued scans with their JSON: pending before failed,
    /// fewest attempts first, oldest first. The sync service's work list.
    pub fn next_to_sync(&self, limit: usize) -> Result<Vec<(Summary, String)>> {
        let mut q = self.conn.prepare(&format!(
            "SELECT {SUMMARY_COLUMNS}, scan_json FROM scans
             WHERE sync_state IN ('pending', 'failed')
             ORDER BY sync_state = 'failed', sync_attempts, stored_at, id
             LIMIT ?1"
        ))?;
        let rows = q.query_map([limit as i64], |r| Ok((summary_from(r)?, r.get(15)?)))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn mark_synced(&mut self, id: i64) -> Result<()> {
        self.require(id, &[SyncState::Pending, SyncState::Failed])?;
        self.conn.execute(
            "UPDATE scans SET sync_state = 'synced', synced_at = ?1, last_attempt_at = ?1,
                    sync_attempts = sync_attempts + 1, last_error = NULL
             WHERE id = ?2",
            params![now() as i64, id],
        )?;
        Ok(())
    }

    pub fn mark_failed(&mut self, id: i64, error: &str) -> Result<()> {
        self.require(id, &[SyncState::Pending, SyncState::Failed])?;
        self.conn.execute(
            "UPDATE scans SET sync_state = 'failed', last_attempt_at = ?1,
                    sync_attempts = sync_attempts + 1, last_error = ?2
             WHERE id = ?3",
            params![now() as i64, error, id],
        )?;
        Ok(())
    }

    /// Counts per sync state, for a status line.
    pub fn queue_counts(&self) -> Result<QueueCounts> {
        let mut c = QueueCounts::default();
        let mut q = self
            .conn
            .prepare("SELECT sync_state, COUNT(*) FROM scans GROUP BY sync_state")?;
        let rows = q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        for row in rows {
            let (state, n) = row?;
            let n = n as u32;
            match SyncState::parse(&state) {
                SyncState::Local => c.local = n,
                SyncState::Pending => c.pending = n,
                SyncState::Synced => c.synced = n,
                SyncState::Failed => c.failed = n,
            }
        }
        Ok(c)
    }

    fn require(&self, id: i64, allowed: &[SyncState]) -> Result<()> {
        let from = self.summary(id)?.sync;
        if allowed.contains(&from) {
            Ok(())
        } else {
            Err(Error::InvalidTransition { id, from })
        }
    }

    fn transition(&mut self, id: i64, allowed: &[SyncState], to: SyncState) -> Result<()> {
        self.require(id, allowed)?;
        self.conn.execute(
            "UPDATE scans SET sync_state = ?1 WHERE id = ?2",
            params![to.as_str(), id],
        )?;
        Ok(())
    }

    fn summaries<P: rusqlite::Params>(&self, sql: &str, params: P) -> Result<Vec<Summary>> {
        let mut q = self.conn.prepare(sql)?;
        let rows = q.query_map(params, summary_from)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct QueueCounts {
    pub local: u32,
    pub pending: u32,
    pub synced: u32,
    pub failed: u32,
}

fn summary_from(r: &Row<'_>) -> rusqlite::Result<Summary> {
    Ok(Summary {
        id: r.get(0)?,
        content_id: r.get(1)?,
        started_at: r.get::<_, i64>(2)?.max(0) as u64,
        stored_at: r.get::<_, i64>(3)?.max(0) as u64,
        origin: if r.get::<_, String>(4)? == "live" {
            Origin::Live
        } else {
            Origin::Imported
        },
        os: r.get(5)?,
        machine: r.get(6)?,
        serial: r.get(7)?,
        headline: parse_verdict(&r.get::<_, String>(8)?),
        grader_version: r.get(9)?,
        label: r.get(10)?,
        sync: SyncState::parse(&r.get::<_, String>(11)?),
        sync_attempts: r.get::<_, i64>(12)?.max(0) as u32,
        last_error: r.get(13)?,
        synced_at: r.get::<_, Option<i64>>(14)?.map(|t| t.max(0) as u64),
    })
}

/// SHA-256 of the scan's JSON, in hex.
pub fn content_id(json: &str) -> String {
    Sha256::digest(json.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn verdict_str(v: Verdict) -> &'static str {
    match v {
        Verdict::Green => "green",
        Verdict::Amber => "amber",
        Verdict::Red => "red",
        Verdict::NotGraded => "not_graded",
    }
}

fn parse_verdict(s: &str) -> Verdict {
    match s {
        "green" => Verdict::Green,
        "amber" => Verdict::Amber,
        "red" => Verdict::Red,
        // Anything unrecognised is shown as missing evidence, never as a pass.
        _ => Verdict::NotGraded,
    }
}

fn origin_str(o: Origin) -> &'static str {
    match o {
        Origin::Live => "live",
        Origin::Imported => "imported",
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hwprobe::model::{Battery, Capacity, CapacityUnit, Field};

    fn scan(serial: &str, started_at: u64) -> Scan {
        let mut s = Scan {
            os: "windows".into(),
            started_at,
            ..Scan::default()
        };
        s.identity.vendor = Field::claimed(Some("LENOVO".into()), "t");
        s.identity.model = Field::claimed(Some("20HRCTO1WW".into()), "t");
        s.identity.serial = Field::claimed(Some(serial.into()), "t");
        s
    }

    fn with_battery(mut s: Scan, full: u64) -> Scan {
        let cap = |v| {
            Field::measured(
                Some(Capacity {
                    value: v,
                    unit: CapacityUnit::MilliwattHours,
                }),
                "t",
            )
        };
        s.batteries.push(Battery {
            name: "BAT0".into(),
            design_capacity: cap(100),
            full_charge_capacity: cap(full),
            ..Battery::default()
        });
        s
    }

    #[test]
    fn stores_lists_and_reads_back_unchanged() {
        let mut st = Store::open_in_memory().unwrap();
        let a = st.insert(&scan("PF0A", 100), Origin::Live).unwrap();
        let b = st.insert(&scan("PF0B", 200), Origin::Imported).unwrap();
        assert!(a.new && b.new);

        let list = st.list().unwrap();
        assert_eq!(list.iter().map(|s| s.id).collect::<Vec<_>>(), [b.id, a.id]);
        assert_eq!(list[1].machine.as_deref(), Some("LENOVO 20HRCTO1WW"));
        assert_eq!(list[1].sync, SyncState::Local, "nothing is shared unasked");
        assert_eq!(list[0].origin, Origin::Imported);

        let (_, back) = st.get(a.id).unwrap();
        assert_eq!(back, scan("PF0A", 100));
    }

    #[test]
    fn the_same_scan_is_stored_once() {
        let mut st = Store::open_in_memory().unwrap();
        let first = st.insert(&scan("PF0A", 100), Origin::Live).unwrap();
        let again = st.insert(&scan("PF0A", 100), Origin::Imported).unwrap();
        assert_eq!(
            again,
            Stored {
                id: first.id,
                new: false
            }
        );
        assert_eq!(st.list().unwrap().len(), 1);
    }

    #[test]
    fn caches_the_grade_and_never_invents_a_pass() {
        let mut st = Store::open_in_memory().unwrap();
        let red = st
            .insert(&with_battery(scan("A", 1), 60), Origin::Live)
            .unwrap();
        let empty = st.insert(&scan("B", 2), Origin::Live).unwrap();
        assert_eq!(st.summary(red.id).unwrap().headline, Verdict::Red);
        assert_eq!(st.summary(empty.id).unwrap().headline, Verdict::NotGraded);
        assert_eq!(parse_verdict("something_new"), Verdict::NotGraded);
    }

    #[test]
    fn regrades_rows_from_another_grader_version() {
        let dir = std::env::temp_dir().join(format!("scanstore-regrade-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("scans.sqlite3");
        let _ = std::fs::remove_file(&path);
        let id = {
            let mut st = Store::open(&path).unwrap();
            let id = st
                .insert(&with_battery(scan("A", 1), 60), Origin::Live)
                .unwrap()
                .id;
            // Simulate a row cached by an older grader with a different verdict.
            st.conn
                .execute(
                    "UPDATE scans SET headline = 'green', grader_version = '0.0.1' WHERE id = ?1",
                    [id],
                )
                .unwrap();
            id
        };
        let st = Store::open(&path).unwrap();
        let s = st.summary(id).unwrap();
        assert_eq!(s.grader_version, GRADER_VERSION);
        assert_eq!(s.headline, Verdict::Red);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn refuses_a_database_from_a_newer_schema() {
        let dir = std::env::temp_dir().join(format!("scanstore-newer-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("scans.sqlite3");
        let _ = std::fs::remove_file(&path);
        Connection::open(&path)
            .unwrap()
            .execute_batch("PRAGMA user_version = 99;")
            .unwrap();
        assert!(matches!(Store::open(&path), Err(Error::NewerSchema(99))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn finds_earlier_scans_of_the_same_machine() {
        let mut st = Store::open_in_memory().unwrap();
        let old = st.insert(&scan("PF0A", 100), Origin::Live).unwrap();
        st.insert(&scan("OTHER", 150), Origin::Live).unwrap();
        let mut by_uuid = scan("ZZZ", 120);
        by_uuid.identity.uuid = Field::claimed(Some("4C4C-0001".into()), "t");
        let uuid_row = st.insert(&by_uuid, Origin::Imported).unwrap();

        let mut now = scan("PF0A", 200);
        now.identity.uuid = Field::claimed(Some("4c4c-0001".into()), "t");
        st.insert(&now, Origin::Live).unwrap();

        let ids: Vec<i64> = st
            .same_machine(&now)
            .unwrap()
            .iter()
            .map(|s| s.id)
            .collect();
        assert_eq!(
            ids,
            [uuid_row.id, old.id],
            "matches serial or UUID, excludes itself"
        );

        let mut anonymous = scan("x", 1);
        anonymous.identity.serial = Field::default();
        assert!(st.same_machine(&anonymous).unwrap().is_empty());
    }

    #[test]
    fn sync_queue_transitions() {
        let mut st = Store::open_in_memory().unwrap();
        let a = st.insert(&scan("A", 1), Origin::Live).unwrap().id;
        let b = st.insert(&scan("B", 2), Origin::Live).unwrap().id;
        let c = st.insert(&scan("C", 3), Origin::Live).unwrap().id;

        assert!(
            st.next_to_sync(10).unwrap().is_empty(),
            "local scans are never sent"
        );
        assert!(matches!(
            st.mark_synced(a),
            Err(Error::InvalidTransition { .. })
        ));

        st.queue_for_sync(a).unwrap();
        st.queue_for_sync(b).unwrap();
        st.queue_for_sync(c).unwrap();
        st.mark_failed(a, "network unreachable").unwrap();
        st.withdraw(c).unwrap();

        let next: Vec<i64> = st
            .next_to_sync(10)
            .unwrap()
            .iter()
            .map(|(s, _)| s.id)
            .collect();
        assert_eq!(
            next,
            [b, a],
            "pending before failed; withdrawn scans drop out"
        );
        let (_, json) = &st.next_to_sync(1).unwrap()[0];
        assert_eq!(content_id(json), st.summary(b).unwrap().content_id);

        st.mark_synced(b).unwrap();
        st.mark_synced(a).unwrap();
        let a_row = st.summary(a).unwrap();
        assert_eq!(a_row.sync, SyncState::Synced);
        assert_eq!(a_row.sync_attempts, 2);
        assert_eq!(a_row.last_error, None);
        assert!(matches!(
            st.queue_for_sync(a),
            Err(Error::InvalidTransition { .. })
        ));
        assert_eq!(
            st.queue_counts().unwrap(),
            QueueCounts {
                local: 1,
                pending: 0,
                synced: 2,
                failed: 0
            }
        );
    }

    #[test]
    fn labels_and_deletes() {
        let mut st = Store::open_in_memory().unwrap();
        let id = st.insert(&scan("A", 1), Origin::Live).unwrap().id;
        st.set_label(id, Some("  eBay listing 1234  ")).unwrap();
        assert_eq!(
            st.summary(id).unwrap().label.as_deref(),
            Some("eBay listing 1234")
        );
        st.set_label(id, Some("   ")).unwrap();
        assert_eq!(st.summary(id).unwrap().label, None);
        st.delete(id).unwrap();
        assert!(matches!(st.delete(id), Err(Error::NotFound(_))));
        assert!(matches!(st.get(id), Err(Error::NotFound(_))));
    }
}
