use criterion::{black_box, criterion_group, criterion_main, Criterion};
use neug_rust::{AccessMode, Database, Mode};
use std::collections::HashMap;
use tempfile::tempdir;

fn bench_connection_lifecycle(c: &mut Criterion) {
    let dir = tempdir().unwrap();
    let db = Database::open(dir.path(), Mode::ReadOnly).unwrap();

    c.bench_function("connection_lifecycle", |b| {
        // black_box prevents the compiler from optimizing away the loop
        // even if the inner functions currently lack side effects.
        b.iter(|| {
            let mut conn = db.connect().unwrap();
            black_box(conn.is_open());
            conn.close();
        })
    });
}

fn bench_query_dispatch(c: &mut Criterion) {
    let dir = tempdir().unwrap();
    let mut db = Database::open(dir.path(), Mode::ReadWrite).unwrap();
    let conn = db.connect().unwrap();

    // Setup schema
    conn.execute("CREATE NODE TABLE User(id INT64, name STRING, age INT64, PRIMARY KEY(id));").unwrap();
    conn.execute("CREATE REL TABLE KNOWS(FROM User TO User);").unwrap();

    // Benchmarking the overhead of string passing and FFI boundary preparation
    let query = "MATCH (n:User {id: 123})-[:KNOWS]->(f:User) RETURN f.name, f.age;";

    c.bench_function("query_dispatch_overhead", |b| {
        b.iter(|| {
            // black_box forces the compiler to treat the query and result as unknown,
            // measuring the actual allocation and dispatch overhead of the Rust wrapper.
            let res = conn.execute(black_box(query)).unwrap();
            black_box(res);
        })
    });
}

criterion_group!(
    benches,
    bench_connection_lifecycle,
    bench_query_dispatch
);
criterion_main!(benches);
