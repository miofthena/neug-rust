use neug_rust::{Database, Mode};
use std::sync::Arc;
use std::thread;
use tempfile::tempdir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- NeuG Parallel Query Example ---");
    let dir = tempdir()?;

    // Setup Phase: Create Database and Seed Data
    println!("1. Initializing and Seeding Database...");
    let mut db = Database::open(dir.path(), Mode::ReadWrite)?;

    {
        let mut conn = db.connect()?;
        conn.execute("CREATE NODE TABLE event(id INT64, type STRING, PRIMARY KEY(id));")?;
        for i in 0..100 {
            let query = format!("CREATE (e:event {{id: {}, type: 'log'}});", i);
            conn.execute(&query)?;
        }
    }

    // By closing and re-opening the Database in ReadOnly mode, we simulate
    // an analytical workload where multiple threads query a static graph concurrently.
    db.close();

    println!("2. Reopening in Read-Only Mode for Analytical Queries...");
    let read_db = Arc::new(Database::open(dir.path(), Mode::ReadOnly)?);

    let mut handles = vec![];
    let thread_count = 4;
    let queries_per_thread = 50;

    println!("3. Launching {} Worker Threads...", thread_count);

    for thread_id in 0..thread_count {
        let db_ref = Arc::clone(&read_db);

        let handle = thread::spawn(move || {
            // Each thread opens its own connection for concurrent access
            let conn = db_ref.connect().expect("Failed to connect");

            let mut success_count = 0;
            for _ in 0..queries_per_thread {
                // Perform read-heavy aggregations
                if conn.execute("MATCH (e:event) RETURN count(e);").is_ok() {
                    success_count += 1;
                }
            }

            println!(
                "   > Thread {} completed {} queries.",
                thread_id, success_count
            );
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    println!("--- Parallel Query Example Completed Successfully ---");
    Ok(())
}
