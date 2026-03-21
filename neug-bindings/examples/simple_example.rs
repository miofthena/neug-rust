use neug_rust::{Database, Mode};
use std::env;
use std::process;
use tempfile::tempdir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 3 {
        println!("Usage: cargo run --example simple_example <csv_data_dir> <db_dir>");
        // For cargo test runner not to fail if run without args:
        if args.len() == 1 {
            println!("Running default in-memory example to avoid failure...");
            return run_default_example();
        }
        process::exit(1);
    }
    let data_dir = &args[1];
    let db_dir = &args[2];

    println!("Loading data from {} into database {}", data_dir, db_dir);

    let person_csv = format!("{}/person.csv", data_dir);
    let person_knows_person_csv = format!("{}/person_knows_person.csv", data_dir);

    let mut db = Database::open(db_dir, Mode::ReadWrite)?;
    let mut conn = db.connect()?;

    // First create the graph schema
    conn.execute("CREATE NODE TABLE person(id INT64, name STRING, age INT64, PRIMARY KEY(id));")?;
    conn.execute("CREATE REL TABLE knows(FROM person TO person, weight DOUBLE);")?;

    // Then load data.
    conn.execute(&format!("COPY person from \"{}\"", person_csv))?;
    conn.execute(&format!(
        "COPY knows from \"{}\" (from=\"person\", to=\"person\")",
        person_knows_person_csv
    ))?;

    let _res = conn.execute("MATCH (n)-[e]-(m) return count(e);")?;
    // Iterate over res here:
    // for record in res { println!("{:?}", record); }
    println!("Query executed successfully");

    db.close();

    Ok(())
}

fn run_default_example() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;

    println!("Running with temporary database at {:?}", dir.path());
    let mut db = Database::open(dir.path(), Mode::ReadWrite)?;
    let mut conn = db.connect()?;
    conn.execute("CREATE NODE TABLE person(id INT64, name STRING, age INT64, PRIMARY KEY(id));")?;
    conn.execute("CREATE REL TABLE knows(FROM person TO person, weight DOUBLE);")?;

    let _res = conn.execute("MATCH (n)-[e]-(m) return count(e);")?;
    println!("Queries executed successfully.");

    Ok(())
}
