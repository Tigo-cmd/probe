//! probe-sync: run the team server and manage device tokens.

use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use probe_sync::{router, Db};

const USAGE: &str = "\
usage:
  probe-sync serve [--db FILE] [--listen ADDR]   run the server
  probe-sync token add NAME [--db FILE]          register a device; prints its token once
  probe-sync token list [--db FILE]              list devices
  probe-sync token revoke NAME [--db FILE]       stop a device's token working

defaults: --db probe-sync.sqlite3  --listen 127.0.0.1:8640

The server speaks plain HTTP. Serve it to other computers through a TLS
reverse proxy (Caddy, nginx); probe refuses plain HTTP except to localhost.";

const DEFAULT_DB: &str = "probe-sync.sqlite3";
const DEFAULT_LISTEN: &str = "127.0.0.1:8640";

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let db = take_option(&mut args, "--db").unwrap_or_else(|| DEFAULT_DB.into());
    let listen = take_option(&mut args, "--listen").unwrap_or_else(|| DEFAULT_LISTEN.into());
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match words.as_slice() {
        ["serve"] => serve(&db, &listen),
        ["token", "add", name] => with_db(&db, |d| {
            let token = d.add_device(name).map_err(|e| e.to_string())?;
            println!("Device {name:?} added. Its token, shown only now:\n\n  {token}\n");
            println!("Enter it in probe under Team sharing on that computer.");
            Ok(())
        }),
        ["token", "list"] => with_db(&db, |d| {
            let devices = d.devices().map_err(|e| e.to_string())?;
            if devices.is_empty() {
                println!("No devices yet. Add one with: probe-sync token add NAME");
            }
            for dev in devices {
                let state = if dev.revoked_at.is_some() {
                    "revoked"
                } else {
                    "active"
                };
                println!("{:<24} {:<8} {} scans", dev.name, state, dev.scans);
            }
            Ok(())
        }),
        ["token", "revoke", name] => with_db(&db, |d| {
            d.revoke_device(name).map_err(|e| e.to_string())?;
            println!("Device {name:?} revoked. Its scans stay on the server.");
            Ok(())
        }),
        ["-h" | "--help" | "help"] | [] => {
            println!("{USAGE}");
            Ok(())
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(64);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("probe-sync: {e}");
            ExitCode::FAILURE
        }
    }
}

fn take_option(args: &mut Vec<String>, name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == name)?;
    args.remove(i);
    (i < args.len()).then(|| args.remove(i))
}

fn with_db(path: &str, f: impl FnOnce(&mut Db) -> Result<(), String>) -> Result<(), String> {
    let mut db = Db::open(path).map_err(|e| format!("{path}: {e}"))?;
    f(&mut db)
}

fn serve(db_path: &str, listen: &str) -> Result<(), String> {
    let db = Db::open(db_path).map_err(|e| format!("{db_path}: {e}"))?;
    let active = db
        .devices()
        .map_err(|e| e.to_string())?
        .iter()
        .filter(|d| d.revoked_at.is_none())
        .count();
    let app = router(Arc::new(Mutex::new(db)));
    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind(listen)
            .await
            .map_err(|e| format!("cannot listen on {listen}: {e}"))?;
        println!(
            "probe-sync listening on http://{listen} ({active} active devices, database {db_path})"
        );
        if active == 0 {
            println!("No devices can connect yet. Add one with: probe-sync token add NAME");
        }
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await
            .map_err(|e| e.to_string())
    })
}
