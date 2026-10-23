//! The client against the real server, over a real socket.

use std::sync::{Arc, Mutex};

use hwprobe::model::{Field, SCAN_SCHEMA_VERSION};
use hwprobe::Scan;
use scanstore::{Origin, Store, SyncState};
use syncclient::{drain, Client, Settings, SyncError};

/// Start a server on a free port; returns its address and a token per device.
fn server(devices: &[&str]) -> (String, Vec<String>, Arc<Mutex<probe_sync::Db>>) {
    let mut db = probe_sync::Db::open_in_memory().unwrap();
    let tokens = devices.iter().map(|d| db.add_device(d).unwrap()).collect();
    let db = Arc::new(Mutex::new(db));
    let app = probe_sync::router(db.clone());
    let std_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    std_listener.set_nonblocking(true).unwrap();
    let addr = std_listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(std_listener).unwrap();
            axum::serve(listener, app).await.unwrap();
        });
    });
    (format!("http://{addr}"), tokens, db)
}

fn scan(serial: &str, at: u64) -> Scan {
    let mut s = Scan {
        schema_version: SCAN_SCHEMA_VERSION,
        os: "windows".into(),
        started_at: at,
        ..Scan::default()
    };
    s.identity.serial = Field::claimed(Some(serial.into()), "t");
    s
}

fn client(server: &str, token: &str) -> Client {
    Client::new(&Settings {
        server: server.into(),
        token: token.into(),
    })
    .unwrap()
}

#[test]
fn queued_scans_reach_the_server_and_other_devices_can_find_them() {
    let (url, tokens, _) = server(&["front desk", "workshop"]);
    let desk = client(&url, &tokens[0]);
    assert_eq!(desk.health().unwrap().device.as_deref(), Some("front desk"));

    let mut store = Store::open_in_memory().unwrap();
    let kept = store
        .insert(&scan("PF0ABCDE", 1_000), Origin::Live)
        .unwrap()
        .id;
    let shared = store
        .insert(&scan("PF0ABCDE", 2_000), Origin::Live)
        .unwrap()
        .id;
    store.queue_for_sync(shared).unwrap();

    let report = drain(&mut store, &desk, 10).unwrap();
    assert_eq!((report.sent, report.already, report.failed), (1, 0, 0));
    assert_eq!(store.summary(shared).unwrap().sync, SyncState::Synced);
    assert_eq!(
        store.summary(kept).unwrap().sync,
        SyncState::Local,
        "never sent unasked"
    );
    assert_eq!(
        drain(&mut store, &desk, 10).unwrap(),
        Default::default(),
        "nothing left"
    );

    // The same scan queued again from a second copy of the history is a no-op upload.
    let mut other = Store::open_in_memory().unwrap();
    let again = other
        .insert(&scan("PF0ABCDE", 2_000), Origin::Imported)
        .unwrap()
        .id;
    other.queue_for_sync(again).unwrap();
    let workshop = client(&url, &tokens[1]);
    assert_eq!(drain(&mut other, &workshop, 10).unwrap().already, 1);

    let found = workshop.lookup(&scan("PF0ABCDE", 3_000), None).unwrap();
    assert_eq!(found.matches.len(), 1);
    assert_eq!(found.matches[0].device, "front desk");
    assert_eq!(found.matches[0].matched_on, ["serial"]);
    let id = &found.matches[0].content_id;
    assert!(workshop
        .lookup(&scan("PF0ABCDE", 3_000), Some(id))
        .unwrap()
        .matches
        .is_empty());

    assert!(matches!(
        workshop.delete(id),
        Err(SyncError::Rejected { status: 404, .. })
    ));
    desk.delete(id).unwrap();
}

#[test]
fn a_revoked_token_stops_the_pass_and_keeps_scans_queued() {
    let (url, tokens, db) = server(&["laptop"]);
    db.lock().unwrap().revoke_device("laptop").unwrap();
    let c = client(&url, &tokens[0]);
    assert!(matches!(c.health(), Err(SyncError::Unauthorized(_))));

    let mut store = Store::open_in_memory().unwrap();
    for at in [1, 2, 3] {
        let id = store.insert(&scan("S", at), Origin::Live).unwrap().id;
        store.queue_for_sync(id).unwrap();
    }
    let report = drain(&mut store, &c, 10).unwrap();
    assert_eq!(
        report.failed, 1,
        "stops at the first refusal instead of burning every retry"
    );
    assert!(report.stopped.unwrap().contains("refused this device"));
    let counts = store.queue_counts().unwrap();
    assert_eq!((counts.pending, counts.failed), (2, 1));
}

#[test]
fn an_unreachable_server_is_a_network_error() {
    let c = client("http://127.0.0.1:9", "probe_x");
    assert!(matches!(c.health(), Err(SyncError::Network(_))));
}
