use criterion::{Criterion, criterion_group, criterion_main};
use neug_rust::{Database, Mode};
use tempfile::tempdir;

fn bench_database_connection(c: &mut Criterion) {
    let dir = tempdir().unwrap();
    let db = Database::open(dir.path(), Mode::ReadWrite).unwrap();

    c.bench_function("connect", |b| {
        b.iter(|| {
            let mut conn = db.connect().unwrap();
            // Just opening and closing a connection
            conn.close();
        })
    });
}

fn bench_simple_query(c: &mut Criterion) {
    let dir = tempdir().unwrap();
    let mut db = Database::open(dir.path(), Mode::ReadWrite).unwrap();

    // Setup schema and some data for the query benchmark
    {
        let mut conn = db.connect().unwrap();
        conn.execute("CREATE NODE TABLE person(id INT64, name STRING, PRIMARY KEY(id));")
            .unwrap();
        // Insert a few nodes
        for i in 0..100 {
            let query = format!("CREATE (p: person {{id: {}, name: 'Node{}'}});", i, i);
            conn.execute(&query).unwrap();
        }
    }

    c.bench_function("simple_match_query", |b| {
        let conn = db.connect().unwrap();
        b.iter(|| {
            // A simple query to count nodes
            let _res = conn.execute("MATCH (n:person) RETURN count(n);").unwrap();
        })
    });
}

criterion_group!(benches, bench_database_connection, bench_simple_query);
criterion_main!(benches);
