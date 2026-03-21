use criterion::{black_box, criterion_group, criterion_main, Criterion};
use neug_rust::{AccessMode, Database, Mode};
use std::collections::HashMap;
use tempfile::tempdir;

fn bench_connection_lifecycle(c: &mut Criterion) {
    let dir = tempdir().unwrap();
    let db = Database::open(dir.path(), Mode::ReadWrite).unwrap();

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

fn bench_parameterized_query(c: &mut Criterion) {
    let dir = tempdir().unwrap();
    let mut db = Database::open(dir.path(), Mode::ReadWrite).unwrap();
    let conn = db.connect().unwrap();

    let query = "MATCH (n:User {id: $user_id}) RETURN n;";

    c.bench_function("parameterized_query_overhead", |b| {
        b.iter(|| {
            // Simulating real-world scenario: allocating a HashMap for parameters
            // and passing it through the API boundary.
            let mut params = HashMap::new();
            params.insert("user_id".to_string(), "9999".to_string());
            
            let res = conn.execute_with_options(
                black_box(query),
                Some(AccessMode::Read),
                Some(&params),
            ).unwrap();
            
            black_box(res);
        })
    });
}

fn bench_batch_insertion(c: &mut Criterion) {
    let dir = tempdir().unwrap();
    let mut db = Database::open(dir.path(), Mode::ReadWrite).unwrap();
    let conn = db.connect().unwrap();

    c.bench_function("batch_query_generation", |b| {
        b.iter(|| {
            // In real workloads, generating the query strings dynamically
            // is often a bottleneck before hitting the DB engine.
            let mut batch = String::with_capacity(1024);
            for i in 0..100 {
                batch.push_str(&format!("CREATE (u:User {{id: {}, name: 'User{}'}});\n", i, i));
            }
            let res = conn.execute(black_box(&batch)).unwrap();
            black_box(res);
        })
    });
}

criterion_group!(
    benches,
    bench_connection_lifecycle,
    bench_query_dispatch,
    bench_parameterized_query,
    bench_batch_insertion
);
criterion_main!(benches);
