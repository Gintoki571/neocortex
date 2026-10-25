//! Community quality probe: Leiden (this crate) versus graphqlite louvain on
//! the same database. Modularity is scored on the weighted snapshot graph for
//! both partitions, so only the partitions differ.
//!
//!   cargo run --release --example community_eval -- <db>

use neocortex::{db::Db, graph, kg};
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

fn louvain(db: &Db, snap: &kg::Snapshot) -> neocortex::Result<Vec<u32>> {
    let result = db.graph().cypher("RETURN louvain(1.0)")?;
    let mut node_of: HashMap<String, u32> = HashMap::new();
    for (&id, &node) in &snap.ent_node {
        node_of.insert(format!("e:{id}"), node);
    }
    for (&id, &node) in &snap.mem_node {
        node_of.insert(format!("m:{id}"), node);
    }
    let mut rows: Vec<(String, i64)> = Vec::new();
    for row in &result {
        // graphqlite returns one row whose `column_0` is an array of
        // {node_id, user_id, community} objects.
        if let Some(graphqlite::Value::Array(items)) = row.get_value("column_0") {
            for item in items {
                let graphqlite::Value::Object(fields) = item else {
                    continue;
                };
                let user = match fields.get("user_id") {
                    Some(graphqlite::Value::String(s)) => s.clone(),
                    Some(graphqlite::Value::Integer(i)) => i.to_string(),
                    _ => continue,
                };
                let community = match fields.get("community") {
                    Some(graphqlite::Value::Integer(i)) => *i,
                    Some(graphqlite::Value::Float(f)) => *f as i64,
                    _ => continue,
                };
                rows.push((user, community));
            }
        } else if let (Ok(user), Ok(community)) =
            (row.get::<String>("user_id"), row.get::<i64>("community"))
        {
            rows.push((user, community));
        }
    }
    let mut dense: HashMap<i64, u32> = HashMap::new();
    let mut next = 0u32;
    let mut comm = vec![u32::MAX; snap.csr.len()];
    for (user, community) in rows {
        let Some(&node) = node_of.get(&user) else {
            continue;
        };
        let id = *dense.entry(community).or_insert_with(|| {
            let id = next;
            next += 1;
            id
        });
        comm[node as usize] = id;
    }
    // Nodes the projection does not contain stay singletons.
    for c in &mut comm {
        if *c == u32::MAX {
            *c = next;
            next += 1;
        }
    }
    Ok(comm)
}

fn report(name: &str, csr: &graph::Csr, comm: &[u32], ms: f64) {
    let q = graph::modularity(csr, comm, 1.0);
    let mut sizes: HashMap<u32, usize> = HashMap::new();
    for &c in comm {
        *sizes.entry(c).or_default() += 1;
    }
    let singletons = sizes.values().filter(|n| **n == 1).count();
    println!(
        "{name}: Q {q:.4}, {} communities, {singletons} singletons, {ms:.1} ms",
        sizes.len()
    );
}

fn main() -> neocortex::Result<()> {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: community_eval <db>");
        return Ok(());
    };
    let db = Db::open(Path::new(&path))?;
    let snap = kg::snapshot(&db, None, false)?;
    println!(
        "nodes {} edges {}",
        snap.csr.len(),
        snap.csr.targets.len() / 2
    );

    let t = Instant::now();
    let old = louvain(&db, &snap)?;
    let old_ms = t.elapsed().as_secs_f64() * 1000.0;

    let t = Instant::now();
    let new = graph::leiden(&snap.csr, 7, 1.0);
    let new_ms = t.elapsed().as_secs_f64() * 1000.0;

    report("louvain (graphqlite, projection)", &snap.csr, &old, old_ms);
    report(
        "leiden  (crate, weighted snapshot)",
        &snap.csr,
        &new,
        new_ms,
    );
    Ok(())
}
