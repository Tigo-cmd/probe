use super::*;
use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt;
use tower::ServiceExt;

fn app() -> (Shared, Router, String, String) {
    let mut db = Db::open_in_memory().unwrap();
    let a = db.add_device("front desk").unwrap();
    let b = db.add_device("workshop").unwrap();
    let shared = Arc::new(Mutex::new(db));
    (shared.clone(), router(shared), a, b)
}

fn scan(serial: &str) -> String {
    let mut s = Scan {
        schema_version: SCAN_SCHEMA_VERSION,
        os: "windows".into(),
        started_at: 1_791_302_400,
        ..Scan::default()
    };
    s.identity.serial = hwprobe::model::Field::claimed(Some(serial.into()), "t");
    s.identity.uuid = hwprobe::model::Field::claimed(Some("4C4C4544-0042".into()), "t");
    serde_json::to_string(&s).unwrap()
}

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: &str,
) -> (StatusCode, String) {
    let mut req = Request::builder().method(method).uri(uri);
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    let res = app
        .clone()
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn uploads_are_idempotent_and_checked_against_their_hash() {
    let (_, app, a, _) = app();
    let json = scan("PF0ABCDE");
    let id = content_id(&json);
    let uri = format!("/v1/scans/{id}");

    let (s, body) = call(&app, "PUT", &uri, Some(&a), &json).await;
    assert_eq!(s, StatusCode::CREATED, "{body}");
    let (s, body) = call(&app, "PUT", &uri, Some(&a), &json).await;
    assert_eq!(s, StatusCode::OK);
    assert!(body.contains("\"new\":false"));

    let cut = &json[..json.len() - 5];
    let (s, body) = call(
        &app,
        "PUT",
        &format!("/v1/scans/{}", content_id(cut)),
        Some(&a),
        cut,
    )
    .await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    let (s, _) = call(&app, "PUT", &uri, Some(&a), &scan("OTHER")).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "body must match its content id");
    let (s, _) = call(&app, "PUT", "/v1/scans/NOT-HEX", Some(&a), &json).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    let (s, body) = call(&app, "GET", &uri, Some(&a), "").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body, json, "stored exactly as uploaded");
}

#[tokio::test]
async fn every_call_but_health_needs_a_valid_token() {
    let (db, app, a, _) = app();
    let (s, body) = call(&app, "GET", "/v1/health", None, "").await;
    assert_eq!(s, StatusCode::OK);
    assert!(body.contains("\"device\":null"));
    let (_, body) = call(&app, "GET", "/v1/health", Some(&a), "").await;
    assert!(body.contains("front desk"));

    let json = scan("X");
    let uri = format!("/v1/scans/{}", content_id(&json));
    assert_eq!(
        call(&app, "PUT", &uri, None, &json).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&app, "PUT", &uri, Some("probe_wrong"), &json).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&app, "GET", "/v1/lookup?serial=X", None, "").await.0,
        StatusCode::UNAUTHORIZED
    );

    db.lock().unwrap().revoke_device("front desk").unwrap();
    assert_eq!(
        call(&app, "PUT", &uri, Some(&a), &json).await.0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn lookup_finds_the_same_machine_from_other_devices() {
    let (_, app, a, b) = app();
    let first = scan("PF0ABCDE");
    let second = scan("PF0ABCDE").replace("1791302400", "1791388800");
    for (json, token) in [(&first, &a), (&second, &b), (&scan("ELSE"), &a)] {
        let uri = format!("/v1/scans/{}", content_id(json));
        assert_eq!(
            call(&app, "PUT", &uri, Some(token), json).await.0,
            StatusCode::CREATED
        );
    }
    let (s, body) = call(
        &app,
        "GET",
        &format!("/v1/lookup?serial=PF0ABCDE&exclude={}", content_id(&second)),
        Some(&b),
        "",
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let l: Lookup = serde_json::from_str(&body).unwrap();
    assert_eq!(
        l.matches.len(),
        1,
        "the excluded scan and the other machine are left out"
    );
    assert_eq!(l.matches[0].device, "front desk");
    assert_eq!(l.matches[0].matched_on, ["serial"]);

    // The UUID is matched case-insensitively and every scan above shares it.
    let (_, body) = call(&app, "GET", "/v1/lookup?uuid=4c4c4544-0042", Some(&b), "").await;
    let l: Lookup = serde_json::from_str(&body).unwrap();
    assert_eq!(l.matches.len(), 3);
    assert_eq!(l.matches[0].started_at, 1_791_388_800, "newest first");

    let (_, body) = call(&app, "GET", "/v1/lookup", Some(&b), "").await;
    assert!(serde_json::from_str::<Lookup>(&body)
        .unwrap()
        .matches
        .is_empty());
}

#[tokio::test]
async fn only_the_uploading_device_can_delete() {
    let (_, app, a, b) = app();
    let json = scan("DEL");
    let uri = format!("/v1/scans/{}", content_id(&json));
    call(&app, "PUT", &uri, Some(&a), &json).await;
    assert_eq!(
        call(&app, "DELETE", &uri, Some(&b), "").await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&app, "DELETE", &uri, Some(&a), "").await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&app, "GET", &uri, Some(&a), "").await.0,
        StatusCode::NOT_FOUND
    );
}

#[test]
fn tokens_are_stored_hashed_and_names_are_unique() {
    let mut db = Db::open_in_memory().unwrap();
    let t = db.add_device("a").unwrap();
    assert!(t.starts_with("probe_") && t.len() == 70);
    let stored: String = db
        .conn
        .query_row("SELECT token_hash FROM devices", [], |r| r.get(0))
        .unwrap();
    assert_ne!(stored, t);
    assert_eq!(stored, token_hash(&t));
    assert!(matches!(
        db.add_device(" a "),
        Err(Error::DuplicateDevice(_))
    ));
    assert!(matches!(
        db.revoke_device("nobody"),
        Err(Error::UnknownDevice(_))
    ));
}

#[test]
fn a_newer_database_is_refused() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA user_version = 99").unwrap();
    assert!(matches!(
        Db::from_connection(conn),
        Err(Error::NewerSchema(99))
    ));
}
