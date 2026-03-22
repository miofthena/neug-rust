use criterion::{Criterion, criterion_group, criterion_main};
use neug_rust::{Database, Mode};
use std::hint::black_box;
use tempfile::tempdir;

fn bench_connection_lifecycle(c: &mut Criterion) {
    let dir = tempdir().unwrap();
    let db = Database::open(dir.path(), Mode::ReadOnly).unwrap();

    c.bench_function("connection_lifecycle", |b| {
        b.iter(|| {
            let conn = db.connect().unwrap();
            black_box(conn.is_open());
        })
    });
}

fn bench_query_dispatch(c: &mut Criterion) {
    let dir = tempdir().unwrap();
    let db = Database::open(dir.path(), Mode::ReadWrite).unwrap();
    let conn = db.connect().unwrap();

    conn.execute("CREATE NODE TABLE User(id INT64, name STRING, age INT64, PRIMARY KEY(id));")
        .unwrap();
    conn.execute("CREATE REL TABLE KNOWS(FROM User TO User);")
        .unwrap();

    let query = "MATCH (n:User {id: 123})-[:KNOWS]->(f:User) RETURN f.name, f.age;";

    c.bench_function("query_dispatch_overhead", |b| {
        b.iter(|| {
            let res = conn.execute(black_box(query)).unwrap();
            black_box(res);
        })
    });
}

fn bench_graph_operations(c: &mut Criterion) {
    let dir = tempdir().unwrap();
    let db = Database::open(dir.path(), Mode::ReadWrite).unwrap();
    let conn = db.connect().unwrap();

    conn.execute("CREATE NODE TABLE Person(id INT64, name STRING, PRIMARY KEY(id));")
        .unwrap();
    conn.execute("CREATE REL TABLE FOLLOWS(FROM Person TO Person, weight DOUBLE);")
        .unwrap();

    let mut id = std::sync::atomic::AtomicU64::new(0);
    c.bench_function("graph_insert_node", |b| {
        b.iter_batched(
            || {
                let current_id = id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                format!(
                    "CREATE (:Person {{id: {}, name: 'User{}'}});",
                    current_id, current_id
                )
            },
            |query| {
                let _ = conn.execute(&query);
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // Setup for traversal benchmark
    for i in 0..100 {
        let _ = conn.execute(&format!(
            "CREATE (:Person {{id: {}, name: 'Populated{}'}});",
            1000000 + i,
            i
        ));
        if i > 0 {
            let _ = conn.execute(&format!("MATCH (a:Person {{id: {}}}), (b:Person {{id: {}}}) CREATE (a)-[:FOLLOWS {{weight: 1.0}}]->(b);", 1000000 + i - 1, 1000000 + i));
        }
    }

    c.bench_function("graph_traverse_path", |b| {
        b.iter(|| {
            // Traverse 3 hops
            let query = "MATCH (a:Person {id: 1000000})-[:FOLLOWS*1..3]->(b:Person) RETURN b.name;";
            let res = conn.execute(black_box(query)).unwrap();
            black_box(res);
        })
    });
}

criterion_group!(
    benches,
    bench_connection_lifecycle,
    bench_query_dispatch,
    bench_graph_operations
);
criterion_main!(benches);
