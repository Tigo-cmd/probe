//! Team sharing: the connection to a probe-sync server.
//!
//! The server address and this device's token live in `sync.json` in the
//! app's data folder, readable only by the user on Unix. The token never
//! goes back to the frontend. Sharing is per scan and only on request: the
//! frontend queues a scan, then a sync pass uploads everything queued.
//!
//! A pass takes the queue from the history, releases it while uploading so
//! the app stays responsive, then records the outcomes.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use hwprobe::Scan;
use scanstore::Summary;
use serde::Serialize;
use syncclient::{normalize_server, Client, DrainReport, Lookup, Settings};

use crate::History;

/// Scans sent per pass; a larger queue drains over several passes.
const BATCH: usize = 25;

pub struct Team {
    path: PathBuf,
    settings: Mutex<Option<Settings>>,
    /// The device name the server reported at the last successful check.
    device: Mutex<Option<String>>,
}

/// What the frontend may know about the connection. Never the token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TeamStatus {
    pub configured: bool,
    pub server: Option<String>,
    pub device: Option<String>,
}

impl Team {
    pub fn open(path: PathBuf) -> Self {
        let settings = fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<Settings>(&t).ok());
        Team {
            path,
            settings: Mutex::new(settings),
            device: Mutex::new(None),
        }
    }

    pub fn status(&self) -> TeamStatus {
        let settings = self.settings.lock().ok().and_then(|s| s.clone());
        TeamStatus {
            configured: settings.is_some(),
            server: settings.map(|s| s.server),
            device: self.device.lock().ok().and_then(|d| d.clone()),
        }
    }

    fn client(&self) -> Result<Client, String> {
        let settings = self
            .settings
            .lock()
            .map_err(|_| "team settings unavailable".to_string())?
            .clone()
            .ok_or("team sharing is not set up")?;
        Client::new(&settings).map_err(|e| e.to_string())
    }

    /// Check the server and token, then keep them.
    pub fn connect(&self, server: &str, token: &str) -> Result<TeamStatus, String> {
        let settings = Settings {
            server: normalize_server(server)?,
            token: token.trim().to_string(),
        };
        let health = Client::new(&settings)
            .and_then(|c| c.health())
            .map_err(|e| e.to_string())?;
        write_private(
            &self.path,
            &serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?,
        )?;
        *self
            .settings
            .lock()
            .map_err(|_| "team settings unavailable")? = Some(settings);
        *self
            .device
            .lock()
            .map_err(|_| "team settings unavailable")? = health.device;
        Ok(self.status())
    }

    /// Forget the server and token. Scans already shared stay on the server.
    pub fn disconnect(&self) -> Result<TeamStatus, String> {
        match fs::remove_file(&self.path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("{}: {e}", self.path.display())),
        }
        *self
            .settings
            .lock()
            .map_err(|_| "team settings unavailable")? = None;
        *self
            .device
            .lock()
            .map_err(|_| "team settings unavailable")? = None;
        Ok(self.status())
    }

    /// Upload whatever the user has queued.
    pub fn sync(&self, history: &History) -> Result<DrainReport, String> {
        let client = self.client()?;
        let batch = history.with(|s| syncclient::queued(s, BATCH).map_err(|e| e.to_string()))?;
        if batch.is_empty() {
            return Ok(DrainReport::default());
        }
        let outcomes = syncclient::upload_batch(&client, batch);
        history.with(|s| syncclient::record(s, outcomes).map_err(|e| e.to_string()))
    }

    pub fn lookup(&self, scan: &Scan, exclude: Option<&str>) -> Result<Lookup, String> {
        self.client()?
            .lookup(scan, exclude)
            .map_err(|e| e.to_string())
    }
}

/// Write a file only its owner can read: it holds an access token.
fn write_private(path: &Path, text: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("tmp");
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let write = || -> std::io::Result<()> {
        use std::io::Write;
        let mut f = opts.open(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        fs::rename(&tmp, path)
    };
    write().map_err(|e| format!("{}: {e}", path.display()))
}

/// A scan's history row after a sharing change, plus what the pass did.
#[derive(Debug, Clone, Serialize)]
pub struct Shared {
    pub record: Summary,
    pub report: Option<DrainReport>,
    /// Why the upload did not happen yet, when it did not.
    pub error: Option<String>,
}

/// Queue a scan for sharing and try to send it now. A failed attempt leaves
/// it queued; the next pass retries it.
pub fn share(team: &Team, history: &History, id: i64) -> Result<Shared, String> {
    history.with(|s| s.queue_for_sync(id).map_err(|e| e.to_string()))?;
    let (report, error) = match team.sync(history) {
        Ok(r) => {
            let e = r.stopped.clone();
            (Some(r), e)
        }
        Err(e) => (None, Some(e)),
    };
    let record = history.with(|s| s.summary(id).map_err(|e| e.to_string()))?;
    Ok(Shared {
        record,
        report,
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_survive_a_restart_and_the_token_is_never_reported() {
        let dir = std::env::temp_dir().join(format!("probe-team-{}", std::process::id()));
        let path = dir.join("sync.json");
        write_private(
            &path,
            &serde_json::to_string(&Settings {
                server: "https://sync.example.com".into(),
                token: "probe_secret".into(),
            })
            .unwrap(),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let team = Team::open(path.clone());
        let status = team.status();
        assert!(status.configured);
        assert_eq!(status.server.as_deref(), Some("https://sync.example.com"));
        assert!(!serde_json::to_string(&status).unwrap().contains("secret"));
        assert!(!team.disconnect().unwrap().configured);
        assert!(!path.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn plain_http_to_another_host_is_refused_before_anything_is_sent() {
        let team = Team::open(std::env::temp_dir().join("probe-team-unused/sync.json"));
        let e = team
            .connect("http://sync.example.com", "probe_x")
            .unwrap_err();
        assert!(e.contains("https"));
        assert!(!team.status().configured);
    }
}
