use neug_rust::{Database, DatabaseOptions, Mode};
use tempfile::tempdir;

#[test]
fn test_local_connection() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("local_conn_db");

    let mut db = Database::open(db_path, Mode::ReadWrite).unwrap();
    let mut conn = db.connect().unwrap();

    assert!(conn.is_open());
    conn.close();
    db.close();
}

#[test]
fn test_open_after_close() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_open_after_close_db");

    let mut db = Database::open(db_path, Mode::ReadWrite).unwrap();

    let mut conn = db.connect().unwrap();
    assert!(conn.is_open());
    conn.close();

    // try to open a new connection after closing the previous one
    let mut new_conn = db.connect().unwrap();
    assert!(new_conn.is_open());
    new_conn.close();

    db.close();
}

#[test]
fn test_local_connection_params() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("local_conn_param_db");

    let options = DatabaseOptions {
        db_path: db_path.to_string_lossy().into_owned(),
        mode: Mode::ReadWrite,
        max_thread_num: 4,
        checkpoint_on_close: true,
    };

    let mut db = Database::with_options(options).unwrap();
    let mut conn = db.connect().unwrap();
    assert!(conn.is_open());
    conn.close();
    db.close();
}

#[test]
fn test_local_connection_after_close() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("conn_after_close_db");

    let mut db = Database::open(db_path, Mode::ReadWrite).unwrap();
    let mut conn = db.connect().unwrap();
    conn.close();

    let res = conn.execute("MATCH (n) RETURN n");
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), "Connection is closed");

    db.close();
}

#[test]
fn test_parallel_connections() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("parallel_conn_db");

    let mut db = Database::open(db_path, Mode::ReadOnly).unwrap();

    let mut connections = vec![];
    for _ in 0..5 {
        connections.push(db.connect().unwrap());
    }

    for conn in connections.iter_mut() {
        let res = conn.execute("MATCH (n) RETURN n");
        assert!(res.is_ok());
        conn.close();
    }

    db.close();
}

#[test]
fn test_parallel_query_executions() {
    use std::thread;

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("parallel_query_db");

    let mut db = Database::open(db_path, Mode::ReadWrite).unwrap();
    // Simulate connection cloning for multiple threads (in real implementation may need Send/Sync bounds)
    // For this mock, we'll create separate connections per thread.

    let mut conn = db.connect().unwrap();
    conn.execute("CREATE NODE TABLE person(id INT64, name STRING, PRIMARY KEY(id));")
        .unwrap();

    let mut threads = vec![];

    for i in 0..10 {
        // In python it shared `conn`. Rust requires Sync for `conn`.
        // To mimic, we will open a new connection for each thread since our mock is not Sync.
        let local_conn = db.connect().unwrap();

        let t = thread::spawn(move || {
            for j in 0..10 {
                let node_id = i * 10 + j;
                let query = format!(
                    "CREATE (p: person {{id: {}, name: 'Node{}'}});",
                    node_id, node_id
                );
                local_conn.execute(&query).unwrap();
            }
        });
        threads.push(t);
    }

    for t in threads {
        t.join().unwrap();
    }

    let res = conn.execute("MATCH (p) RETURN p.id AS id ORDER BY id;");
    assert!(res.is_ok());

    conn.close();
    db.close();
}

#[test]
fn test_access_mode() {
    use neug_rust::AccessMode;

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("access_mode_db");

    let mut db = Database::open(db_path, Mode::ReadWrite).unwrap();
    let mut conn = db.connect().unwrap();

    let modes = [
        AccessMode::Read,
        AccessMode::Insert,
        AccessMode::Update,
        AccessMode::Schema,
    ];

    for mode in modes {
        let query = format!(
            "CREATE NODE TABLE test_table_{}(id INT64, PRIMARY KEY(id));",
            mode.as_str()
        );
        let res = conn.execute_with_options(&query, Some(mode), None);
        assert!(res.is_ok());
    }

    // In Rust, because `AccessMode` is an enum, we cannot accidentally pass invalid modes
    // like "delete" or "drop" which the python test tests for. This proves Rust API is safer.

    conn.close();
    db.close();
}
