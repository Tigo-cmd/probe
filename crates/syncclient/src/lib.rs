//! Client for the probe-sync team server.
//!
//! The local history decides what is shared: only scans the user queued
//! (`pending` or `failed`) are ever sent. [`drain`] uploads them oldest first
//! and records each outcome in the store. The server checks every upload
//! against its content id, so retrying after a lost response is safe.
//!
//! Scans carry serial numbers and MAC addresses, so the client refuses plain
//! HTTP to anything but this computer.

use std::fmt;
use std::time::Duration;

use hwprobe::grade::Verdict;
use hwprobe::Scan;
use scanstore::Store;
use serde::{Deserialize, Serialize};

pub const API: &str = "v1";
const TIMEOUT: Duration = Duration::from_secs(20);

/// Where to sync and as which device. Kept by the app in its data folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub server: String,
    pub token: String,
}

/// A server address in canonical form: scheme and host, no trailing slash.
/// HTTPS is required unless the host is this computer.
pub fn normalize_server(input: &str) -> Result<String, String> {
    let s = input.trim().trim_end_matches('/');
    let (scheme, rest) = s
        .split_once("://")
        .ok_or("the server address must start with https://")?;
    let scheme = scheme.to_ascii_lowercase();
    let host_port = rest.split('/').next().unwrap_or("");
    if host_port.is_empty() || rest.contains(['?', '#', '@', ' ']) {
        return Err("the server address is not a valid web address".into());
    }
    let host = if let Some(v6) = host_port.strip_prefix('[') {
        v6.split(']').next().unwrap_or("")
    } else {
        host_port.split(':').next().unwrap_or("")
    }
    .to_ascii_lowercase();
    let loopback_v4 = host.split('.').count() == 4
        && host.starts_with("127.")
        && host
            .split('.')
            .all(|p| !p.is_empty() && p.len() <= 3 && p.bytes().all(|b| b.is_ascii_digit()));
    let local = host == "localhost" || host == "::1" || loopback_v4;
    match scheme.as_str() {
        "https" => {}
        "http" if local => {}
        "http" => {
            return Err(
                "scans include serial numbers, so the server must use https:// (plain http is allowed only for localhost)"
                    .into(),
            )
        }
        _ => return Err("the server address must start with https://".into()),
    }
    Ok(format!("{scheme}://{rest}"))
}

// ------------------------------------------------------------- wire types

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Health {
    pub ok: bool,
    pub api: String,
    pub server_version: String,
    pub grader_version: String,
    pub device: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Uploaded {
    pub content_id: String,
    pub new: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteScan {
    pub content_id: String,
    pub started_at: u64,
    pub received_at: u64,
    pub device: String,
    pub os: String,
    pub machine: Option<String>,
    pub serial: Option<String>,
    pub headline: Verdict,
    pub matched_on: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lookup {
    pub matches: Vec<RemoteScan>,
    pub truncated: bool,
}

#[derive(Deserialize)]
struct ApiError {
    error: String,
}

// ----------------------------------------------------------------- errors

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncError {
    /// The settings are unusable.
    Config(String),
    /// The server could not be reached.
    Network(String),
    /// The token was refused or revoked.
    Unauthorized(String),
    /// The server refused this one scan; other scans may still go through.
    Rejected { status: u16, message: String },
    /// The server failed; try again later.
    Server { status: u16, message: String },
    /// The reply was not what this client understands.
    BadResponse(String),
}

impl SyncError {
    /// Whether to stop sending the rest of the queue: the next scan would
    /// fail the same way.
    pub fn stops_drain(&self) -> bool {
        !matches!(self, SyncError::Rejected { .. })
    }
}

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncError::Config(m) => write!(f, "{m}"),
            SyncError::Network(m) => write!(f, "could not reach the team server: {m}"),
            SyncError::Unauthorized(m) => write!(f, "the team server refused this device: {m}"),
            SyncError::Rejected { status, message } => {
                write!(f, "the team server refused the scan ({status}): {message}")
            }
            SyncError::Server { status, message } => {
                write!(f, "the team server failed ({status}): {message}")
            }
            SyncError::BadResponse(m) => write!(f, "unexpected reply from the team server: {m}"),
        }
    }
}

impl std::error::Error for SyncError {}

// ----------------------------------------------------------------- client

pub struct Client {
    agent: ureq::Agent,
    base: String,
    token: String,
}

impl Client {
    pub fn new(settings: &Settings) -> Result<Self, SyncError> {
        let base = normalize_server(&settings.server).map_err(SyncError::Config)?;
        let token = settings.token.trim().to_string();
        if token.is_empty() || token.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(SyncError::Config(
                "the access token is missing or malformed".into(),
            ));
        }
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .http_status_as_error(false)
            .build()
            .into();
        Ok(Client { agent, base, token })
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{API}/{path}", self.base)
    }

    fn auth(&self) -> String {
        format!("Bearer {}", self.token)
    }

    /// Check the server is reachable and the token is accepted.
    pub fn health(&self) -> Result<Health, SyncError> {
        let res = self
            .agent
            .get(&self.url("health"))
            .header("Authorization", &self.auth())
            .call();
        let health: Health = parse(res)?;
        if health.api != API {
            return Err(SyncError::BadResponse(format!(
                "the server speaks API {}, this probe speaks {API}",
                health.api
            )));
        }
        if health.device.is_none() {
            return Err(SyncError::Unauthorized(
                "this access token is not valid or was revoked".into(),
            ));
        }
        Ok(health)
    }

    /// Upload one scan exactly as stored locally.
    pub fn upload(&self, content_id: &str, json: &str) -> Result<Uploaded, SyncError> {
        let res = self
            .agent
            .put(&self.url(&format!("scans/{content_id}")))
            .header("Authorization", &self.auth())
            .content_type("application/json")
            .send(json);
        parse(res)
    }

    /// Scans on the server that claim the same machine as `scan`.
    pub fn lookup(&self, scan: &Scan, exclude: Option<&str>) -> Result<Lookup, SyncError> {
        let id = &scan.identity;
        let mut req = self
            .agent
            .get(&self.url("lookup"))
            .header("Authorization", &self.auth());
        for (key, value) in [
            ("uuid", id.uuid.get()),
            ("serial", id.serial.get()),
            ("board_serial", id.board_serial.get()),
        ] {
            if let Some(v) = value {
                req = req.query(key, v);
            }
        }
        if let Some(e) = exclude {
            req = req.query("exclude", e);
        }
        parse(req.call())
    }

    /// Remove a scan this device uploaded.
    pub fn delete(&self, content_id: &str) -> Result<(), SyncError> {
        let res = self
            .agent
            .delete(&self.url(&format!("scans/{content_id}")))
            .header("Authorization", &self.auth())
            .call();
        match res {
            Ok(r) if r.status().as_u16() == 204 => Ok(()),
            other => parse::<serde_json::Value>(other).map(|_| ()),
        }
    }
}

fn parse<T: serde::de::DeserializeOwned>(
    res: Result<ureq::http::Response<ureq::Body>, ureq::Error>,
) -> Result<T, SyncError> {
    let mut res = res.map_err(|e| SyncError::Network(e.to_string()))?;
    let status = res.status().as_u16();
    let text = res
        .body_mut()
        .read_to_string()
        .map_err(|e| SyncError::Network(e.to_string()))?;
    if (200..300).contains(&status) {
        return serde_json::from_str(&text).map_err(|e| SyncError::BadResponse(e.to_string()));
    }
    let message = serde_json::from_str::<ApiError>(&text)
        .map(|e| e.error)
        .unwrap_or_else(|_| text.chars().take(200).collect());
    Err(match status {
        401 | 403 => SyncError::Unauthorized(message),
        400..=499 => SyncError::Rejected { status, message },
        _ => SyncError::Server { status, message },
    })
}

// ------------------------------------------------------------------ drain

/// What one pass over the queue did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DrainReport {
    /// Scans the server did not have before.
    pub sent: u32,
    /// Scans the server already had; marked synced all the same.
    pub already: u32,
    /// Scans that failed and stay queued for a retry.
    pub failed: u32,
    /// Why the pass stopped early, when it did.
    pub stopped: Option<String>,
}

/// One queued scan to upload: its local id, content id and stored JSON.
pub type Queued = (i64, String, String);

/// Upload `batch` in order, stopping at the first error that would repeat.
/// Returns each attempted scan's outcome; unattempted ones are left out.
pub fn upload_batch(
    client: &Client,
    batch: Vec<Queued>,
) -> Vec<(i64, Result<Uploaded, SyncError>)> {
    let mut out = Vec::new();
    for (id, content_id, json) in batch {
        let r = client.upload(&content_id, &json);
        let stop = r.as_ref().err().is_some_and(SyncError::stops_drain);
        out.push((id, r));
        if stop {
            break;
        }
    }
    out
}

/// Record upload outcomes in the store and summarise them.
pub fn record(
    store: &mut Store,
    outcomes: Vec<(i64, Result<Uploaded, SyncError>)>,
) -> scanstore::Result<DrainReport> {
    let mut report = DrainReport::default();
    for (id, r) in outcomes {
        match r {
            Ok(u) => {
                store.mark_synced(id)?;
                if u.new {
                    report.sent += 1;
                } else {
                    report.already += 1;
                }
            }
            Err(e) => {
                store.mark_failed(id, &e.to_string())?;
                report.failed += 1;
                if e.stops_drain() {
                    report.stopped = Some(e.to_string());
                }
            }
        }
    }
    Ok(report)
}

/// The queue's work list, in the form [`upload_batch`] takes.
pub fn queued(store: &Store, limit: usize) -> scanstore::Result<Vec<Queued>> {
    Ok(store
        .next_to_sync(limit)?
        .into_iter()
        .map(|(s, json)| (s.id, s.content_id, json))
        .collect())
}

/// Send up to `limit` queued scans and record the outcomes. Holds the store
/// for the whole pass; the desktop app instead takes the batch, releases the
/// store while uploading, and records afterwards.
pub fn drain(store: &mut Store, client: &Client, limit: usize) -> scanstore::Result<DrainReport> {
    let batch = queued(store, limit)?;
    let outcomes = upload_batch(client, batch);
    record(store, outcomes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_http_is_only_for_this_computer() {
        assert_eq!(
            normalize_server(" https://sync.example.com/ ").unwrap(),
            "https://sync.example.com"
        );
        assert_eq!(
            normalize_server("HTTPS://sync.example.com:8443/probe").unwrap(),
            "https://sync.example.com:8443/probe"
        );
        assert!(normalize_server("http://localhost:8640").is_ok());
        assert!(normalize_server("http://127.0.0.1:8640").is_ok());
        assert!(normalize_server("http://[::1]:8640").is_ok());
        for bad in [
            "http://sync.example.com",
            "http://localhost.evil.com",
            "http://127.evil.com",
            "http://127.0.0.1.evil.com",
            "sync.example.com",
            "ftp://sync.example.com",
            "https://",
            "https://user@sync.example.com",
            "https://sync.example.com/?x=1",
        ] {
            assert!(normalize_server(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_missing_token_is_a_settings_problem() {
        let s = Settings {
            server: "https://sync.example.com".into(),
            token: "  ".into(),
        };
        assert!(matches!(Client::new(&s), Err(SyncError::Config(_))));
    }

    #[test]
    fn only_a_rejected_scan_lets_the_queue_continue() {
        assert!(!SyncError::Rejected {
            status: 422,
            message: String::new()
        }
        .stops_drain());
        assert!(SyncError::Network(String::new()).stops_drain());
        assert!(SyncError::Unauthorized(String::new()).stops_drain());
        assert!(SyncError::Server {
            status: 500,
            message: String::new()
        }
        .stops_drain());
    }
}
