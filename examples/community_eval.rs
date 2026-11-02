//! Community quality probe: Leiden (this crate) versus graphqlite louvain on
//! the same database. Modularity is scored on the weighted snapshot graph for
//! both partitions, so only the partitions differ. The last line scores the
//! partition the engine actually serves (`kg::communities`, what `overview` and
//! entity cards show) on the entity-induced subgraph, which is where a hub's
//! membership shows up.
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

fn report(name: &str, csr: &graph::Csr, comm: &[u32], ms: Option<f64>) {
    let q = graph::modularity(csr, comm, 1.0);
    let mut sizes: HashMap<u32, usize> = HashMap::new();
    for &c in comm {
        *sizes.entry(c).or_default() += 1;
    }
    let singletons = sizes.values().filter(|n| **n == 1).count();
    let timing = ms.map(|ms| format!(", {ms:.1} ms")).unwrap_or_default();
    println!(
        "{name}: Q {q:.4}, {} communities, {singletons} singletons{timing}",
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

    report(
        "louvain (graphqlite, projection)",
        &snap.csr,
        &old,
        Some(old_ms),
    );
    report(
        "leiden  (crate, weighted snapshot)",
        &snap.csr,
        &new,
        Some(new_ms),
    );

    // Where entity grouping is visible: the entity-induced subgraph. The flat
    // Leiden partition is the "before" number (it is what an H2-19-less build
    // serves), the served partition is what `overview` shows now.
    let (csr, entities) = kg::entity_subgraph(&snap);
    let comm: Vec<u32> = entities
        .iter()
        .map(|e| new[snap.ent_node[e] as usize])
        .collect();
    report("flat    (leiden, entity subgraph)", &csr, &comm, None);
    let t = Instant::now();
    let served = kg::community_quality(&db)?;
    let served_ms = t.elapsed().as_secs_f64() * 1000.0;
    println!(
        "served  (kg::communities, entity subgraph): Q {:.4}, {} communities, {} singletons, {served_ms:.1} ms",
        served.modularity, served.communities, served.singletons
    );
    Ok(())
}
