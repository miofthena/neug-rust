use neug_rust::{Database, Mode};
use rand::Rng;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tempfile::tempdir;

const NUM_PERSONS: i64 = 2_000;
const NUM_MESSAGES: i64 = 5_000;
const AVG_FRIENDS: i64 = 5;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Silence C++ glog spam from the neug engine globally for the benchmark
    unsafe {
        let dev_null = libc::open(b"/dev/null\0".as_ptr() as *const _, libc::O_WRONLY);
        if dev_null != -1 {
            libc::dup2(dev_null, libc::STDERR_FILENO);
            libc::close(dev_null);
        }
    }

    println!("=== LDBC SNB Interactive Macro-Benchmark ===");
    println!("Simulating a social network graph...");

    let dir = tempdir()?;
    let db_path = dir.path();

    // ==========================================
    // PHASE 1: DATA GENERATION & INGESTION
    // ==========================================
    println!("\n[Phase 1] Database Ingestion (OLTP Write)");
    let load_start = Instant::now();

    {
        // 1. Open Database in ReadWrite mode
        let mut db = Database::open(db_path, Mode::ReadWrite)?;
        let mut conn = db.connect()?;

        // 2. Create Schema
        println!("  -> Creating Schema...");
        conn.execute("CREATE NODE TABLE Person(id INT64, firstName STRING, lastName STRING, PRIMARY KEY(id));")?;
        conn.execute("CREATE NODE TABLE Message(id INT64, content STRING, PRIMARY KEY(id));")?;
        conn.execute("CREATE REL TABLE KNOWS(FROM Person TO Person, creationDate INT64);")?;
        conn.execute("CREATE REL TABLE HAS_CREATOR(FROM Message TO Person);")?;

        let mut rng = rand::thread_rng();

        // 3. Insert Persons
        println!("  -> Inserting {} Persons...", NUM_PERSONS);
        for id in 0..NUM_PERSONS {
            let query = format!(
                "CREATE (p:Person {{id: {}, firstName: 'John{}', lastName: 'Doe{}'}});",
                id, id, id
            );
            conn.execute(&query)?;
        }

        // 4. Insert Messages
        println!("  -> Inserting {} Messages...", NUM_MESSAGES);
        for id in 0..NUM_MESSAGES {
            let query = format!(
                "CREATE (m:Message {{id: {}, content: 'Hello World {}'}});",
                id, id
            );
            conn.execute(&query)?;
        }

        // 5. Insert KNOWS relationships (random network)
        println!(
            "  -> Generating KNOWS graph (~{} edges)...",
            NUM_PERSONS * AVG_FRIENDS
        );
        let mut edges_inserted = 0;
        for src in 0..NUM_PERSONS {
            for _ in 0..AVG_FRIENDS {
                let dst = rng.gen_range(0..NUM_PERSONS);
                if src != dst {
                    let query = format!(
                        "MATCH (a:Person {{id: {}}}), (b:Person {{id: {}}}) CREATE (a)-[:KNOWS {{creationDate: 2026}}]->(b);",
                        src, dst
                    );
                    if conn.execute(&query).is_ok() {
                        edges_inserted += 1;
                    }
                }
            }
        }

        // 6. Insert HAS_CREATOR relationships (1 per message)
        println!("  -> Generating HAS_CREATOR graph...");
        for msg_id in 0..NUM_MESSAGES {
            let creator_id = rng.gen_range(0..NUM_PERSONS);
            let query = format!(
                "MATCH (m:Message {{id: {}}}), (p:Person {{id: {}}}) CREATE (m)-[:HAS_CREATOR]->(p);",
                msg_id, creator_id
            );
            conn.execute(&query)?;
        }

        println!("  -> Ingestion completed in {:.2?}", load_start.elapsed());
        println!("     Total nodes: {}", NUM_PERSONS + NUM_MESSAGES);
        println!("     Total edges: {}", edges_inserted + NUM_MESSAGES);

        // Ensure data is flushed to disk before reopening
        db.close();
    }

    // ==========================================
    // PHASE 2: CONCURRENT QUERY EXECUTION
    // ==========================================
    println!("\n[Phase 2] High-Concurrency Query Execution (LDBC Workload)");

    let read_db = Arc::new(Database::open(db_path, Mode::ReadOnly)?);

    let thread_count = 8;
    let queries_per_thread = 500;
    println!("  -> Launching {} concurrent threads...", thread_count);
    println!(
        "  -> Executing {} queries per thread...",
        queries_per_thread
    );

    let query_start = Instant::now();
    let mut handles = vec![];

    for t_id in 0..thread_count {
        let db_ref = Arc::clone(&read_db);

        let handle = thread::spawn(move || {
            let mut rng = rand::thread_rng();
            // Each thread maintains its own connection state machine inside the C++ engine
            let conn = db_ref.connect().expect("Failed to connect");

            let mut total_latency = Duration::new(0, 0);

            for _ in 0..queries_per_thread {
                let person_id = rng.gen_range(0..NUM_PERSONS);

                // LDBC SNB Interactive Query 2 Variant:
                // "Given a start Person, find the recent Messages created by their friends"
                // This is a 2-hop traversal query testing graph locality and JOIN performance.
                let query = format!(
                    "MATCH (p:Person {{id: {}}})-[:KNOWS]->(friend:Person)<-[:HAS_CREATOR]-(msg:Message) RETURN msg.content LIMIT 20;",
                    person_id
                );

                let start = Instant::now();
                // We use black_box in benchmarks to prevent compiler optimizations,
                // but since this is an example binary, executing the string is enough.
                let res = conn.execute(&query);
                total_latency += start.elapsed();

                if res.is_err() {
                    println!("Query failed: {:?}", res.err());
                }
            }

            total_latency
        });

        handles.push((t_id, handle));
    }

    let mut cumulative_thread_time = Duration::new(0, 0);
    for (t_id, handle) in handles {
        let thread_time = handle.join().unwrap();
        cumulative_thread_time += thread_time;
        // println!("     Thread {} completed in {:.2?}", t_id, thread_time);
    }

    let total_wall_time = query_start.elapsed();
    let total_queries = thread_count * queries_per_thread;
    let avg_latency = cumulative_thread_time / total_queries as u32;
    let qps = (total_queries as f64) / total_wall_time.as_secs_f64();

    println!("\n=== Benchmark Results ===");
    println!("Total Wall-clock Time:  {:.2?}", total_wall_time);
    println!("Total Queries Executed: {}", total_queries);
    println!("Average Query Latency:  {:.2?}", avg_latency);
    println!("Throughput (QPS):       {:.2} queries/sec", qps);

    Ok(())
}
