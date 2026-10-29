use crate::db::Db;
use crate::error::Result;
use crate::graph::Csr;
use rusqlite::params;
use std::collections::{HashMap, HashSet};

pub struct Snapshot {
    pub csr: Csr,
    pub node_kind: Vec<(char, i64)>,
    pub mem_node: HashMap<i64, u32>,
    pub ent_node: HashMap<i64, u32>,
}

fn run(db: &Db, q: &str, params: &[(&str, serde_json::Value)]) -> Result<()> {
    let mut b = db.graph().cypher_builder(q);
    for (k, v) in params {
        b = b.param(k, v.clone());
    }
    b.run()?;
    Ok(())
}

/// First words of a memory's text: the readable `name` a Cypher query shows
/// (`MATCH (m:Memory) RETURN m.name`). Full text stays relational and in FTS.
fn memory_name(content: &str) -> String {
    let mut name: String = content
        .split_whitespace()
        .take(8)
        .collect::<Vec<_>>()
        .join(" ");
    if name.chars().count() > 80 {
        name = name.chars().take(79).collect::<String>() + "…";
    }
    name
}

pub fn project_memory(db: &Db, id: i64, kind: &str, content: &str) -> Result<()> {
    run(
        db,
        "MERGE (n:Memory {id:$id}) SET n.kind=$kind, n.name=$name",
        &[
            ("id", format!("m:{id}").into()),
            ("kind", kind.into()),
            ("name", memory_name(content).into()),
        ],
    )
}

pub fn project_entity(db: &Db, id: i64, name: &str, kind: &str) -> Result<()> {
    run(
        db,
        "MERGE (n:Entity {id:$id}) SET n.name=$name, n.type=$kind",
        &[
            ("id", format!("e:{id}").into()),
            ("name", name.into()),
            ("kind", kind.into()),
        ],
    )
}

/// Project an entity using its stored name/type (so a bare mention never
/// clobbers an existing type).
pub fn project_entity_from_db(db: &Db, id: i64) -> Result<()> {
    let (name, kind): (String, String) =
        db.sql()
            .query_row("SELECT name,type FROM entities WHERE id=?1", [id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?;
    project_entity(db, id, &name, &kind)
}

/// Advance the write generation used to invalidate caches.
pub fn bump_gen(db: &Db) -> Result<()> {
    db.sql().execute(
        "INSERT INTO meta(key,value) VALUES('gen','1')
         ON CONFLICT(key) DO UPDATE SET value=CAST(CAST(value AS INTEGER)+1 AS TEXT)",
        [],
    )?;
    Ok(())
}

pub fn project_mention(db: &Db, memory: i64, entity: i64) -> Result<()> {
    // One MERGE statement: the two-`MATCH` form made graphqlite plan a
    // cartesian product and cost O(edges) per call on a grown graph.
    run(
        db,
        "MERGE (m:Memory {id:$m})-[:MENTIONS]->(e:Entity {id:$e})",
        &[
            ("m", format!("m:{memory}").into()),
            ("e", format!("e:{entity}").into()),
        ],
    )
}

pub fn project_similar(db: &Db, a: i64, b: i64, score: f32) -> Result<()> {
    run(
        db,
        "MERGE (a:Entity {id:$a})-[r:SIMILAR]->(b:Entity {id:$b}) SET r.score=$score",
        &[
            ("a", format!("e:{a}").into()),
            ("b", format!("e:{b}").into()),
            ("score", f64::from(score).into()),
        ],
    )
}

/// B5: a name-containment edge ("Cascais marina" contains "Cascais"), the
/// weak hop the semantic beam and PPR walk.
pub fn project_name_part(db: &Db, a: i64, b: i64, weight: f32) -> Result<()> {
    run(
        db,
        "MERGE (a:Entity {id:$a})-[r:CONTAINS]->(b:Entity {id:$b}) SET r.weight=$weight",
        &[
            ("a", format!("e:{a}").into()),
            ("b", format!("e:{b}").into()),
            ("weight", f64::from(weight).into()),
        ],
    )
}

/// Project the containment edges of one entity. The caller must have
/// projected the entity's node first: the edge MERGE matches both endpoints,
/// and the write path links a new entity before its node exists.
pub fn project_name_parts(db: &Db, entity: i64) -> Result<()> {
    let rows: Vec<(i64, i64, f32)> = {
        let mut st = db
            .sql()
            .prepare("SELECT a,b,weight FROM name_part WHERE a=?1 OR b=?1")?;
        let rows = st.query_map([entity], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    for (a, b, weight) in rows {
        project_name_part(db, a, b, weight)?;
    }
    Ok(())
}

pub fn project_relation(db: &Db, id: i64, src: i64, dst: i64, predicate: &str) -> Result<()> {
    run(db, "MERGE (a:Entity {id:$a})-[r:REL {rid:$rid}]->(b:Entity {id:$b}) SET r.predicate=$predicate", &[
        ("a", format!("e:{src}").into()), ("b", format!("e:{dst}").into()),
        ("rid", id.into()), ("predicate", predicate.into()),
    ])
}

pub fn remove_memory(db: &Db, id: i64) -> Result<()> {
    run(
        db,
        "MATCH (n:Memory {id:$id}) DETACH DELETE n",
        &[("id", format!("m:{id}").into())],
    )
}

pub fn remove_entity(db: &Db, id: i64) -> Result<()> {
    run(
        db,
        "MATCH (n:Entity {id:$id}) DETACH DELETE n",
        &[("id", format!("e:{id}").into())],
    )
}

/// Delete one REL edge by its relation id, leaving both entities in place.
pub fn remove_relation(db: &Db, id: i64) -> Result<()> {
    run(
        db,
        "MATCH (a)-[r:REL {rid:$rid}]->(b) DELETE r",
        &[("rid", id.into())],
    )
}

/// Re-project the edges and the entity node owned by `entity` after a merge:
/// the surviving node keeps its mentions and the relations that were moved
/// onto it. Nothing else in the projection is touched.
pub fn reproject_entity(db: &Db, entity: i64) -> Result<()> {
    project_entity_from_db(db, entity)?;
    let mentions: Vec<i64> = {
        let mut st = db
            .sql()
            .prepare("SELECT memory_id FROM mentions WHERE entity_id=?1")?;
        let rows = st.query_map([entity], |r| r.get(0))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    for memory in mentions {
        project_mention(db, memory, entity)?;
    }
    let similar: Vec<(i64, i64, f32)> = {
        let mut st = db
            .sql()
            .prepare("SELECT a,b,score FROM similar WHERE a=?1 OR b=?1")?;
        let rows = st.query_map([entity], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    for (a, b, score) in similar {
        project_similar(db, a, b, score)?;
    }
    project_name_parts(db, entity)?;
    let relations: Vec<(i64, i64, i64, String)> = {
        let mut st = db.sql().prepare(
            "SELECT r.id,r.src,r.dst,p.canonical FROM relations r
             JOIN predicates p ON p.id=r.predicate_id
             WHERE r.src=?1 OR r.dst=?1",
        )?;
        let rows = st.query_map([entity], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    for (id, src, dst, predicate) in relations {
        project_relation(db, id, src, dst, &predicate)?;
    }
    Ok(())
}

pub fn rebuild_all(db: &Db) -> Result<()> {
    db.graph().cypher("MATCH (n) DETACH DELETE n")?;
    {
        let mut st = db.sql().prepare("SELECT id,kind,content FROM memories")?;
        let rows: Vec<(i64, String, String)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (id, kind, content) in rows {
            project_memory(db, id, &kind, &content)?;
        }
    }
    {
        let mut st = db.sql().prepare("SELECT id,name,type FROM entities")?;
        let rows: Vec<(i64, String, String)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (id, name, kind) in rows {
            project_entity(db, id, &name, &kind)?;
        }
    }
    {
        let mut st = db
            .sql()
            .prepare("SELECT memory_id,entity_id FROM mentions")?;
        let rows: Vec<(i64, i64)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (m, e) in rows {
            project_mention(db, m, e)?;
        }
    }
    {
        let mut st = db.sql().prepare("SELECT a,b,score FROM similar")?;
        let rows: Vec<(i64, i64, f32)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (a, b, s) in rows {
            project_similar(db, a, b, s)?;
        }
    }
    {
        let mut st = db.sql().prepare("SELECT a,b,weight FROM name_part")?;
        let rows: Vec<(i64, i64, f32)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (a, b, w) in rows {
            project_name_part(db, a, b, w)?;
        }
    }
    {
        let mut st = db.sql().prepare("SELECT r.id,r.src,r.dst,p.canonical FROM relations r JOIN predicates p ON p.id=r.predicate_id")?;
        let rows: Vec<(i64, i64, i64, String)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (id, src, dst, p) in rows {
            project_relation(db, id, src, dst, &p)?;
        }
    }
    Ok(())
}

pub fn snapshot(db: &Db, as_of: Option<i64>, include_ended: bool) -> Result<Snapshot> {
    snapshot_with(db, as_of, include_ended, true)
}

/// `name_parts` = false leaves the B5 containment edges out of the snapshot.
pub fn snapshot_with(
    db: &Db,
    as_of: Option<i64>,
    include_ended: bool,
    name_parts: bool,
) -> Result<Snapshot> {
    build_snapshot(db, as_of, include_ended, name_parts, false)
}

/// `selective_mentions` weights every mention edge by
/// `1/sqrt(deg(memory) * deg(entity))` instead of 1: a mention counts by how
/// selective it is, so a hub that hundreds of memories mention stops
/// out-weighing the entity-entity relations that say what it is. Used by
/// community detection only (PPR keeps the flat snapshot).
fn build_snapshot(
    db: &Db,
    as_of: Option<i64>,
    include_ended: bool,
    name_parts: bool,
    selective_mentions: bool,
) -> Result<Snapshot> {
    let mut nodes = Vec::new();
    let mut mem_node = HashMap::new();
    let mut ent_node = HashMap::new();
    {
        // Forgotten memories (expired without a superseding memory) are not
        // part of the graph. Superseded memories are history and stay only
        // when the caller asks for ended nodes.
        let sql = if include_ended {
            "SELECT id FROM memories WHERE expired_at IS NULL OR superseded_by IS NOT NULL ORDER BY id"
        } else {
            "SELECT id FROM memories WHERE expired_at IS NULL ORDER BY id"
        };
        let mut st = db.sql().prepare(sql)?;
        for row in st.query_map([], |r| r.get::<_, i64>(0))? {
            let id = row?;
            mem_node.insert(id, nodes.len() as u32);
            nodes.push(('m', id));
        }
    }
    {
        let mut st = db.sql().prepare("SELECT id FROM entities ORDER BY id")?;
        for row in st.query_map([], |r| r.get::<_, i64>(0))? {
            let id = row?;
            ent_node.insert(id, nodes.len() as u32);
            nodes.push(('e', id));
        }
    }
    let mut edges = Vec::new();
    let mut ments: Vec<(u32, u32)> = Vec::new();
    let mut st = db
        .sql()
        .prepare("SELECT memory_id,entity_id FROM mentions")?;
    for row in st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
        let (m, e) = row?;
        if let (Some(&a), Some(&b)) = (mem_node.get(&m), ent_node.get(&e)) {
            ments.push((a, b));
        }
    }
    if selective_mentions {
        let mut mem_deg: HashMap<u32, f32> = HashMap::new();
        let mut ent_deg: HashMap<u32, f32> = HashMap::new();
        for &(a, b) in &ments {
            *mem_deg.entry(a).or_insert(0.0) += 1.0;
            *ent_deg.entry(b).or_insert(0.0) += 1.0;
        }
        for &(a, b) in &ments {
            let w = 1.0 / (mem_deg[&a] * ent_deg[&b]).sqrt();
            edges.push((a, b, w));
        }
    } else {
        for &(a, b) in &ments {
            edges.push((a, b, 1.0));
        }
    }
    if let Some(at) = as_of {
        let mut st = db.sql().prepare(
            "SELECT src,dst,evidence_count FROM relations
             WHERE valid_from<=?1 AND (valid_to IS NULL OR valid_to>?1) AND expired_at IS NULL",
        )?;
        let mut rows = st.query(params![at])?;
        while let Some(r) = rows.next()? {
            let (s, d, c): (i64, i64, i64) = (r.get(0)?, r.get(1)?, r.get(2)?);
            if let (Some(&a), Some(&b)) = (ent_node.get(&s), ent_node.get(&d)) {
                edges.push((a, b, (1.0 + c as f64).ln().max(0.5) as f32));
            }
        }
    } else if include_ended {
        let mut st = db
            .sql()
            .prepare("SELECT src,dst,evidence_count FROM relations")?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let (s, d, c): (i64, i64, i64) = (r.get(0)?, r.get(1)?, r.get(2)?);
            if let (Some(&a), Some(&b)) = (ent_node.get(&s), ent_node.get(&d)) {
                edges.push((a, b, (1.0 + c as f64).ln().max(0.5) as f32));
            }
        }
    } else {
        // No clock: "currently valid" is an SQL predicate, not a timestamp.
        let mut st = db.sql().prepare(
            "SELECT src,dst,evidence_count FROM relations
             WHERE valid_to IS NULL AND expired_at IS NULL",
        )?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let (s, d, c): (i64, i64, i64) = (r.get(0)?, r.get(1)?, r.get(2)?);
            if let (Some(&a), Some(&b)) = (ent_node.get(&s), ent_node.get(&d)) {
                edges.push((a, b, (1.0 + c as f64).ln().max(0.5) as f32));
            }
        }
    }
    let mut st = db.sql().prepare("SELECT a,b,score FROM similar")?;
    for row in st.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, f32>(2)?,
        ))
    })? {
        let (a, b, s) = row?;
        if let (Some(&u), Some(&v)) = (ent_node.get(&a), ent_node.get(&b)) {
            edges.push((u, v, s));
        }
    }
    if name_parts {
        let mut st = db.sql().prepare("SELECT a,b,weight FROM name_part")?;
        for row in st.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, f32>(2)?,
            ))
        })? {
            let (a, b, w) = row?;
            if let (Some(&u), Some(&v)) = (ent_node.get(&a), ent_node.get(&b)) {
                edges.push((u, v, w));
            }
        }
    }
    Ok(Snapshot {
        csr: Csr::from_edges(nodes.len(), &edges),
        node_kind: nodes,
        mem_node,
        ent_node,
    })
}

/// Fixed seed for community detection: the same graph always yields the same
/// partition, so ids only change when the graph does.
const COMMUNITY_SEED: u64 = 7;

/// H2-19: modularity isolates a hub. On the project scenario the hub carries 53
/// mention edges against 6.2 of relation weight, so the best cut puts it alone
/// in a community labelled by itself while its facts sit in other communities.
/// A one-entity community groups nothing: attach it to the community that holds
/// most of that entity's live relations (ties: more evidence, then the lowest
/// community id). Entities without a live relation stay where they are; the
/// pass is deterministic and leaves Leiden's partition otherwise untouched.
fn attach_singletons(db: &Db, snap: &Snapshot, partition: &mut [u32]) -> Result<()> {
    let mut live: HashMap<i64, Vec<(i64, i64)>> = HashMap::new();
    {
        let mut st = db.sql().prepare(
            "SELECT src,dst,evidence_count FROM relations
             WHERE valid_to IS NULL AND expired_at IS NULL",
        )?;
        for row in st.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })? {
            let (src, dst, evidence) = row?;
            if src == dst {
                continue;
            }
            live.entry(src).or_default().push((dst, evidence));
            live.entry(dst).or_default().push((src, evidence));
        }
    }
    let mut members: HashMap<u32, usize> = HashMap::new();
    for &node in snap.ent_node.values() {
        *members.entry(partition[node as usize]).or_insert(0) += 1;
    }
    let mut entities: Vec<(i64, u32)> = snap.ent_node.iter().map(|(&e, &n)| (e, n)).collect();
    entities.sort_unstable_by_key(|&(entity, _)| entity);
    for (entity, node) in entities {
        let own = partition[node as usize];
        if members.get(&own).copied().unwrap_or(0) != 1 {
            continue;
        }
        let Some(neighbours) = live.get(&entity) else {
            continue;
        };
        let mut score: HashMap<u32, (usize, i64)> = HashMap::new();
        for &(other, evidence) in neighbours {
            let Some(&onode) = snap.ent_node.get(&other) else {
                continue;
            };
            let community = partition[onode as usize];
            if community == own {
                continue;
            }
            let entry = score.entry(community).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += evidence;
        }
        let best = score.into_iter().max_by(|a, b| {
            a.1 .0
                .cmp(&b.1 .0)
                .then_with(|| a.1 .1.cmp(&b.1 .1))
                .then_with(|| b.0.cmp(&a.0))
        });
        let Some((target, _)) = best else {
            continue;
        };
        partition[node as usize] = target;
        members.insert(own, 0);
        *members.entry(target).or_insert(0) += 1;
    }
    Ok(())
}

pub fn communities(db: &Db) -> Result<HashMap<i64, i64>> {
    let snap = build_snapshot(db, None, false, true, true)?;
    let mut partition = crate::graph::leiden(&snap.csr, COMMUNITY_SEED, 1.0);
    attach_singletons(db, &snap, &mut partition)?;
    let mut current: HashMap<i64, u32> = HashMap::with_capacity(snap.ent_node.len());
    for (&entity, &node) in &snap.ent_node {
        current.insert(entity, partition[node as usize]);
    }

    // Keep ids stable across runs: match each new community to the previous
    // community it shares the most entities with (greedy, one-to-one). A
    // community with no meaningful overlap gets a fresh id above all old ones.
    let mut previous: HashMap<i64, i64> = HashMap::new();
    {
        let mut st = db
            .sql()
            .prepare("SELECT entity_id,community FROM communities")?;
        for row in st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
            let (entity, community) = row?;
            previous.insert(entity, community);
        }
    }
    let mut overlap: HashMap<(i64, u32), usize> = HashMap::new();
    for (&entity, &community) in &current {
        if let Some(&old) = previous.get(&entity) {
            *overlap.entry((old, community)).or_insert(0) += 1;
        }
    }
    let mut pairs: Vec<(usize, i64, u32)> = overlap
        .into_iter()
        .map(|((old, new), shared)| (shared, old, new))
        .collect();
    pairs.sort_unstable_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.cmp(&b.2))
    });
    let mut assigned: HashMap<u32, i64> = HashMap::new();
    let mut used: HashSet<i64> = HashSet::new();
    for (_, old, new) in pairs {
        if assigned.contains_key(&new) || used.contains(&old) {
            continue;
        }
        assigned.insert(new, old);
        used.insert(old);
    }
    let first_fresh = previous.values().copied().max().map_or(0, |m| m + 1);
    let mut fresh: Vec<u32> = current
        .values()
        .copied()
        .filter(|c| !assigned.contains_key(c))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    fresh.sort_unstable();
    for (next, community) in (first_fresh..).zip(fresh) {
        assigned.insert(community, next);
    }

    let mut out: HashMap<i64, i64> = HashMap::with_capacity(current.len());
    for (entity, community) in &current {
        out.insert(*entity, assigned[community]);
    }
    let now = crate::temporal::Clock::system().now;
    db.sql().execute("DELETE FROM communities", [])?;
    for (id, community) in &out {
        db.sql().execute(
            "INSERT OR REPLACE INTO communities(entity_id,community,computed_at) VALUES (?1,?2,?3)",
            params![id, community, now],
        )?;
    }
    db.sql().execute(
        "INSERT OR REPLACE INTO meta(key,value)
         VALUES('communities_gen', COALESCE((SELECT value FROM meta WHERE key='gen'),'0'))",
        [],
    )?;
    Ok(out)
}

/// The entity-induced subgraph of a snapshot: entities only, weighted by the
/// edges between them. Memory nodes exist so that a hub's mentions can pull it
/// into a community; they say nothing about how the entities themselves group,
/// so modularity is scored here. Returns the CSR and the entity id of each node.
pub fn entity_subgraph(snap: &Snapshot) -> (Csr, Vec<i64>) {
    let mut entities: Vec<i64> = snap.ent_node.keys().copied().collect();
    entities.sort_unstable();
    let index: HashMap<i64, u32> = entities
        .iter()
        .enumerate()
        .map(|(i, &e)| (e, i as u32))
        .collect();
    let mut edges = Vec::new();
    for &entity in &entities {
        let node = snap.ent_node[&entity] as usize;
        for k in snap.csr.offsets[node]..snap.csr.offsets[node + 1] {
            let other = snap.csr.targets[k];
            let ('e', other_id) = snap.node_kind[other as usize] else {
                continue;
            };
            // Each edge once: keep the copy that starts at the lower entity id.
            if other_id > entity {
                edges.push((index[&entity], index[&other_id], snap.csr.weights[k]));
            }
        }
    }
    (Csr::from_edges(entities.len(), &edges), entities)
}

/// Quality of the partition the engine serves (`communities`, the one
/// `overview` and entity cards show), scored on the entity-induced subgraph.
/// `examples/community_eval` prints it: the flat Leiden partition the probe used
/// to score cannot see H2-19 (mentions weight the community graph, attach moves
/// a hub), so its number never moved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommunityQuality {
    pub modularity: f64,
    pub communities: usize,
    /// Communities holding exactly one entity: the shape H2-19 removes.
    pub singletons: usize,
}

pub fn community_quality(db: &Db) -> Result<CommunityQuality> {
    let snap = snapshot(db, None, false)?;
    let (csr, entities) = entity_subgraph(&snap);
    let labels = communities(db)?;
    let comm: Vec<u32> = entities
        .iter()
        .map(|entity| labels.get(entity).copied().unwrap_or(-1) as u32)
        .collect();
    let mut sizes: HashMap<u32, usize> = HashMap::new();
    for &c in &comm {
        *sizes.entry(c).or_insert(0) += 1;
    }
    Ok(CommunityQuality {
        modularity: crate::graph::modularity(&csr, &comm, 1.0),
        communities: sizes.len(),
        singletons: sizes.values().filter(|n| **n == 1).count(),
    })
}
