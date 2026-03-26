use neug_rust::{Database, Mode};
use tempfile::tempdir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- NeuG CRUD Operations Example ---");
    let dir = tempdir()?;
    let db = Database::open(dir.path(), Mode::ReadWrite)?;
    let conn = db.connect()?;

    println!("1. Creating Graph Schema...");
    conn.execute("CREATE NODE TABLE user(id INT64, age INT64, name STRING, PRIMARY KEY(id));")?;
    conn.execute("CREATE REL TABLE follows(FROM user TO user, since INT64);")?;

    println!("2. Inserting Nodes (Create)...");
    conn.execute("CREATE (u:user {id: 1, age: 25, name: 'Alice'});")?;
    conn.execute("CREATE (u:user {id: 2, age: 30, name: 'Bob'});")?;
    conn.execute("CREATE (u:user {id: 3, age: 22, name: 'Charlie'});")?;

    println!("3. Creating Relationships (Create)...");
    conn.execute(
        "MATCH (a:user {id: 1}), (b:user {id: 2}) CREATE (a)-[e:follows {since: 2023}]->(b);",
    )?;
    conn.execute(
        "MATCH (a:user {id: 2}), (c:user {id: 3}) CREATE (a)-[e:follows {since: 2024}]->(c);",
    )?;

    println!("4. Querying Data (Read)...");
    let res = conn.execute("MATCH (u:user) RETURN u.name AS Name, u.age AS Age;")?;
    println!("   > Queried all users successfully. Results:\n{}", res);

    let followers = conn.execute("MATCH (a)-[:follows]->(b) RETURN a.name, b.name;")?;
    println!(
        "   > Queried followers graph successfully. Results:\n{}",
        followers
    );

    println!("5. Updating Properties (Update)...");
    conn.execute("MATCH (u:user {id: 1}) SET u.age = 26;")?;
    println!("   > Updated Alice's age.");

    println!("6. Deleting Data (Delete)...");
    conn.execute("MATCH (u:user {id: 3}) DETACH DELETE u;")?;
    println!("   > Deleted Charlie and their relationships.");

    println!("--- CRUD Example Completed Successfully ---");
    Ok(())
}
