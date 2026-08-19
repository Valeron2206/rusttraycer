//! F7 harden (no behavior change): secrets must not land in pid.json or host.db columns.

use rt_storage::Store;
use tempfile::tempdir;

const FORBIDDEN: &[&str] = &["token", "pat", "password", "dsn", "keyring"];

fn is_forbidden(name: &str) -> bool {
    let low = name.to_ascii_lowercase();
    FORBIDDEN.iter().any(|k| low == *k)
}

#[test]
fn pid_json_shape_has_no_secret_keys() {
    let dir = tempdir().unwrap();
    let info = rt_host::bind::PidFile::new("host-1".into(), 1, 1234);
    rt_host::bind::write_pid_file(dir.path(), &info).unwrap();
    let text = std::fs::read_to_string(rt_host::bind::pid_path(dir.path())).unwrap();
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    let obj = v.as_object().expect("pid.json object");
    for key in obj.keys() {
        assert!(
            !is_forbidden(key),
            "pid.json must not contain secret key {key}"
        );
    }
    if let Some(proto) = obj.get("protocol").and_then(|p| p.as_object()) {
        for key in proto.keys() {
            assert!(
                !is_forbidden(key),
                "pid.json protocol must not contain secret key {key}"
            );
        }
    }
    for k in ["hostId", "pid", "rpcUrl", "wsUrl", "startedAt", "protocol"] {
        assert!(obj.contains_key(k), "missing public key {k}");
    }
}

#[test]
fn host_schema_tables_have_no_secret_columns() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("host.db");
    let store = Store::open(&db).unwrap();
    store.migrate().unwrap();
    drop(store);

    let conn = rusqlite::Connection::open(&db).unwrap();
    for table in ["agents", "tasks", "host", "policies"] {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info(\"{table}\")"))
            .unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert!(!cols.is_empty(), "table {table} must exist after migrate");
        for c in &cols {
            assert!(!is_forbidden(c), "secret-like column {c} on {table}");
        }
    }
}
