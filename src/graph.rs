//! Graph algorithms over an in-memory CSR snapshot of the knowledge graph:
//! approximate personalized PageRank (Andersen-Chung-Lang push). The
//! graphqlite projection and adjacency loading live in `kg.rs`.

use std::collections::VecDeque;

/// Undirected weighted graph in CSR form. Node ids are dense 0..n.
pub struct Csr {
    pub offsets: Vec<usize>,
    pub targets: Vec<u32>,
    pub weights: Vec<f32>,
    /// Weighted degree (sum of incident weights) per node.
    pub wdeg: Vec<f64>,
}

impl Csr {
    /// Build from (a, b, w) pairs; each pair is added in both directions,
    /// duplicate pairs are merged by summing weights, self-loops and w <= 0
    /// are dropped.
    pub fn from_edges(n: usize, edges: &[(u32, u32, f32)]) -> Csr {
        if n == 0 {
            return Csr {
                offsets: vec![0],
                targets: Vec::new(),
                weights: Vec::new(),
                wdeg: Vec::new(),
            };
        }

        let mut directed = Vec::with_capacity(edges.len() * 2);
        for &(u, v, w) in edges {
            if u != v && w > 0.0 && !w.is_nan() && (u as usize) < n && (v as usize) < n {
                directed.push((u, v, w));
                directed.push((v, u, w));
            }
        }
        directed.sort_unstable_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

        let mut offsets = vec![0usize; n + 1];
        let mut targets = Vec::with_capacity(directed.len());
        let mut weights = Vec::with_capacity(directed.len());
        let mut wdeg = vec![0.0f64; n];

        let mut it = directed.into_iter();
        if let Some(first) = it.next() {
            let mut curr_u = first.0;
            let mut curr_v = first.1;
            let mut curr_w = first.2;
            for (u, v, w) in it {
                if u == curr_u && v == curr_v {
                    curr_w += w;
                } else {
                    targets.push(curr_v);
                    weights.push(curr_w);
                    wdeg[curr_u as usize] += curr_w as f64;
                    offsets[curr_u as usize + 1] += 1;
                    curr_u = u;
                    curr_v = v;
                    curr_w = w;
                }
            }
            targets.push(curr_v);
            weights.push(curr_w);
            wdeg[curr_u as usize] += curr_w as f64;
            offsets[curr_u as usize + 1] += 1;
        }

        for i in 0..n {
            offsets[i + 1] += offsets[i];
        }

        Csr {
            offsets,
            targets,
            weights,
            wdeg,
        }
    }

    pub fn len(&self) -> usize {
        self.wdeg.len()
    }

    pub fn is_empty(&self) -> bool {
        self.wdeg.is_empty()
    }
}

/// Personalized PageRank by ACL push (non-lazy):
/// p = alpha * s + (1 - alpha) * p * D^-1 W, where s = seeds normalised to
/// sum 1. Push node u while r(u) >= eps * wdeg(u); nodes with wdeg 0 keep
/// their residual (act as self-loops). Stops after max_pushes pushes.
/// Returns (node, score) with score > 0, sorted descending.
pub fn ppr_push(
    g: &Csr,
    seeds: &[(u32, f64)],
    alpha: f64,
    eps: f64,
    max_pushes: usize,
) -> Vec<(u32, f64)> {
    let n = g.len();
    if n == 0 || max_pushes == 0 {
        return Vec::new();
    }

    let mut seed_mass = vec![0.0f64; n];
    let mut total_weight = 0.0f64;
    for &(u, w) in seeds {
        let u_idx = u as usize;
        if u_idx < n && w > 0.0 && !w.is_nan() {
            seed_mass[u_idx] += w;
            total_weight += w;
        }
    }

    if total_weight <= 0.0 {
        return Vec::new();
    }

    let mut r = vec![0.0f64; n];
    let mut p = vec![0.0f64; n];
    let mut in_queue = vec![false; n];
    let mut queue = VecDeque::new();

    for u in 0..n {
        let w = seed_mass[u];
        if w > 0.0 {
            let norm_w = w / total_weight;
            r[u] = norm_w;
            let thresh = if g.wdeg[u] == 0.0 {
                eps
            } else {
                eps * g.wdeg[u]
            };
            if norm_w >= thresh {
                queue.push_back(u as u32);
                in_queue[u] = true;
            }
        }
    }

    let mut pushes = 0;
    while let Some(u) = queue.pop_front() {
        let u_idx = u as usize;
        in_queue[u_idx] = false;
        let ru = r[u_idx];
        if ru <= 0.0 {
            continue;
        }

        pushes += 1;
        if g.wdeg[u_idx] == 0.0 {
            p[u_idx] += ru;
            r[u_idx] = 0.0;
        } else {
            p[u_idx] += alpha * ru;
            let to_spread = (1.0 - alpha) * ru;
            let deg = g.wdeg[u_idx];
            r[u_idx] = 0.0;

            let start = g.offsets[u_idx];
            let end = g.offsets[u_idx + 1];
            for i in start..end {
                let v = g.targets[i];
                let v_idx = v as usize;
                let w = g.weights[i] as f64;
                r[v_idx] += to_spread * (w / deg);
                let thresh = if g.wdeg[v_idx] == 0.0 {
                    eps
                } else {
                    eps * g.wdeg[v_idx]
                };
                if r[v_idx] >= thresh && !in_queue[v_idx] {
                    queue.push_back(v);
                    in_queue[v_idx] = true;
                }
            }
        }

        if pushes >= max_pushes {
            break;
        }
    }

    let mut result: Vec<(u32, f64)> = p
        .into_iter()
        .enumerate()
        .filter(|&(_, score)| score > 0.0 && !score.is_nan())
        .map(|(node, score)| (node as u32, score))
        .collect();

    result.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    result
}

/// Exact PPR by power iteration with the same semantics as ppr_push
/// (reference for tests).
pub fn ppr_power(g: &Csr, seeds: &[(u32, f64)], alpha: f64, iters: usize) -> Vec<f64> {
    let n = g.len();
    if n == 0 {
        return Vec::new();
    }

    let mut s = vec![0.0f64; n];
    let mut total_weight = 0.0f64;
    for &(u, w) in seeds {
        let u_idx = u as usize;
        if u_idx < n && w > 0.0 && !w.is_nan() {
            s[u_idx] += w;
            total_weight += w;
        }
    }

    if total_weight <= 0.0 {
        return vec![0.0; n];
    }

    for val in s.iter_mut() {
        *val /= total_weight;
    }

    let mut x = s.clone();
    for _ in 0..iters {
        let mut y = vec![0.0f64; n];
        for u in 0..n {
            let xu = x[u];
            if xu == 0.0 {
                continue;
            }
            if g.wdeg[u] == 0.0 {
                y[u] += xu;
            } else {
                let deg = g.wdeg[u];
                let start = g.offsets[u];
                let end = g.offsets[u + 1];
                for i in start..end {
                    let v = g.targets[i] as usize;
                    let w = g.weights[i] as f64;
                    y[v] += xu * (w / deg);
                }
            }
        }

        for i in 0..n {
            x[i] = alpha * s[i] + (1.0 - alpha) * y[i];
        }
    }

    x
}

// ---------------------------------------------------------------- Leiden

const EPS: f64 = 1e-12;

/// Deterministic pseudorandom source: seeded XorShift64, so a graph plus a
/// seed always yields the same node visit order.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below(i + 1);
            items.swap(i, j);
        }
    }
}

/// Undirected weighted graph in f64, used at every Leiden level.
struct Work {
    n: usize,
    offsets: Vec<usize>,
    targets: Vec<u32>,
    weights: Vec<f64>,
    /// Weight inside the node itself: edges collapsed by earlier aggregation.
    selfw: Vec<f64>,
    /// Full weighted degree, including `selfw`.
    wdeg: Vec<f64>,
    total: f64,
}

impl Work {
    fn from_csr(csr: &Csr) -> Self {
        Work {
            n: csr.len(),
            offsets: csr.offsets.clone(),
            targets: csr.targets.clone(),
            weights: csr.weights.iter().map(|w| f64::from(*w)).collect(),
            selfw: vec![0.0; csr.len()],
            wdeg: csr.wdeg.clone(),
            total: csr.wdeg.iter().sum(),
        }
    }

    /// Same merge rules as `Csr::from_edges`, but each undirected pair is
    /// expected once and kept in f64.
    fn from_edges(n: usize, edges: &[(u32, u32, f64)]) -> Self {
        if n == 0 {
            return Work {
                n: 0,
                offsets: vec![0],
                targets: Vec::new(),
                weights: Vec::new(),
                selfw: Vec::new(),
                wdeg: Vec::new(),
                total: 0.0,
            };
        }
        let mut directed = Vec::with_capacity(edges.len() * 2);
        for &(u, v, w) in edges {
            if u != v && w > 0.0 && (u as usize) < n && (v as usize) < n {
                directed.push((u, v, w));
                directed.push((v, u, w));
            }
        }
        directed.sort_unstable_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        let mut offsets = vec![0usize; n + 1];
        let mut targets = Vec::with_capacity(directed.len());
        let mut weights = Vec::with_capacity(directed.len());
        let mut wdeg = vec![0.0f64; n];
        let mut it = directed.into_iter().peekable();
        while let Some((u, v, mut w)) = it.next() {
            while let Some(&(nu, nv, nw)) = it.peek() {
                if nu == u && nv == v {
                    w += nw;
                    it.next();
                } else {
                    break;
                }
            }
            targets.push(v);
            weights.push(w);
            wdeg[u as usize] += w;
            offsets[u as usize + 1] += 1;
        }
        for i in 0..n {
            offsets[i + 1] += offsets[i];
        }
        let total = wdeg.iter().sum();
        Work {
            n,
            offsets,
            targets,
            weights,
            selfw: vec![0.0; n],
            wdeg,
            total,
        }
    }

    /// Fast local moving: every node joins the community with the largest
    /// modularity gain, in a seeded visit order, until a pass changes nothing.
    fn local_moving(&self, resolution: f64, rng: &mut Rng) -> Vec<u32> {
        let n = self.n;
        let mut comm: Vec<u32> = (0..n as u32).collect();
        let mut tot = self.wdeg.clone();
        let mut to = vec![0.0f64; n];
        let mut touched: Vec<u32> = Vec::new();
        let mut perm: Vec<u32> = (0..n as u32).collect();
        for _ in 0..64 {
            rng.shuffle(&mut perm);
            let mut moved = 0usize;
            for &node in &perm {
                let i = node as usize;
                let deg = self.wdeg[i];
                let old = comm[i] as usize;
                tot[old] -= deg;
                touched.clear();
                for e in self.offsets[i]..self.offsets[i + 1] {
                    let c = comm[self.targets[e] as usize] as usize;
                    if to[c] == 0.0 {
                        touched.push(c as u32);
                    }
                    to[c] += self.weights[e];
                }
                let mut best = old;
                let mut best_gain = to[old] - resolution * deg * tot[old] / self.total;
                for &c32 in &touched {
                    let c = c32 as usize;
                    let gain = to[c] - resolution * deg * tot[c] / self.total;
                    // Ties go to the lowest community id: deterministic and
                    // independent of neighbour iteration order.
                    if gain > best_gain + EPS || ((gain - best_gain).abs() <= EPS && c < best) {
                        if gain > best_gain {
                            best_gain = gain;
                        }
                        best = c;
                    }
                }
                for &c in &touched {
                    to[c as usize] = 0.0;
                }
                tot[best] += deg;
                comm[i] = best as u32;
                if best != old {
                    moved += 1;
                }
            }
            if moved == 0 {
                break;
            }
        }
        comm
    }

    /// Refinement: inside every coarse community, run local moving from
    /// singletons, then split each result into connected components. Every
    /// returned community is connected in this graph (the Leiden guarantee).
    fn refine(&self, part: &[u32], k: usize, resolution: f64, rng: &mut Rng) -> Vec<u32> {
        let n = self.n;
        let mut groups: Vec<Vec<u32>> = vec![Vec::new(); k];
        for (i, &c) in part.iter().enumerate() {
            groups[c as usize].push(i as u32);
        }
        let mut local = vec![u32::MAX; n];
        let mut edges: Vec<(u32, u32, f64)> = Vec::new();
        let mut refined = vec![u32::MAX; n];
        let mut comp = Vec::new();
        let mut queue = VecDeque::new();
        let mut next = 0u32;
        for group in groups {
            if group.len() == 1 {
                refined[group[0] as usize] = next;
                next += 1;
                continue;
            }
            for (li, &node) in group.iter().enumerate() {
                local[node as usize] = li as u32;
            }
            edges.clear();
            for &node in &group {
                let i = node as usize;
                for e in self.offsets[i]..self.offsets[i + 1] {
                    let j = self.targets[e] as usize;
                    let lj = local[j];
                    // Each undirected edge once: from its lower local index.
                    if lj != u32::MAX && local[i] < lj {
                        edges.push((local[i], lj, self.weights[e]));
                    }
                }
            }
            let sub = Work::from_edges(group.len(), &edges);
            let lp = sub.local_moving(resolution, rng);
            comp.clear();
            comp.resize(sub.n, u32::MAX);
            for start in 0..sub.n {
                if comp[start] != u32::MAX {
                    continue;
                }
                comp[start] = next;
                queue.clear();
                queue.push_back(start as u32);
                while let Some(x) = queue.pop_front() {
                    let xi = x as usize;
                    for e in sub.offsets[xi]..sub.offsets[xi + 1] {
                        let t = sub.targets[e] as usize;
                        if comp[t] == u32::MAX && lp[t] == lp[xi] {
                            comp[t] = next;
                            queue.push_back(t as u32);
                        }
                    }
                }
                next += 1;
            }
            for (li, &node) in group.iter().enumerate() {
                refined[node as usize] = comp[li];
            }
            for &node in &group {
                local[node as usize] = u32::MAX;
            }
        }
        refined
    }

    /// Collapse `part` (dense ids < k) into one node per community.
    fn aggregate(&self, part: &[u32], k: usize) -> Work {
        let mut internal = vec![0.0f64; k];
        let mut acc: std::collections::HashMap<(u32, u32), f64> = std::collections::HashMap::new();
        for i in 0..self.n {
            let a = part[i];
            // Weight already inside node i (from earlier levels) belongs to
            // the new community's interior. Losing it would shrink `total`
            // and make later merges look free.
            internal[a as usize] += self.selfw[i];
            for e in self.offsets[i]..self.offsets[i + 1] {
                let b = part[self.targets[e] as usize];
                let w = self.weights[e];
                if a == b {
                    // Both edge directions land here: 2x the undirected weight,
                    // which is what summed member degrees hold.
                    internal[a as usize] += w;
                } else if a < b {
                    *acc.entry((a, b)).or_insert(0.0) += w;
                }
            }
        }
        let edges: Vec<(u32, u32, f64)> = acc.into_iter().map(|((a, b), w)| (a, b, w)).collect();
        let mut g = Work::from_edges(k, &edges);
        for (c, w) in internal.into_iter().enumerate() {
            g.wdeg[c] += w;
            g.selfw[c] = w;
        }
        g.total = g.wdeg.iter().sum();
        g
    }
}

/// Renumber community ids densely in order of first appearance.
fn renumber(comm: &mut [u32]) -> usize {
    let mut map: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    let mut next = 0u32;
    for c in comm.iter_mut() {
        let id = *map.entry(*c).or_insert_with(|| {
            let id = next;
            next += 1;
            id
        });
        *c = id;
    }
    next as usize
}

/// Deterministic Leiden community detection (Traag, Waltman & van Eck 2019).
///
/// Repeatedly: local moving, refinement that splits every community into
/// connected pieces (the well-connected guarantee Louvain lacks), then
/// aggregation by the refined partition. The visit order comes from a seeded
/// XorShift64 and ties go to the lowest community id, so a given graph and
/// seed always produce the same partition.
pub fn leiden(csr: &Csr, seed: u64, resolution: f64) -> Vec<u32> {
    let n = csr.len();
    if n == 0 {
        return Vec::new();
    }
    if csr.wdeg.iter().all(|w| *w <= 0.0) {
        return (0..n as u32).collect();
    }
    let mut rng = Rng::new(seed);
    let mut graph = Work::from_csr(csr);
    let mut membership: Vec<u32> = (0..n as u32).collect();
    for _ in 0..16 {
        let mut part = graph.local_moving(resolution, &mut rng);
        let k = renumber(&mut part);
        if k == graph.n {
            for m in &mut membership {
                *m = part[*m as usize];
            }
            renumber(&mut membership);
            return membership;
        }
        let refined = graph.refine(&part, k, resolution, &mut rng);
        let rk = refined.iter().copied().max().map_or(0, |m| m as usize + 1);
        for m in &mut membership {
            *m = refined[*m as usize];
        }
        if rk == graph.n {
            break;
        }
        graph = graph.aggregate(&refined, rk);
    }
    renumber(&mut membership);
    membership
}

/// Newman modularity of `comm` on `csr` at the given resolution.
pub fn modularity(csr: &Csr, comm: &[u32], resolution: f64) -> f64 {
    let n = csr.len();
    if n == 0 || comm.len() != n {
        return 0.0;
    }
    let m2: f64 = csr.wdeg.iter().sum();
    if m2 <= 0.0 {
        return 0.0;
    }
    let k = comm.iter().copied().max().map_or(0, |c| c as usize + 1);
    let mut tot = vec![0.0f64; k];
    let mut inside = vec![0.0f64; k];
    for i in 0..n {
        tot[comm[i] as usize] += csr.wdeg[i];
        for e in csr.offsets[i]..csr.offsets[i + 1] {
            if comm[csr.targets[e] as usize] == comm[i] {
                inside[comm[i] as usize] += f64::from(csr.weights[e]);
            }
        }
    }
    (0..k)
        .map(|c| inside[c] / m2 - resolution * (tot[c] / m2).powi(2))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csr_from_edges_basics() {
        // 4 nodes: 0, 1, 2, 3
        // Edges: (0, 1, 2.0), (1, 0, 1.0) -> duplicate should merge to weight 3.0
        // (1, 2, 1.5)
        // (2, 2, 5.0) -> self loop should be dropped
        // (0, 3, -1.0) -> negative weight dropped
        // (0, 3, 0.0) -> zero weight dropped
        // (0, 5, 2.0) -> out of bounds (n=4) dropped
        let edges = vec![
            (0, 1, 2.0),
            (1, 0, 1.0),
            (1, 2, 1.5),
            (2, 2, 5.0),
            (0, 3, -1.0),
            (0, 3, 0.0),
            (0, 5, 2.0),
        ];
        let csr = Csr::from_edges(4, &edges);
        assert_eq!(csr.len(), 4);
        assert!(!csr.is_empty());

        // Node 0 should have neighbor 1 with weight 3.0
        assert_eq!(csr.offsets[0], 0);
        assert_eq!(csr.offsets[1], 1);
        assert_eq!(csr.targets[0], 1);
        assert_eq!(csr.weights[0], 3.0);
        assert!((csr.wdeg[0] - 3.0).abs() < 1e-6);

        // Node 1 should have neighbors 0 (w=3.0) and 2 (w=1.5), sorted by target id
        assert_eq!(csr.offsets[2], 3);
        assert_eq!(&csr.targets[csr.offsets[1]..csr.offsets[2]], &[0, 2]);
        assert_eq!(&csr.weights[csr.offsets[1]..csr.offsets[2]], &[3.0, 1.5]);
        assert!((csr.wdeg[1] - 4.5).abs() < 1e-6);

        // Node 2 should have neighbor 1 with weight 1.5
        assert_eq!(csr.offsets[3], 4);
        assert_eq!(csr.targets[3], 1);
        assert_eq!(csr.weights[3], 1.5);
        assert!((csr.wdeg[2] - 1.5).abs() < 1e-6);

        // Node 3 is isolated
        assert_eq!(csr.offsets[4], 4);
        assert_eq!(csr.wdeg[3], 0.0);
    }

    #[test]
    fn test_ppr_three_node_path() {
        // Path graph 0 - 1 - 2, edge weights 1.0
        let edges = vec![(0, 1, 1.0), (1, 2, 1.0)];
        let g = Csr::from_edges(3, &edges);

        // Seed at node 0 with weight 1.0
        let seeds = vec![(0, 1.0)];
        let alpha = 0.5;

        // Exact analytical values: node 0: 7/12 (~0.58333), node 1: 1/3 (~0.33333), node 2: 1/12 (~0.08333)
        let exact_0 = 7.0 / 12.0;
        let exact_1 = 1.0 / 3.0;
        let exact_2 = 1.0 / 12.0;

        let power = ppr_power(&g, &seeds, alpha, 200);
        assert_eq!(power.len(), 3);
        assert!((power[0] - exact_0).abs() < 1e-5, "power[0]={}", power[0]);
        assert!((power[1] - exact_1).abs() < 1e-5, "power[1]={}", power[1]);
        assert!((power[2] - exact_2).abs() < 1e-5, "power[2]={}", power[2]);

        let push = ppr_push(&g, &seeds, alpha, 1e-8, 100_000);
        assert_eq!(push.len(), 3);
        // Sorted desc by score: 0, then 1, then 2
        assert_eq!(push[0].0, 0);
        assert_eq!(push[1].0, 1);
        assert_eq!(push[2].0, 2);
        assert!((push[0].1 - exact_0).abs() < 1e-5, "push[0]={}", push[0].1);
        assert!((push[1].1 - exact_1).abs() < 1e-5, "push[1]={}", push[1].1);
        assert!((push[2].1 - exact_2).abs() < 1e-5, "push[2]={}", push[2].1);
    }

    #[test]
    fn test_seeds_out_of_range_and_ignored() {
        let edges = vec![(0, 1, 1.0)];
        let g = Csr::from_edges(2, &edges);

        // All invalid seeds
        let seeds = vec![(2, 1.0), (3, 5.0), (0, -1.0), (1, 0.0)];
        let res = ppr_push(&g, &seeds, 0.5, 1e-5, 1000);
        assert!(res.is_empty());

        let power_res = ppr_power(&g, &seeds, 0.5, 50);
        assert_eq!(power_res, vec![0.0, 0.0]);

        // Mix of valid and invalid
        let seeds2 = vec![(10, 5.0), (0, 2.0), (0, -1.0)];
        let res2 = ppr_push(&g, &seeds2, 0.5, 1e-5, 1000);
        assert!(!res2.is_empty());
        assert_eq!(res2[0].0, 0);
    }

    #[test]
    fn test_monotonic_star_graph() {
        // Star graph: center 0, leaves 1, 2, 3, 4, 5
        let edges = vec![
            (0, 1, 1.0),
            (0, 2, 1.0),
            (0, 3, 1.0),
            (0, 4, 1.0),
            (0, 5, 1.0),
        ];
        let g = Csr::from_edges(6, &edges);
        let seeds = vec![(0, 1.0)];
        let res = ppr_push(&g, &seeds, 0.5, 1e-6, 10_000);
        assert_eq!(res[0].0, 0, "Seed center node must have highest score");
        for entry in &res[1..] {
            assert!(entry.1 < res[0].1);
        }
    }

    struct XorShift64 {
        state: u64,
    }

    impl XorShift64 {
        fn new(seed: u64) -> Self {
            Self {
                state: if seed == 0 { 0xdeadbeef } else { seed },
            }
        }

        fn next_u64(&mut self) -> u64 {
            let mut x = self.state;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.state = x;
            x
        }

        fn gen_range(&mut self, low: usize, high: usize) -> usize {
            if low >= high {
                return low;
            }
            low + (self.next_u64() as usize % (high - low))
        }

        fn gen_f32(&mut self) -> f32 {
            ((self.next_u64() & 0xFFFFFF) as f32) / 16777216.0
        }
    }

    #[test]
    fn test_property_random_graphs_l1_convergence() {
        let mut rng = XorShift64::new(42);

        for trial in 0..20 {
            let n = rng.gen_range(50, 501);
            let num_edges = rng.gen_range(n / 2, n * 3);
            let mut edges = Vec::with_capacity(num_edges);

            for _ in 0..num_edges {
                let u = rng.gen_range(0, n) as u32;
                let v = rng.gen_range(0, n) as u32;
                let w = rng.gen_f32() * 5.0 + 0.1;
                edges.push((u, v, w));
            }

            let g = Csr::from_edges(n, &edges);

            let num_seeds = rng.gen_range(1, 6);
            let mut seeds = Vec::new();
            for _ in 0..num_seeds {
                let s = rng.gen_range(0, n + 10) as u32; // some may be out of range
                let w = rng.gen_f32() as f64 * 3.0 + 0.1;
                seeds.push((s, w));
            }

            let alpha = 0.5;
            let push_res = ppr_push(&g, &seeds, alpha, 1e-8, 500_000);
            let power_res = ppr_power(&g, &seeds, alpha, 200);

            // Reconstruct dense vector from push_res
            let mut push_dense = vec![0.0f64; n];
            for (node, score) in push_res {
                push_dense[node as usize] = score;
            }

            let mut l1_diff = 0.0f64;
            for i in 0..n {
                l1_diff += (push_dense[i] - power_res[i]).abs();
            }

            assert!(
                l1_diff < 1e-3,
                "Trial {}: L1 diff {} >= 1e-3 (n={})",
                trial,
                l1_diff,
                n
            );
        }
    }

    #[test]
    #[ignore]
    fn test_perf_large_graph() {
        use std::time::Instant;

        let n = 200_000;
        let num_edges = 2_000_000;
        let mut rng = XorShift64::new(12345);

        let mut edges = Vec::with_capacity(num_edges);
        for _ in 0..num_edges {
            let u = rng.gen_range(0, n) as u32;
            let v = rng.gen_range(0, n) as u32;
            let w = rng.gen_f32() + 0.01;
            edges.push((u, v, w));
        }

        let g = Csr::from_edges(n, &edges);
        let seeds = vec![(0, 1.0), (100, 0.5)];
        let alpha = 0.5;
        let eps = 1e-5;

        let start = Instant::now();
        let res = ppr_push(&g, &seeds, alpha, eps, 200_000);
        let duration = start.elapsed();

        println!("PPR push on 200k nodes, 2M edges took: {:?}", duration);
        assert!(
            duration.as_millis() < 100,
            "PPR push took {:?}, expected < 100ms in release",
            duration
        );
        assert!(!res.is_empty());
    }

    fn two_triangles() -> Csr {
        let edges = vec![
            (0, 1, 1.0),
            (1, 2, 1.0),
            (0, 2, 1.0),
            (3, 4, 1.0),
            (4, 5, 1.0),
            (3, 5, 1.0),
            (2, 3, 1.0),
        ];
        Csr::from_edges(6, &edges)
    }

    #[test]
    fn leiden_splits_two_cliques_deterministically() {
        let g = two_triangles();
        let a = leiden(&g, 7, 1.0);
        let b = leiden(&g, 7, 1.0);
        assert_eq!(a, b, "same graph and seed must give the same partition");
        assert_eq!(a[0], a[1]);
        assert_eq!(a[1], a[2]);
        assert_eq!(a[3], a[4]);
        assert_eq!(a[4], a[5]);
        assert_ne!(a[0], a[3]);
        let q = modularity(&g, &a, 1.0);
        assert!((q - 0.357).abs() < 0.01, "Q={q}");
    }

    #[test]
    fn leiden_communities_are_connected() {
        // Three cliques joined by single bridges; Leiden must not leave a
        // community in disconnected pieces.
        let mut edges = Vec::new();
        for base in [0u32, 10, 20] {
            for i in 0..4 {
                for j in (i + 1)..4 {
                    edges.push((base + i, base + j, 1.0));
                }
            }
        }
        edges.push((3, 10, 1.0));
        edges.push((13, 20, 1.0));
        edges.push((2, 22, 0.1)); // weak diagonal chord
        let g = Csr::from_edges(24, &edges);
        let comm = leiden(&g, 42, 1.0);
        assert!(comm.iter().any(|c| *c != comm[0]), "expected >1 community");
        let mut reached = [false; 24];
        for start in 0..24 {
            if reached[start] {
                continue;
            }
            let c = comm[start];
            let mut stack = vec![start];
            reached[start] = true;
            while let Some(x) = stack.pop() {
                for e in g.offsets[x]..g.offsets[x + 1] {
                    let t = g.targets[e] as usize;
                    if !reached[t] && comm[t] == c {
                        reached[t] = true;
                        stack.push(t);
                    }
                }
            }
        }
        assert!(
            reached.iter().all(|r| *r),
            "every node reached from its community"
        );
    }

    #[test]
    fn leiden_isolated_nodes_are_singletons() {
        let g = Csr::from_edges(4, &[(0, 1, 2.0)]);
        let comm = leiden(&g, 1, 1.0);
        assert_eq!(comm[0], comm[1]);
        assert_ne!(comm[2], comm[0], "isolated node joins nothing");
        assert_ne!(comm[3], comm[0], "isolated node joins nothing");
        assert_ne!(comm[2], comm[3], "isolated nodes are separate singletons");
    }
}
