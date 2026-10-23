//! probe-sync: the team server that probe's scan history syncs to.
//!
//! A shop or team runs one server. Each computer running probe gets its own
//! access token (`probe-sync token add NAME`), and scans the user chose to
//! share are uploaded to it. In return any device can ask "has the team seen
//! this machine before?" and get the earlier scans that claim the same
//! SMBIOS UUID, system serial or board serial.
//!
//! Rules this crate keeps, matching the local store:
//! - A scan is stored exactly as uploaded and is keyed by the SHA-256 of its
//!   JSON. The server checks the hash, so a retry is a no-op and a truncated
//!   or altered upload is refused.
//! - Verdicts are never stored: they are computed by the grader this server
//!   ships with whenever a scan is listed, so they cannot go stale.
//! - Only the device that uploaded a scan can delete it.
//! - Tokens are stored as SHA-256 hashes. A token is shown once, when made.
//!
//! The server speaks plain HTTP. Put it behind a TLS reverse proxy for
//! anything but localhost; the client refuses plain HTTP to other hosts.

use std::path::Path;
use std::sync::{Arc, Mutex};

use axum::extract::{DefaultBodyLimit, Path as UrlPath, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, put};
use axum::{Json, Router};
use hwprobe::grade::{self, Verdict};
use hwprobe::model::SCAN_SCHEMA_VERSION;
use hwprobe::Scan;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Bumped with every migration; stored in SQLite's `user_version`.
pub const SCHEMA_VERSION: u32 = 1;
/// The API version in every path.
pub const API: &str = "v1";
/// Scans are a few tens of kilobytes; anything far larger is not a scan.
pub const MAX_SCAN_BYTES: usize = 4 * 1024 * 1024;
/// Most matches returned by one lookup.
pub const LOOKUP_LIMIT: usize = 50;

// ------------------------------------------------------------- wire types

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Health {
    pub ok: bool,
    pub api: String,
    pub server_version: String,
    pub grader_version: String,
    /// The device the token in the request belongs to, if one was sent and valid.
    pub device: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Uploaded {
    pub content_id: String,
    /// False when the server already had this exact scan.
    pub new: bool,
}

/// A stored scan without its body, as a lookup returns it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteScan {
    pub content_id: String,
    pub started_at: u64,
    pub received_at: u64,
    /// The name of the device that uploaded it.
    pub device: String,
    pub os: String,
    pub machine: Option<String>,
    pub serial: Option<String>,
    /// Graded now, by this server's grader.
    pub headline: Verdict,
    /// Which identifiers matched the lookup: `uuid`, `serial`, `board_serial`.
    pub matched_on: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lookup {
    pub matches: Vec<RemoteScan>,
    /// True when more matches exist than were returned.
    pub truncated: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LookupQuery {
    pub uuid: Option<String>,
    pub serial: Option<String>,
    pub board_serial: Option<String>,
    /// Leave this scan out of the results, usually the one being viewed.
    pub exclude: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
}

// ---------------------------------------------------------------- storage

#[derive(Debug)]
pub enum Error {
    Sqlite(rusqlite::Error),
    NewerSchema(u32),
    DuplicateDevice(String),
    UnknownDevice(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Sqlite(e) => write!(f, "database: {e}"),
            Error::NewerSchema(v) => write!(
                f,
                "the database was written by a newer probe-sync (schema {v}, this build reads {SCHEMA_VERSION})"
            ),
            Error::DuplicateDevice(n) => write!(f, "a device named {n:?} already exists"),
            Error::UnknownDevice(n) => write!(f, "no active device named {n:?}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Sqlite(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Device {
    pub id: i64,
    pub name: String,
    pub created_at: u64,
    pub revoked_at: Option<u64>,
    pub scans: u64,
}

/// What happened to an upload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Put {
    Created,
    AlreadyStored,
}

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_connection(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(conn: Connection) -> Result<Self> {
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        let mut db = Db { conn };
        db.migrate()?;
        Ok(db)
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
                "CREATE TABLE devices (
                    id          INTEGER PRIMARY KEY,
                    name        TEXT NOT NULL UNIQUE,
                    token_hash  TEXT NOT NULL UNIQUE,
                    created_at  INTEGER NOT NULL,
                    revoked_at  INTEGER
                );
                CREATE TABLE scans (
                    content_id   TEXT PRIMARY KEY,
                    scan_json    TEXT NOT NULL,
                    device_id    INTEGER NOT NULL REFERENCES devices (id),
                    received_at  INTEGER NOT NULL,
                    started_at   INTEGER NOT NULL,
                    os           TEXT NOT NULL,
                    machine      TEXT,
                    serial       TEXT,
                    board_serial TEXT,
                    uuid         TEXT
                );
                CREATE INDEX scans_by_serial ON scans (serial);
                CREATE INDEX scans_by_board_serial ON scans (board_serial);
                CREATE INDEX scans_by_uuid ON scans (uuid);
                PRAGMA user_version = 1;",
            )?;
            tx.commit()?;
        }
        Ok(())
    }

    // --------------------------------------------------------------- devices

    /// Register a device and return its token. The token is not stored and
    /// cannot be shown again.
    pub fn add_device(&mut self, name: &str) -> Result<String> {
        let name = name.trim();
        let token = new_token();
        match self.conn.execute(
            "INSERT INTO devices (name, token_hash, created_at) VALUES (?1, ?2, ?3)",
            params![name, token_hash(&token), now() as i64],
        ) {
            Ok(_) => Ok(token),
            Err(rusqlite::Error::SqliteFailure(e, _))
                if e.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                Err(Error::DuplicateDevice(name.to_string()))
            }
            Err(e) => Err(e.into()),
        }
    }

    pub fn revoke_device(&mut self, name: &str) -> Result<()> {
        match self.conn.execute(
            "UPDATE devices SET revoked_at = ?1 WHERE name = ?2 AND revoked_at IS NULL",
            params![now() as i64, name.trim()],
        )? {
            0 => Err(Error::UnknownDevice(name.to_string())),
            _ => Ok(()),
        }
    }

    pub fn devices(&self) -> Result<Vec<Device>> {
        let mut q = self.conn.prepare(
            "SELECT d.id, d.name, d.created_at, d.revoked_at,
                    (SELECT COUNT(*) FROM scans s WHERE s.device_id = d.id)
             FROM devices d ORDER BY d.name",
        )?;
        let rows = q.query_map([], |r| {
            Ok(Device {
                id: r.get(0)?,
                name: r.get(1)?,
                created_at: r.get::<_, i64>(2)?.max(0) as u64,
                revoked_at: r.get::<_, Option<i64>>(3)?.map(|t| t.max(0) as u64),
                scans: r.get::<_, i64>(4)?.max(0) as u64,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// The active device a token belongs to.
    pub fn device_for_token(&self, token: &str) -> Result<Option<(i64, String)>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, name FROM devices WHERE token_hash = ?1 AND revoked_at IS NULL",
                [token_hash(token)],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }

    // ----------------------------------------------------------------- scans

    /// Store an already validated scan.
    pub fn put_scan(
        &mut self,
        content_id: &str,
        json: &str,
        scan: &Scan,
        device: i64,
    ) -> Result<Put> {
        let id = &scan.identity;
        let machine = [id.vendor.get(), id.model.get()]
            .into_iter()
            .flatten()
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        let changed = self.conn.execute(
            "INSERT OR IGNORE INTO scans (content_id, scan_json, device_id, received_at, started_at,
                                          os, machine, serial, board_serial, uuid)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                content_id,
                json,
                device,
                now() as i64,
                scan.started_at as i64,
                scan.os,
                (!machine.is_empty()).then_some(machine),
                id.serial.get(),
                id.board_serial.get(),
                id.uuid.get().map(|u| u.to_ascii_lowercase()),
            ],
        )?;
        Ok(if changed == 1 {
            Put::Created
        } else {
            Put::AlreadyStored
        })
    }

    pub fn get_scan(&self, content_id: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT scan_json FROM scans WHERE content_id = ?1",
                [content_id],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Delete a scan the device uploaded. `Ok(false)` when there is no such
    /// scan from this device.
    pub fn delete_scan(&mut self, content_id: &str, device: i64) -> Result<bool> {
        Ok(self.conn.execute(
            "DELETE FROM scans WHERE content_id = ?1 AND device_id = ?2",
            params![content_id, device],
        )? == 1)
    }

    /// Scans that claim the same machine, newest first.
    pub fn lookup(&self, q: &LookupQuery) -> Result<Lookup> {
        let clean = |v: &Option<String>| {
            v.as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        };
        let uuid = clean(&q.uuid).map(|u| u.to_ascii_lowercase());
        let serial = clean(&q.serial);
        let board = clean(&q.board_serial);
        if uuid.is_none() && serial.is_none() && board.is_none() {
            return Ok(Lookup {
                matches: Vec::new(),
                truncated: false,
            });
        }
        let mut stmt = self.conn.prepare(
            "SELECT s.content_id, s.started_at, s.received_at, d.name, s.os, s.machine,
                    s.serial, s.board_serial, s.uuid, s.scan_json
             FROM scans s JOIN devices d ON d.id = s.device_id
             WHERE (?4 IS NULL OR s.content_id != ?4)
               AND ((?1 IS NOT NULL AND s.uuid = ?1)
                 OR (?2 IS NOT NULL AND s.serial = ?2)
                 OR (?3 IS NOT NULL AND s.board_serial = ?3))
             ORDER BY s.started_at DESC, s.content_id
             LIMIT ?5",
        )?;
        let rows = stmt.query_map(
            params![
                uuid,
                serial,
                board,
                clean(&q.exclude),
                (LOOKUP_LIMIT + 1) as i64
            ],
            |r| {
                let row_serial: Option<String> = r.get(6)?;
                let row_board: Option<String> = r.get(7)?;
                let row_uuid: Option<String> = r.get(8)?;
                let json: String = r.get(9)?;
                let mut matched_on = Vec::new();
                if uuid.is_some() && row_uuid == uuid {
                    matched_on.push("uuid".to_string());
                }
                if serial.is_some() && row_serial == serial {
                    matched_on.push("serial".to_string());
                }
                if board.is_some() && row_board == board {
                    matched_on.push("board_serial".to_string());
                }
                // Graded now, so a lookup never shows an outdated verdict.
                let headline = serde_json::from_str::<Scan>(&json)
                    .map(|s| grade::grade(&s).headline)
                    .unwrap_or(Verdict::NotGraded);
                Ok(RemoteScan {
                    content_id: r.get(0)?,
                    started_at: r.get::<_, i64>(1)?.max(0) as u64,
                    received_at: r.get::<_, i64>(2)?.max(0) as u64,
                    device: r.get(3)?,
                    os: r.get(4)?,
                    machine: r.get(5)?,
                    serial: row_serial,
                    headline,
                    matched_on,
                })
            },
        )?;
        let mut matches: Vec<RemoteScan> = rows.collect::<std::result::Result<_, _>>()?;
        let truncated = matches.len() > LOOKUP_LIMIT;
        matches.truncate(LOOKUP_LIMIT);
        Ok(Lookup { matches, truncated })
    }
}

/// A fresh access token: `probe_` and 32 random bytes in hex.
fn new_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("the operating system's random source is available");
    format!("probe_{}", hex(&bytes))
}

pub fn token_hash(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

/// SHA-256 of a scan's JSON, in hex: the same content id the local store uses.
pub fn content_id(json: &str) -> String {
    hex(&Sha256::digest(json.as_bytes()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn is_content_id(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// -------------------------------------------------------------------- http

pub type Shared = Arc<Mutex<Db>>;

pub fn router(db: Shared) -> Router {
    Router::new()
        .route(&format!("/{API}/health"), get(health))
        .route(
            &format!("/{API}/scans/{{content_id}}"),
            put(upload).get(download).delete(remove),
        )
        .route(&format!("/{API}/lookup"), get(lookup))
        .layer(DefaultBodyLimit::max(MAX_SCAN_BYTES))
        .with_state(db)
}

fn fail(status: StatusCode, message: impl Into<String>) -> Response {
    (
        status,
        Json(ApiError {
            error: message.into(),
        }),
    )
        .into_response()
}

/// Run a database call off the async runtime.
async fn with_db<T: Send + 'static>(
    db: &Shared,
    f: impl FnOnce(&mut Db) -> Result<T> + Send + 'static,
) -> std::result::Result<T, Response> {
    let db = db.clone();
    tokio::task::spawn_blocking(move || {
        let mut guard = db
            .lock()
            .map_err(|_| "database unavailable after an earlier failure".to_string())?;
        f(&mut guard).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, e))
}

fn bearer(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

/// The device behind the request's token, or a 401 response.
async fn authenticate(
    db: &Shared,
    headers: &HeaderMap,
) -> std::result::Result<(i64, String), Response> {
    let Some(token) = bearer(headers) else {
        return Err(fail(
            StatusCode::UNAUTHORIZED,
            "an access token is required",
        ));
    };
    with_db(db, move |d| d.device_for_token(&token))
        .await?
        .ok_or_else(|| {
            fail(
                StatusCode::UNAUTHORIZED,
                "this access token is not valid or was revoked",
            )
        })
}

async fn health(State(db): State<Shared>, headers: HeaderMap) -> Response {
    let device = match bearer(&headers) {
        Some(token) => match with_db(&db, move |d| d.device_for_token(&token)).await {
            Ok(found) => found.map(|(_, name)| name),
            Err(r) => return r,
        },
        None => None,
    };
    Json(Health {
        ok: true,
        api: API.to_string(),
        server_version: env!("CARGO_PKG_VERSION").to_string(),
        grader_version: grade::GRADER_VERSION.to_string(),
        device,
    })
    .into_response()
}

async fn upload(
    State(db): State<Shared>,
    UrlPath(content_id): UrlPath<String>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let (device, _) = match authenticate(&db, &headers).await {
        Ok(d) => d,
        Err(r) => return r,
    };
    if !is_content_id(&content_id) {
        return fail(
            StatusCode::BAD_REQUEST,
            "the content id must be 64 lower-case hex digits",
        );
    }
    if self::content_id(&body) != content_id {
        return fail(
            StatusCode::BAD_REQUEST,
            "the body does not match its content id: it was altered or cut short",
        );
    }
    let scan: Scan = match serde_json::from_str(&body) {
        Ok(s) => s,
        Err(e) => {
            return fail(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("not a probe scan: {e}"),
            )
        }
    };
    if scan.schema_version > SCAN_SCHEMA_VERSION {
        return fail(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!(
                "the scan uses schema {}, newer than this server reads ({SCAN_SCHEMA_VERSION}): update probe-sync",
                scan.schema_version
            ),
        );
    }
    let id = content_id.clone();
    match with_db(&db, move |d| d.put_scan(&id, &body, &scan, device)).await {
        Ok(put) => {
            let status = if put == Put::Created {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            };
            (
                status,
                Json(Uploaded {
                    content_id,
                    new: put == Put::Created,
                }),
            )
                .into_response()
        }
        Err(r) => r,
    }
}

async fn download(
    State(db): State<Shared>,
    UrlPath(content_id): UrlPath<String>,
    headers: HeaderMap,
) -> Response {
    if let Err(r) = authenticate(&db, &headers).await {
        return r;
    }
    if !is_content_id(&content_id) {
        return fail(
            StatusCode::BAD_REQUEST,
            "the content id must be 64 lower-case hex digits",
        );
    }
    match with_db(&db, move |d| d.get_scan(&content_id)).await {
        Ok(Some(json)) => ([(header::CONTENT_TYPE, "application/json")], json).into_response(),
        Ok(None) => fail(StatusCode::NOT_FOUND, "no such scan"),
        Err(r) => r,
    }
}

async fn remove(
    State(db): State<Shared>,
    UrlPath(content_id): UrlPath<String>,
    headers: HeaderMap,
) -> Response {
    let (device, _) = match authenticate(&db, &headers).await {
        Ok(d) => d,
        Err(r) => return r,
    };
    match with_db(&db, move |d| d.delete_scan(&content_id, device)).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => fail(
            StatusCode::NOT_FOUND,
            "no such scan uploaded by this device",
        ),
        Err(r) => r,
    }
}

async fn lookup(
    State(db): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<LookupQuery>,
) -> Response {
    if let Err(r) = authenticate(&db, &headers).await {
        return r;
    }
    match with_db(&db, move |d| d.lookup(&q)).await {
        Ok(l) => Json(l).into_response(),
        Err(r) => r,
    }
}

#[cfg(test)]
mod tests;
