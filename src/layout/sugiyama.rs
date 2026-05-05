use crate::diagram::Direction;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use std::collections::{HashMap, HashSet};

pub struct SimpleEdge {
    pub from: String,
    pub to: String,
}

struct DummyInfo {
    original_from: String,
    original_to: String,
    chain_index: usize,
}

#[derive(Clone)]
pub struct SubgraphLayout {
    pub positions: HashMap<String, (f32, f32)>,
    pub edge_waypoints: HashMap<(String, String), Vec<(f32, f32)>>,
}

pub fn layout_nodes_grouped(
    node_ids: &[String],
    node_sizes: &HashMap<String, (f32, f32)>,
    edges: &[SimpleEdge],
    direction: Direction,
    node_groups: Option<&HashMap<String, usize>>,
) -> SubgraphLayout {
    if node_ids.is_empty() {
        return SubgraphLayout {
            positions: HashMap::new(),
            edge_waypoints: HashMap::new(),
        };
    }

    if node_ids.len() == 1 {
        let id = &node_ids[0];
        let (w, h) = node_sizes.get(id).copied().unwrap_or((120.0, 40.0));
        let mut positions = HashMap::new();
        positions.insert(id.clone(), (w / 2.0, h / 2.0));
        return SubgraphLayout {
            positions,
            edge_waypoints: HashMap::new(),
        };
    }

    let node_set: HashSet<&str> = node_ids.iter().map(|s| s.as_str()).collect();
    let internal_edges: Vec<&SimpleEdge> = edges
        .iter()
        .filter(|e| node_set.contains(e.from.as_str()) && node_set.contains(e.to.as_str()))
        .collect();

    let (mut layers, _reversed) = assign_layers(node_ids, &internal_edges);
    let mut extended_sizes = node_sizes.clone();
    let mut extended_groups = node_groups.cloned();
    let (normalized_edges, dummy_info) = normalize_edges(
        &mut layers,
        &internal_edges,
        &mut extended_sizes,
        extended_groups.as_mut(),
    );
    let normalized_refs: Vec<&SimpleEdge> = normalized_edges.iter().collect();
    let dummy_set: HashSet<String> = dummy_info.keys().cloned().collect();

    let ordered = minimize_crossings_grouped(&layers, &normalized_refs, extended_groups.as_ref());
    let mut all_positions = assign_coordinates_grouped(
        &ordered,
        &extended_sizes,
        direction,
        extended_groups.as_ref(),
        &dummy_set,
        &normalized_edges,
    );

    straighten_dummy_chains(&mut all_positions, &dummy_info, direction);

    let (positions, edge_waypoints) = denormalize_edges(&all_positions, &dummy_info);

    SubgraphLayout {
        positions,
        edge_waypoints,
    }
}

fn normalize_edges(
    layers: &mut Vec<Vec<String>>,
    edges: &[&SimpleEdge],
    node_sizes: &mut HashMap<String, (f32, f32)>,
    mut node_groups: Option<&mut HashMap<String, usize>>,
) -> (Vec<SimpleEdge>, HashMap<String, DummyInfo>) {
    let node_to_layer: HashMap<String, usize> = layers
        .iter()
        .enumerate()
        .flat_map(|(li, layer)| layer.iter().map(move |id| (id.clone(), li)))
        .collect();

    let mut new_edges: Vec<SimpleEdge> = Vec::new();
    let mut dummy_info: HashMap<String, DummyInfo> = HashMap::new();

    for (edge_idx, edge) in edges.iter().enumerate() {
        let from_layer = match node_to_layer.get(&edge.from) {
            Some(&l) => l,
            None => {
                new_edges.push(SimpleEdge {
                    from: edge.from.clone(),
                    to: edge.to.clone(),
                });
                continue;
            }
        };
        let to_layer = match node_to_layer.get(&edge.to) {
            Some(&l) => l,
            None => {
                new_edges.push(SimpleEdge {
                    from: edge.from.clone(),
                    to: edge.to.clone(),
                });
                continue;
            }
        };

        let (start_layer, end_layer, start_id, end_id) = if from_layer < to_layer {
            (from_layer, to_layer, &edge.from, &edge.to)
        } else if from_layer > to_layer {
            (to_layer, from_layer, &edge.to, &edge.from)
        } else {
            new_edges.push(SimpleEdge {
                from: edge.from.clone(),
                to: edge.to.clone(),
            });
            continue;
        };

        let span = end_layer - start_layer;
        if span <= 1 {
            new_edges.push(SimpleEdge {
                from: edge.from.clone(),
                to: edge.to.clone(),
            });
            continue;
        }

        let source_group = node_groups
            .as_ref()
            .and_then(|g| g.get(&edge.from).copied());

        let mut prev_id = start_id.clone();
        for i in 1..span {
            let layer_idx = start_layer + i;
            let dummy_id = format!("__d_{}_{}_{}_{}", edge.from, edge.to, edge_idx, layer_idx);

            node_sizes.insert(dummy_id.clone(), (0.0, 0.0));
            layers[layer_idx].push(dummy_id.clone());

            if let Some(ref mut groups) = node_groups {
                if let Some(g) = source_group {
                    groups.insert(dummy_id.clone(), g);
                }
            }

            dummy_info.insert(
                dummy_id.clone(),
                DummyInfo {
                    original_from: edge.from.clone(),
                    original_to: edge.to.clone(),
                    chain_index: i - 1,
                },
            );

            new_edges.push(SimpleEdge {
                from: prev_id,
                to: dummy_id.clone(),
            });
            prev_id = dummy_id;
        }

        new_edges.push(SimpleEdge {
            from: prev_id,
            to: end_id.clone(),
        });
    }

    (new_edges, dummy_info)
}

fn straighten_dummy_chains(
    positions: &mut HashMap<String, (f32, f32)>,
    dummy_info: &HashMap<String, DummyInfo>,
    direction: Direction,
) {
    let is_horizontal = matches!(direction, Direction::LR | Direction::RL);

    let mut chains: HashMap<(String, String), Vec<(String, usize)>> = HashMap::new();
    for (dummy_id, info) in dummy_info {
        chains
            .entry((info.original_from.clone(), info.original_to.clone()))
            .or_default()
            .push((dummy_id.clone(), info.chain_index));
    }

    for ((from, to), mut dummies) in chains {
        let source_pos = match positions.get(&from) {
            Some(&p) => p,
            None => continue,
        };
        let target_pos = match positions.get(&to) {
            Some(&p) => p,
            None => continue,
        };

        dummies.sort_by_key(|(_, idx)| *idx);
        let n = dummies.len();

        for (i, (dummy_id, _)) in dummies.iter().enumerate() {
            if let Some(pos) = positions.get_mut(dummy_id) {
                let t = (i + 1) as f32 / (n + 1) as f32;
                if is_horizontal {
                    let ideal_y = source_pos.1 + t * (target_pos.1 - source_pos.1);
                    pos.1 = pos.1 * 0.3 + ideal_y * 0.7;
                } else {
                    let ideal_x = source_pos.0 + t * (target_pos.0 - source_pos.0);
                    pos.0 = pos.0 * 0.3 + ideal_x * 0.7;
                }
            }
        }
    }
}

fn denormalize_edges(
    all_positions: &HashMap<String, (f32, f32)>,
    dummy_info: &HashMap<String, DummyInfo>,
) -> (HashMap<String, (f32, f32)>, HashMap<(String, String), Vec<(f32, f32)>>) {
    let mut edge_dummies: HashMap<(String, String), Vec<(String, usize)>> = HashMap::new();
    for (dummy_id, info) in dummy_info {
        edge_dummies
            .entry((info.original_from.clone(), info.original_to.clone()))
            .or_default()
            .push((dummy_id.clone(), info.chain_index));
    }

    let mut edge_waypoints: HashMap<(String, String), Vec<(f32, f32)>> = HashMap::new();
    for (edge_key, mut dummies) in edge_dummies {
        dummies.sort_by_key(|(_, idx)| *idx);
        let waypoints: Vec<(f32, f32)> = dummies
            .iter()
            .filter_map(|(id, _)| all_positions.get(id).copied())
            .collect();
        if !waypoints.is_empty() {
            edge_waypoints.insert(edge_key, waypoints);
        }
    }

    let positions: HashMap<String, (f32, f32)> = all_positions
        .iter()
        .filter(|(id, _)| !dummy_info.contains_key(id.as_str()))
        .map(|(id, pos)| (id.clone(), *pos))
        .collect();

    (positions, edge_waypoints)
}

fn longest_path_ranking(
    graph: &DiGraph<String, ()>,
    topo: &[NodeIndex],
) -> HashMap<NodeIndex, i32> {
    let mut rank: HashMap<NodeIndex, i32> = HashMap::new();
    for &node in topo {
        let max_pred = graph
            .neighbors_directed(node, petgraph::Direction::Incoming)
            .filter_map(|pred| rank.get(&pred).map(|r| r + 1))
            .max()
            .unwrap_or(0);
        rank.insert(node, max_pred);
    }
    rank
}

fn slack(
    rank: &HashMap<NodeIndex, i32>,
    src: NodeIndex,
    tgt: NodeIndex,
) -> i32 {
    rank[&tgt] - rank[&src] - 1
}

fn feasible_tight_tree(
    graph: &DiGraph<String, ()>,
    rank: &mut HashMap<NodeIndex, i32>,
) -> HashSet<(NodeIndex, NodeIndex)> {
    let all_nodes: Vec<NodeIndex> = graph.node_indices().collect();
    if all_nodes.is_empty() {
        return HashSet::new();
    }

    let mut tree_nodes: HashSet<NodeIndex> = HashSet::new();
    let mut tree_edges: HashSet<(NodeIndex, NodeIndex)> = HashSet::new();

    tree_nodes.insert(all_nodes[0]);

    loop {
        let mut grew = true;
        while grew {
            grew = false;
            let mut current: Vec<NodeIndex> = tree_nodes.iter().copied().collect();
            current.sort();
            for &node in &current {
                for edge in graph.edges_directed(node, petgraph::Direction::Outgoing) {
                    let tgt = edge.target();
                    if !tree_nodes.contains(&tgt) && slack(rank, node, tgt) == 0 {
                        tree_nodes.insert(tgt);
                        let canonical = (node.min(tgt), node.max(tgt));
                        tree_edges.insert(canonical);
                        grew = true;
                    }
                }
                for edge in graph.edges_directed(node, petgraph::Direction::Incoming) {
                    let src = edge.source();
                    if !tree_nodes.contains(&src) && slack(rank, src, node) == 0 {
                        tree_nodes.insert(src);
                        let canonical = (src.min(node), src.max(node));
                        tree_edges.insert(canonical);
                        grew = true;
                    }
                }
            }
        }

        if tree_nodes.len() == all_nodes.len() {
            break;
        }

        let mut best_slack = i32::MAX;
        let mut best_delta = 0i32;
        let mut best_edge: Option<(NodeIndex, NodeIndex)> = None;

        let mut sorted_tn: Vec<NodeIndex> = tree_nodes.iter().copied().collect();
        sorted_tn.sort();
        for &tn in &sorted_tn {
            for edge in graph.edges_directed(tn, petgraph::Direction::Outgoing) {
                let tgt = edge.target();
                if !tree_nodes.contains(&tgt) {
                    let s = slack(rank, tn, tgt);
                    if s < best_slack {
                        best_slack = s;
                        best_delta = s;
                        best_edge = Some((tn, tgt));
                    }
                }
            }
            for edge in graph.edges_directed(tn, petgraph::Direction::Incoming) {
                let src = edge.source();
                if !tree_nodes.contains(&src) {
                    let s = slack(rank, src, tn);
                    if s < best_slack {
                        best_slack = s;
                        best_delta = -s;
                        best_edge = Some((src, tn));
                    }
                }
            }
        }

        if best_edge.is_none() {
            break;
        }

        for &tn in &tree_nodes {
            *rank.get_mut(&tn).unwrap() += best_delta;
        }
    }

    tree_edges
}

struct LowLimResult {
    low: HashMap<NodeIndex, usize>,
    lim: HashMap<NodeIndex, usize>,
    parent: HashMap<NodeIndex, Option<NodeIndex>>,
}

fn compute_low_lim(
    tree_edges: &HashSet<(NodeIndex, NodeIndex)>,
    all_nodes: &[NodeIndex],
    root: NodeIndex,
) -> LowLimResult {
    let mut adj: HashMap<NodeIndex, Vec<NodeIndex>> = HashMap::new();
    for &node in all_nodes {
        adj.entry(node).or_default();
    }
    for &(a, b) in tree_edges {
        adj.entry(a).or_default().push(b);
        adj.entry(b).or_default().push(a);
    }
    for v in adj.values_mut() {
        v.sort();
    }

    let mut low: HashMap<NodeIndex, usize> = HashMap::new();
    let mut lim: HashMap<NodeIndex, usize> = HashMap::new();
    let mut parent: HashMap<NodeIndex, Option<NodeIndex>> = HashMap::new();

    enum Action {
        Enter(NodeIndex, Option<NodeIndex>),
        Exit(NodeIndex),
    }

    let mut counter = 1usize;
    let mut stack = vec![Action::Enter(root, None)];
    let mut visited: HashSet<NodeIndex> = HashSet::new();

    while let Some(action) = stack.pop() {
        match action {
            Action::Enter(node, par) => {
                if !visited.insert(node) {
                    continue;
                }
                parent.insert(node, par);
                stack.push(Action::Exit(node));
                if let Some(neighbors) = adj.get(&node) {
                    for &nb in neighbors.iter().rev() {
                        if !visited.contains(&nb) {
                            stack.push(Action::Enter(nb, Some(node)));
                        }
                    }
                }
            }
            Action::Exit(node) => {
                lim.insert(node, counter);
                let min_child = adj.get(&node)
                    .map(|nbs| nbs.iter()
                        .filter(|nb| parent.get(nb) == Some(&Some(node)))
                        .filter_map(|nb| low.get(nb).copied())
                        .min()
                        .unwrap_or(counter))
                    .unwrap_or(counter);
                low.insert(node, min_child.min(counter));
                counter += 1;
            }
        }
    }

    LowLimResult { low, lim, parent }
}

fn is_descendant(
    node: NodeIndex,
    ancestor: NodeIndex,
    low: &HashMap<NodeIndex, usize>,
    lim: &HashMap<NodeIndex, usize>,
) -> bool {
    if let (Some(&nl), Some(&al), Some(&alim)) = (lim.get(&node), low.get(&ancestor), lim.get(&ancestor)) {
        al <= nl && nl <= alim
    } else {
        false
    }
}

fn compute_cut_values(
    graph: &DiGraph<String, ()>,
    tree_edges: &HashSet<(NodeIndex, NodeIndex)>,
    ll: &LowLimResult,
) -> HashMap<(NodeIndex, NodeIndex), f64> {
    let mut cutvalues: HashMap<(NodeIndex, NodeIndex), f64> = HashMap::new();

    let mut nodes_by_lim: Vec<NodeIndex> = ll.lim.keys().copied().collect();
    nodes_by_lim.sort_by_key(|n| ll.lim[n]);

    for &node in &nodes_by_lim {
        let par = match ll.parent.get(&node) {
            Some(Some(p)) => *p,
            _ => continue,
        };

        let canonical = (node.min(par), node.max(par));
        if !tree_edges.contains(&canonical) {
            continue;
        }

        let child_is_tail = graph.find_edge(node, par).is_some();

        let edge_weight = 1.0f64;
        let mut cv = edge_weight;

        for edge_ref in graph.edges_directed(node, petgraph::Direction::Outgoing) {
            let other = edge_ref.target();
            if other == par { continue; }
            let is_out = true;
            let points_to_head = is_out == child_is_tail;
            if points_to_head { cv += 1.0; } else { cv -= 1.0; }

            let other_canonical = (node.min(other), node.max(other));
            if tree_edges.contains(&other_canonical) {
                if let Some(&other_cv) = cutvalues.get(&other_canonical) {
                    if points_to_head { cv -= other_cv; } else { cv += other_cv; }
                }
            }
        }

        for edge_ref in graph.edges_directed(node, petgraph::Direction::Incoming) {
            let other = edge_ref.source();
            if other == par { continue; }
            let is_out = false;
            let points_to_head = is_out == child_is_tail;
            if points_to_head { cv += 1.0; } else { cv -= 1.0; }

            let other_canonical = (node.min(other), node.max(other));
            if tree_edges.contains(&other_canonical) {
                if let Some(&other_cv) = cutvalues.get(&other_canonical) {
                    if points_to_head { cv -= other_cv; } else { cv += other_cv; }
                }
            }
        }

        cutvalues.insert(canonical, cv);
    }

    cutvalues
}

fn leave_edge(
    cutvalues: &HashMap<(NodeIndex, NodeIndex), f64>,
) -> Option<(NodeIndex, NodeIndex)> {
    cutvalues.iter()
        .filter(|(_, &cv)| cv < -1e-9)
        .min_by(|a, b| {
            a.1.partial_cmp(b.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(b.0))
        })
        .map(|(&edge, _)| edge)
}

fn enter_edge(
    graph: &DiGraph<String, ()>,
    leaving: (NodeIndex, NodeIndex),
    tree_edges: &HashSet<(NodeIndex, NodeIndex)>,
    rank: &HashMap<NodeIndex, i32>,
    ll: &LowLimResult,
) -> Option<(NodeIndex, NodeIndex)> {
    let (a, b) = leaving;
    let (tail, head) = if graph.find_edge(a, b).is_some() {
        (a, b)
    } else {
        (b, a)
    };

    let tail_lim = ll.lim.get(&tail).copied().unwrap_or(0);
    let head_lim = ll.lim.get(&head).copied().unwrap_or(0);
    let flip = tail_lim > head_lim;

    let tail_label = if flip { head } else { tail };

    let mut best: Option<(NodeIndex, NodeIndex)> = None;
    let mut best_slack = i32::MAX;

    for edge_ref in graph.edge_references() {
        let src = edge_ref.source();
        let tgt = edge_ref.target();
        let canonical = (src.min(tgt), src.max(tgt));
        if tree_edges.contains(&canonical) {
            continue;
        }

        let src_desc = is_descendant(src, tail_label, &ll.low, &ll.lim);
        let tgt_desc = is_descendant(tgt, tail_label, &ll.low, &ll.lim);

        if (flip == src_desc) && (flip != tgt_desc) {
            let s = slack(rank, src, tgt);
            if s < best_slack {
                best_slack = s;
                best = Some((src, tgt));
            }
        }
    }

    best
}

fn network_simplex_rank(
    graph: &DiGraph<String, ()>,
    topo: &[NodeIndex],
) -> Option<HashMap<NodeIndex, i32>> {
    let all_nodes: Vec<NodeIndex> = graph.node_indices().collect();
    if all_nodes.is_empty() {
        return Some(HashMap::new());
    }
    if all_nodes.len() == 1 {
        let mut r = HashMap::new();
        r.insert(all_nodes[0], 0);
        return Some(r);
    }

    if graph.edge_count() == 0 {
        let mut r = HashMap::new();
        for &n in &all_nodes {
            r.insert(n, 0);
        }
        return Some(r);
    }

    let mut rank = longest_path_ranking(graph, topo);

    let mut tree_edges = feasible_tight_tree(graph, &mut rank);

    if tree_edges.len() < all_nodes.len().saturating_sub(1) {
        return None;
    }

    let root = all_nodes[0];
    let mut ll = compute_low_lim(&tree_edges, &all_nodes, root);
    let mut cutvalues = compute_cut_values(graph, &tree_edges, &ll);

    let max_iter = all_nodes.len() * all_nodes.len();
    for _ in 0..max_iter {
        let leaving = match leave_edge(&cutvalues) {
            Some(e) => e,
            None => break,
        };

        let entering = match enter_edge(graph, leaving, &tree_edges, &rank, &ll) {
            Some(e) => e,
            None => break,
        };

        tree_edges.remove(&leaving);
        let entering_canonical = (entering.0.min(entering.1), entering.0.max(entering.1));
        tree_edges.insert(entering_canonical);

        ll = compute_low_lim(&tree_edges, &all_nodes, root);

        let mut rank_visited: HashSet<NodeIndex> = HashSet::new();
        let mut rank_stack = vec![root];
        rank_visited.insert(root);

        let mut tree_adj: HashMap<NodeIndex, Vec<NodeIndex>> = HashMap::new();
        for &(a, b) in &tree_edges {
            tree_adj.entry(a).or_default().push(b);
            tree_adj.entry(b).or_default().push(a);
        }
        for v in tree_adj.values_mut() {
            v.sort();
        }

        while let Some(node) = rank_stack.pop() {
            if let Some(neighbors) = tree_adj.get(&node) {
                for &nb in neighbors {
                    if rank_visited.insert(nb) {
                        if graph.find_edge(nb, node).is_some() {
                            rank.insert(nb, rank[&node] - 1);
                        } else if graph.find_edge(node, nb).is_some() {
                            rank.insert(nb, rank[&node] + 1);
                        }
                        rank_stack.push(nb);
                    }
                }
            }
        }

        cutvalues = compute_cut_values(graph, &tree_edges, &ll);
    }

    let min_rank = rank.values().copied().min().unwrap_or(0);
    for r in rank.values_mut() {
        *r -= min_rank;
    }

    Some(rank)
}

fn assign_layers(
    node_ids: &[String],
    edges: &[&SimpleEdge],
) -> (Vec<Vec<String>>, HashSet<(String, String)>) {
    let mut pg: DiGraph<String, ()> = DiGraph::new();
    let mut idx_map: HashMap<String, NodeIndex> = HashMap::new();

    for id in node_ids {
        let idx = pg.add_node(id.clone());
        idx_map.insert(id.clone(), idx);
    }

    for edge in edges {
        if let (Some(&from), Some(&to)) = (idx_map.get(&edge.from), idx_map.get(&edge.to)) {
            pg.add_edge(from, to, ());
        }
    }

    let mut reversed = HashSet::new();

    let mut visited = HashSet::new();
    let mut on_stack = HashSet::new();
    let mut back_edges: Vec<(NodeIndex, NodeIndex)> = Vec::new();

    let mut sorted_starts: Vec<_> = idx_map.iter().collect();
    sorted_starts.sort_by_key(|(name, _)| (*name).clone());
    for (_, &start) in &sorted_starts {
        if !visited.contains(&start) {
            dfs_find_back_edges(&pg, start, &mut visited, &mut on_stack, &mut back_edges);
        }
    }

    for (from, to) in &back_edges {
        if let Some(edge_id) = pg.find_edge(*from, *to) {
            pg.remove_edge(edge_id);
            pg.add_edge(*to, *from, ());
            reversed.insert((pg[*from].clone(), pg[*to].clone()));
        }
    }

    let topo = petgraph::algo::toposort(&pg, None)
        .unwrap_or_else(|_| {
            let mut v: Vec<_> = idx_map.values().copied().collect();
            v.sort_by_key(|idx| pg[*idx].clone());
            v
        });

    let layers_map = if let Some(ns_rank) = network_simplex_rank(&pg, &topo) {
        let mut lm: HashMap<NodeIndex, usize> = HashMap::new();
        for (&node, &r) in &ns_rank {
            lm.insert(node, r as usize);
        }
        lm
    } else {
        let mut rank_fwd: HashMap<NodeIndex, usize> = HashMap::new();
        for &node in &topo {
            let max_pred = pg
                .neighbors_directed(node, petgraph::Direction::Incoming)
                .filter_map(|pred| rank_fwd.get(&pred).map(|l| l + 1))
                .max()
                .unwrap_or(0);
            rank_fwd.insert(node, max_pred);
        }
        let max_layer_fwd = rank_fwd.values().copied().max().unwrap_or(0);
        let mut depth_from_bottom: HashMap<NodeIndex, usize> = HashMap::new();
        for &node in topo.iter().rev() {
            let max_succ = pg
                .neighbors_directed(node, petgraph::Direction::Outgoing)
                .filter_map(|succ| depth_from_bottom.get(&succ).map(|d| d + 1))
                .max()
                .unwrap_or(0);
            depth_from_bottom.insert(node, max_succ);
        }
        let mut layers_map: HashMap<NodeIndex, usize> = HashMap::new();
        for &node in &topo {
            let earliest = rank_fwd[&node];
            let latest = max_layer_fwd.saturating_sub(depth_from_bottom[&node]);
            let balanced = (earliest + latest + 1) / 2;
            layers_map.insert(node, balanced.max(earliest).min(latest));
        }
        let valid = pg.edge_indices().all(|e| {
            let (src, tgt) = pg.edge_endpoints(e).unwrap();
            layers_map[&src] < layers_map[&tgt]
        });
        if !valid {
            layers_map = rank_fwd;
        }
        layers_map
    };

    let max_layer = layers_map.values().copied().max().unwrap_or(0);
    let mut layers: Vec<Vec<String>> = vec![Vec::new(); max_layer + 1];
    for (&node, &layer) in &layers_map {
        layers[layer].push(pg[node].clone());
    }
    for layer in &mut layers {
        layer.sort();
    }

    (layers, reversed)
}

fn dfs_find_back_edges(
    graph: &DiGraph<String, ()>,
    node: NodeIndex,
    visited: &mut HashSet<NodeIndex>,
    on_stack: &mut HashSet<NodeIndex>,
    back_edges: &mut Vec<(NodeIndex, NodeIndex)>,
) {
    visited.insert(node);
    on_stack.insert(node);

    for edge in graph.edges(node) {
        let target = edge.target();
        if !visited.contains(&target) {
            dfs_find_back_edges(graph, target, visited, on_stack, back_edges);
        } else if on_stack.contains(&target) {
            back_edges.push((node, target));
        }
    }

    on_stack.remove(&node);
}

fn minimize_crossings_grouped(
    layers: &[Vec<String>],
    edges: &[&SimpleEdge],
    node_groups: Option<&HashMap<String, usize>>,
) -> Vec<Vec<String>> {
    if layers.is_empty() {
        return Vec::new();
    }

    let mut best = layers.to_vec();

    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in edges {
        adj.entry(edge.from.as_str())
            .or_default()
            .push(edge.to.as_str());
        adj.entry(edge.to.as_str())
            .or_default()
            .push(edge.from.as_str());
    }

    let group_of = |id: &str| -> usize {
        node_groups
            .and_then(|g| g.get(id).copied())
            .unwrap_or(usize::MAX)
    };

    // Initial grouping pass: sort each layer by subgraph, preserving
    // relative order within each group
    if node_groups.is_some() {
        for layer in &mut best {
            layer.sort_by_key(|id| group_of(id));
        }
    }

    // Barycentric crossing minimization — sort by (group, barycenter) so
    // subgraph members stay together while minimizing crossings within groups
    for _ in 0..12 {
        for li in 1..best.len() {
            let prev_positions: HashMap<&str, usize> = best[li - 1]
                .iter()
                .enumerate()
                .map(|(i, id)| (id.as_str(), i))
                .collect();

            let mut scored: Vec<(usize, f32, String)> = best[li]
                .iter()
                .map(|id| {
                    let g = group_of(id);
                    let neighbors = adj.get(id.as_str()).cloned().unwrap_or_default();
                    let positions: Vec<f32> = neighbors
                        .iter()
                        .filter_map(|n| prev_positions.get(n).map(|&p| p as f32))
                        .collect();
                    let bary = if positions.is_empty() {
                        f32::MAX
                    } else {
                        positions.iter().sum::<f32>() / positions.len() as f32
                    };
                    (g, bary, id.clone())
                })
                .collect();

            scored.sort_by(|a, b| {
                a.0.cmp(&b.0)
                    .then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            });
            best[li] = scored.into_iter().map(|(_, _, id)| id).collect();
        }

        for li in (0..best.len().saturating_sub(1)).rev() {
            let next_positions: HashMap<&str, usize> = best[li + 1]
                .iter()
                .enumerate()
                .map(|(i, id)| (id.as_str(), i))
                .collect();

            let mut scored: Vec<(usize, f32, String)> = best[li]
                .iter()
                .map(|id| {
                    let g = group_of(id);
                    let neighbors = adj.get(id.as_str()).cloned().unwrap_or_default();
                    let positions: Vec<f32> = neighbors
                        .iter()
                        .filter_map(|n| next_positions.get(n).map(|&p| p as f32))
                        .collect();
                    let bary = if positions.is_empty() {
                        f32::MAX
                    } else {
                        positions.iter().sum::<f32>() / positions.len() as f32
                    };
                    (g, bary, id.clone())
                })
                .collect();

            scored.sort_by(|a, b| {
                a.0.cmp(&b.0)
                    .then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            });
            best[li] = scored.into_iter().map(|(_, _, id)| id).collect();
        }
    }

    // Reorder subgraph groups within each layer by average barycenter of the
    // group, so that groups whose neighbors are on the left appear on the left
    if node_groups.is_some() {
        for li in 0..best.len() {
            let neighbor_positions: HashMap<&str, usize> = if li > 0 {
                best[li - 1]
                    .iter()
                    .enumerate()
                    .map(|(i, id)| (id.as_str(), i))
                    .collect()
            } else if li + 1 < best.len() {
                best[li + 1]
                    .iter()
                    .enumerate()
                    .map(|(i, id)| (id.as_str(), i))
                    .collect()
            } else {
                HashMap::new()
            };

            // Compute average barycenter per group
            let mut group_bary: HashMap<usize, (f32, usize)> = HashMap::new();
            for id in &best[li] {
                let g = group_of(id);
                let neighbors = adj.get(id.as_str()).cloned().unwrap_or_default();
                let positions: Vec<f32> = neighbors
                    .iter()
                    .filter_map(|n| neighbor_positions.get(n).map(|&p| p as f32))
                    .collect();
                if !positions.is_empty() {
                    let avg = positions.iter().sum::<f32>() / positions.len() as f32;
                    let entry = group_bary.entry(g).or_insert((0.0, 0));
                    entry.0 += avg;
                    entry.1 += 1;
                }
            }
            let group_avg: HashMap<usize, f32> = group_bary
                .into_iter()
                .map(|(g, (sum, cnt))| (g, sum / cnt as f32))
                .collect();

            best[li].sort_by(|a, b| {
                let ga = group_of(a);
                let gb = group_of(b);
                let ba = group_avg.get(&ga).copied().unwrap_or(f32::MAX);
                let bb = group_avg.get(&gb).copied().unwrap_or(f32::MAX);
                ba.partial_cmp(&bb)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(ga.cmp(&gb))
            });
        }
    }

    best
}

fn assign_coordinates_grouped(
    layers: &[Vec<String>],
    node_sizes: &HashMap<String, (f32, f32)>,
    direction: Direction,
    node_groups: Option<&HashMap<String, usize>>,
    dummy_nodes: &HashSet<String>,
    edges: &[SimpleEdge],
) -> HashMap<String, (f32, f32)> {
    if let Some(positions) = bk_coordinate_assignment(
        layers, node_sizes, direction, node_groups, dummy_nodes, edges,
    ) {
        return positions;
    }
    assign_coordinates_simple(layers, node_sizes, direction, node_groups, dummy_nodes)
}

fn bk_build_node_positions<'a>(
    layers: &'a [Vec<String>],
) -> HashMap<&'a str, (usize, usize)> {
    let mut pos = HashMap::new();
    for (li, layer) in layers.iter().enumerate() {
        for (pi, id) in layer.iter().enumerate() {
            pos.insert(id.as_str(), (li, pi));
        }
    }
    pos
}

fn bk_build_adjacency<'a>(
    layers: &'a [Vec<String>],
    edges: &[SimpleEdge],
    node_pos: &HashMap<&str, (usize, usize)>,
) -> (HashMap<&'a str, Vec<&'a str>>, HashMap<&'a str, Vec<&'a str>>) {
    let mut upper: HashMap<&'a str, Vec<&'a str>> = HashMap::new();
    let mut lower: HashMap<&'a str, Vec<&'a str>> = HashMap::new();

    for layer in layers {
        for id in layer {
            upper.entry(id.as_str()).or_default();
            lower.entry(id.as_str()).or_default();
        }
    }

    let id_in_layers: HashMap<&str, &'a str> = layers
        .iter()
        .flat_map(|l| l.iter().map(|s| (s.as_str(), s.as_str())))
        .collect();

    for edge in edges {
        let from_str = match id_in_layers.get(edge.from.as_str()) {
            Some(s) => *s,
            None => continue,
        };
        let to_str = match id_in_layers.get(edge.to.as_str()) {
            Some(s) => *s,
            None => continue,
        };
        let from_layer = match node_pos.get(from_str) {
            Some(&(l, _)) => l,
            None => continue,
        };
        let to_layer = match node_pos.get(to_str) {
            Some(&(l, _)) => l,
            None => continue,
        };
        if from_layer + 1 == to_layer {
            upper.entry(to_str).or_default().push(from_str);
            lower.entry(from_str).or_default().push(to_str);
        } else if to_layer + 1 == from_layer {
            upper.entry(from_str).or_default().push(to_str);
            lower.entry(to_str).or_default().push(from_str);
        }
    }

    for list in upper.values_mut() {
        list.sort_by_key(|id| node_pos.get(id).map(|p| p.1).unwrap_or(0));
        list.dedup();
    }
    for list in lower.values_mut() {
        list.sort_by_key(|id| node_pos.get(id).map(|p| p.1).unwrap_or(0));
        list.dedup();
    }

    (upper, lower)
}

fn bk_compute_separation(
    left_id: &str,
    right_id: &str,
    node_sizes: &HashMap<String, (f32, f32)>,
    dummy_nodes: &HashSet<String>,
    node_groups: Option<&HashMap<String, usize>>,
    is_horizontal: bool,
) -> f32 {
    let node_spacing = super::NODE_SPACING.with(|c| c.get());
    let edge_spacing = super::EDGE_SPACING;
    let group_spacing = node_spacing * 3.0;
    let default_size = (super::DEFAULT_NODE_WIDTH, super::DEFAULT_NODE_HEIGHT);

    let left_size = node_sizes.get(left_id).copied().unwrap_or(default_size);
    let right_size = node_sizes.get(right_id).copied().unwrap_or(default_size);

    let left_half = if is_horizontal { left_size.1 } else { left_size.0 } / 2.0;
    let right_half = if is_horizontal { right_size.1 } else { right_size.0 } / 2.0;

    let left_dummy = dummy_nodes.contains(left_id);
    let right_dummy = dummy_nodes.contains(right_id);
    let gap = match (left_dummy, right_dummy) {
        (true, true) => edge_spacing,
        (true, false) | (false, true) => (edge_spacing + node_spacing) / 2.0,
        (false, false) => node_spacing,
    };

    let group_of = |id: &str| -> usize {
        node_groups
            .and_then(|g| g.get(id).copied())
            .unwrap_or(usize::MAX)
    };
    let extra = if group_of(left_id) != group_of(right_id) {
        group_spacing - gap
    } else {
        0.0
    };

    left_half + gap + extra + right_half
}

fn bk_find_type1_conflicts(
    layers: &[Vec<String>],
    upper_neighbors: &HashMap<&str, Vec<&str>>,
    dummy_nodes: &HashSet<String>,
    node_pos: &HashMap<&str, (usize, usize)>,
) -> HashSet<(String, String)> {
    let mut conflicts = HashSet::new();

    for li in 1..layers.len() {
        let prev_layer = &layers[li - 1];
        let curr_layer = &layers[li];

        let mut inner_positions: Vec<usize> = Vec::new();
        for id in curr_layer {
            if !dummy_nodes.contains(id) {
                continue;
            }
            let neighbors = match upper_neighbors.get(id.as_str()) {
                Some(n) => n,
                None => continue,
            };
            for &nb in neighbors {
                if dummy_nodes.contains(nb) {
                    if let Some(&(_, pos)) = node_pos.get(nb) {
                        inner_positions.push(pos);
                    }
                }
            }
        }
        inner_positions.sort();

        if inner_positions.is_empty() {
            continue;
        }

        let mut k0: usize = 0;
        let mut scan_pos = 0usize;

        for (l, id) in curr_layer.iter().enumerate() {
            let k1_candidate = if dummy_nodes.contains(id) {
                let neighbors = upper_neighbors.get(id.as_str()).cloned().unwrap_or_default();
                let mut max_inner = None;
                for nb in &neighbors {
                    if dummy_nodes.contains(*nb) {
                        if let Some(&(_, pos)) = node_pos.get(*nb) {
                            max_inner = Some(max_inner.map_or(pos, |m: usize| m.max(pos)));
                        }
                    }
                }
                max_inner
            } else {
                None
            };

            let k1 = k1_candidate.unwrap_or_else(|| {
                if l == curr_layer.len() - 1 {
                    prev_layer.len().saturating_sub(1)
                } else {
                    return usize::MAX;
                }
            });

            if k1 == usize::MAX {
                continue;
            }

            while scan_pos <= l {
                let scan_id = &curr_layer[scan_pos];
                let neighbors = upper_neighbors.get(scan_id.as_str()).cloned().unwrap_or_default();
                for nb in neighbors {
                    let nb_pos = node_pos.get(nb).map(|p| p.1).unwrap_or(0);
                    if nb_pos < k0 || nb_pos > k1 {
                        let key = if scan_id.as_str() < nb {
                            (scan_id.clone(), nb.to_string())
                        } else {
                            (nb.to_string(), scan_id.clone())
                        };
                        conflicts.insert(key);
                    }
                }
                scan_pos += 1;
            }
            k0 = k1;
        }
    }

    conflicts
}

fn bk_vertical_alignment(
    layers: &[Vec<String>],
    node_pos: &HashMap<&str, (usize, usize)>,
    neighbors: &HashMap<&str, Vec<&str>>,
    conflicts: &HashSet<(String, String)>,
    left_to_right: bool,
    top_to_bottom: bool,
) -> (HashMap<String, String>, HashMap<String, String>) {
    let mut root: HashMap<String, String> = HashMap::new();
    let mut align: HashMap<String, String> = HashMap::new();

    for layer in layers {
        for id in layer {
            root.insert(id.clone(), id.clone());
            align.insert(id.clone(), id.clone());
        }
    }

    let layer_order: Vec<usize> = if top_to_bottom {
        (0..layers.len()).collect()
    } else {
        (0..layers.len()).rev().collect()
    };

    for &li in &layer_order {
        let layer = &layers[li];
        let node_order: Vec<usize> = if left_to_right {
            (0..layer.len()).collect()
        } else {
            (0..layer.len()).rev().collect()
        };

        let mut prev_idx: Option<usize> = None;

        for &ni in &node_order {
            let v = &layer[ni];
            let nbs = match neighbors.get(v.as_str()) {
                Some(n) if !n.is_empty() => n,
                _ => continue,
            };

            let median_indices = if nbs.len() % 2 == 1 {
                vec![nbs.len() / 2]
            } else {
                vec![nbs.len() / 2 - 1, nbs.len() / 2]
            };

            let ordered_medians: Vec<usize> = if left_to_right {
                median_indices
            } else {
                median_indices.into_iter().rev().collect()
            };

            for mi in ordered_medians {
                if align[v] != *v {
                    break;
                }
                let w = nbs[mi];

                let key = if v.as_str() < w {
                    (v.clone(), w.to_string())
                } else {
                    (w.to_string(), v.clone())
                };
                if conflicts.contains(&key) {
                    continue;
                }

                let w_pos = node_pos.get(w).map(|p| p.1).unwrap_or(0);

                let ok = match prev_idx {
                    None => true,
                    Some(pi) => {
                        if left_to_right { w_pos > pi } else { w_pos < pi }
                    }
                };
                if !ok {
                    continue;
                }

                align.insert(w.to_string(), v.clone());
                let root_w = root[w].clone();
                root.insert(v.clone(), root_w.clone());
                align.insert(v.clone(), root_w);
                prev_idx = Some(w_pos);
            }
        }
    }

    (root, align)
}

fn bk_horizontal_compaction(
    layers: &[Vec<String>],
    _node_pos: &HashMap<&str, (usize, usize)>,
    root_map: &HashMap<String, String>,
    _align: &HashMap<String, String>,
    node_sizes: &HashMap<String, (f32, f32)>,
    dummy_nodes: &HashSet<String>,
    node_groups: Option<&HashMap<String, usize>>,
    left_to_right: bool,
    is_horizontal: bool,
) -> HashMap<String, f32> {
    let all_roots: HashSet<&str> = root_map.values().map(|s| s.as_str()).collect();

    let mut root_x: HashMap<&str, f32> = HashMap::new();

    let mut block_constraints: Vec<(&str, &str, f32)> = Vec::new();

    for layer in layers {
        let order: Vec<usize> = if left_to_right {
            (0..layer.len()).collect()
        } else {
            (0..layer.len()).rev().collect()
        };

        for i in 1..order.len() {
            let left_idx = order[i - 1];
            let right_idx = order[i];
            let (left_idx, right_idx) = if left_to_right {
                (left_idx, right_idx)
            } else {
                (right_idx, left_idx)
            };

            let left_id = &layer[left_idx];
            let right_id = &layer[right_idx];

            let left_root = root_map[left_id].as_str();
            let right_root = root_map[right_id].as_str();

            if left_root == right_root {
                continue;
            }

            let sep = bk_compute_separation(
                left_id, right_id, node_sizes, dummy_nodes, node_groups, is_horizontal,
            );

            block_constraints.push((left_root, right_root, sep));
        }
    }

    let mut max_sep: HashMap<(&str, &str), f32> = HashMap::new();
    for &(left, right, sep) in &block_constraints {
        let entry = max_sep.entry((left, right)).or_insert(0.0f32);
        *entry = entry.max(sep);
    }

    let mut predecessors: HashMap<&str, Vec<(&str, f32)>> = HashMap::new();
    let mut successors: HashMap<&str, Vec<(&str, f32)>> = HashMap::new();
    let mut in_degree: HashMap<&str, usize> = HashMap::new();

    for &r in &all_roots {
        predecessors.entry(r).or_default();
        successors.entry(r).or_default();
        in_degree.entry(r).or_insert(0);
    }

    for (&(left, right), &sep) in &max_sep {
        predecessors.entry(right).or_default().push((left, sep));
        successors.entry(left).or_default().push((right, sep));
        *in_degree.entry(right).or_insert(0) += 1;
    }

    let mut queue: Vec<&str> = all_roots
        .iter()
        .filter(|r| in_degree.get(*r).copied().unwrap_or(0) == 0)
        .copied()
        .collect();
    queue.sort();

    let mut topo_order: Vec<&str> = Vec::new();
    while let Some(node) = queue.pop() {
        topo_order.push(node);
        if let Some(succs) = successors.get(node) {
            for &(succ, _) in succs {
                let deg = in_degree.get_mut(succ).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    queue.push(succ);
                    queue.sort();
                }
            }
        }
    }

    for &r in &topo_order {
        let x = predecessors
            .get(r)
            .map(|preds| {
                preds
                    .iter()
                    .filter_map(|&(pred, sep)| root_x.get(pred).map(|&px| px + sep))
                    .fold(0.0f32, f32::max)
            })
            .unwrap_or(0.0);
        root_x.insert(r, x);
    }

    for &r in topo_order.iter().rev() {
        if let Some(succs) = successors.get(r) {
            let min_succ = succs
                .iter()
                .filter_map(|&(succ, sep)| root_x.get(succ).map(|&sx| sx - sep))
                .fold(f32::MAX, f32::min);
            if min_succ < f32::MAX {
                let current = root_x[r];
                if min_succ > current {
                    root_x.insert(r, min_succ);
                }
            }
        }
    }

    let mut xs: HashMap<String, f32> = HashMap::new();
    for layer in layers {
        for id in layer {
            let r = root_map[id].as_str();
            let x = root_x.get(r).copied().unwrap_or(0.0);
            xs.insert(id.clone(), x);
        }
    }

    xs
}

fn bk_balance_alignments(
    all_xs: &[HashMap<String, f32>; 4],
    all_node_ids: &[&str],
) -> HashMap<String, f32> {
    let mut widths: [f32; 4] = [0.0; 4];
    let mut mins: [f32; 4] = [f32::MAX; 4];
    let mut maxs: [f32; 4] = [f32::MIN; 4];

    for (i, xs) in all_xs.iter().enumerate() {
        for &x in xs.values() {
            mins[i] = mins[i].min(x);
            maxs[i] = maxs[i].max(x);
        }
        widths[i] = maxs[i] - mins[i];
    }

    let smallest = widths
        .iter()
        .enumerate()
        .min_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap_or(0);

    let mut shifted: [HashMap<String, f32>; 4] = Default::default();
    for (i, xs) in all_xs.iter().enumerate() {
        let delta = if i % 2 == 0 {
            mins[smallest] - mins[i]
        } else {
            maxs[smallest] - maxs[i]
        };
        for (id, &x) in xs {
            shifted[i].insert(id.clone(), x + delta);
        }
    }

    let mut result: HashMap<String, f32> = HashMap::new();
    for &id in all_node_ids {
        let mut vals: Vec<f32> = (0..4)
            .filter_map(|i| shifted[i].get(id).copied())
            .collect();
        vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = if vals.len() >= 4 {
            (vals[1] + vals[2]) / 2.0
        } else if vals.len() >= 2 {
            (vals[0] + vals[vals.len() - 1]) / 2.0
        } else {
            vals.first().copied().unwrap_or(0.0)
        };
        result.insert(id.to_string(), median);
    }

    result
}

fn bk_build_final_positions(
    layers: &[Vec<String>],
    node_sizes: &HashMap<String, (f32, f32)>,
    cross_coords: &HashMap<String, f32>,
    direction: Direction,
) -> HashMap<String, (f32, f32)> {
    let layer_spacing = super::LAYER_SPACING.with(|c| c.get());
    let default_size = (super::DEFAULT_NODE_WIDTH, super::DEFAULT_NODE_HEIGHT);
    let is_horizontal = matches!(direction, Direction::LR | Direction::RL);

    let max_main_per_row: Vec<f32> = layers
        .iter()
        .map(|row| {
            row.iter()
                .map(|id| {
                    let (w, h) = node_sizes.get(id).copied().unwrap_or(default_size);
                    if is_horizontal { w } else { h }
                })
                .fold(0.0f32, f32::max)
        })
        .collect();

    let total_main: f32 =
        max_main_per_row.iter().sum::<f32>() + (layers.len().saturating_sub(1) as f32) * layer_spacing;

    let mut positions = HashMap::new();
    let mut main_cursor = 0.0f32;

    for (row_idx, row) in layers.iter().enumerate() {
        let row_main_size = max_main_per_row[row_idx];
        let main_pos = main_cursor + row_main_size / 2.0;

        for id in row {
            let cross_pos = cross_coords.get(id).copied().unwrap_or(0.0);

            let (x, y) = match direction {
                Direction::TD | Direction::TB => (cross_pos, main_pos),
                Direction::BT => (cross_pos, total_main - main_pos),
                Direction::LR => (main_pos, cross_pos),
                Direction::RL => (total_main - main_pos, cross_pos),
            };

            positions.insert(id.clone(), (x, y));
        }

        main_cursor += row_main_size + layer_spacing;
    }

    positions
}

fn bk_coordinate_assignment(
    layers: &[Vec<String>],
    node_sizes: &HashMap<String, (f32, f32)>,
    direction: Direction,
    node_groups: Option<&HashMap<String, usize>>,
    dummy_nodes: &HashSet<String>,
    edges: &[SimpleEdge],
) -> Option<HashMap<String, (f32, f32)>> {
    if layers.is_empty() {
        return Some(HashMap::new());
    }

    let is_horizontal = matches!(direction, Direction::LR | Direction::RL);

    let node_pos = bk_build_node_positions(layers);
    let (upper_neighbors, lower_neighbors) = bk_build_adjacency(layers, edges, &node_pos);

    let conflicts = bk_find_type1_conflicts(layers, &upper_neighbors, dummy_nodes, &node_pos);

    let orientations: [(bool, bool); 4] = [
        (true, true),   // UL: left-to-right, top-to-bottom, predecessors
        (false, true),  // UR: right-to-left, top-to-bottom, predecessors
        (true, false),  // DL: left-to-right, bottom-to-top, successors
        (false, false), // DR: right-to-left, bottom-to-top, successors
    ];

    let mut all_xs: [HashMap<String, f32>; 4] = Default::default();

    for (i, &(left_to_right, top_to_bottom)) in orientations.iter().enumerate() {
        let use_upper = top_to_bottom;
        let nbrs = if use_upper { &upper_neighbors } else { &lower_neighbors };

        let (root_map, align_map) =
            bk_vertical_alignment(layers, &node_pos, nbrs, &conflicts, left_to_right, top_to_bottom);

        all_xs[i] = bk_horizontal_compaction(
            layers,
            &node_pos,
            &root_map,
            &align_map,
            node_sizes,
            dummy_nodes,
            node_groups,
            left_to_right,
            is_horizontal,
        );
    }

    let all_node_ids: Vec<&str> = layers
        .iter()
        .flat_map(|l| l.iter().map(|s| s.as_str()))
        .collect();

    let mut final_cross = bk_balance_alignments(&all_xs, &all_node_ids);

    let default_size = (super::DEFAULT_NODE_WIDTH, super::DEFAULT_NODE_HEIGHT);
    let margin = 20.0f32;
    let mut min_edge = f32::MAX;
    for &id in &all_node_ids {
        if let Some(&cx) = final_cross.get(id) {
            let (w, h) = node_sizes.get(id).copied().unwrap_or(default_size);
            let half = if is_horizontal { h } else { w } / 2.0;
            min_edge = min_edge.min(cx - half);
        }
    }
    if min_edge < margin {
        let shift = margin - min_edge;
        for v in final_cross.values_mut() {
            *v += shift;
        }
    }

    let positions = bk_build_final_positions(layers, node_sizes, &final_cross, direction);

    if positions
        .values()
        .any(|(x, y)| x.is_nan() || y.is_nan() || x.is_infinite() || y.is_infinite())
    {
        return None;
    }

    Some(positions)
}

fn assign_coordinates_simple(
    layers: &[Vec<String>],
    node_sizes: &HashMap<String, (f32, f32)>,
    direction: Direction,
    node_groups: Option<&HashMap<String, usize>>,
    dummy_nodes: &HashSet<String>,
) -> HashMap<String, (f32, f32)> {
    let layer_spacing = super::LAYER_SPACING.with(|c| c.get());
    let node_spacing = super::NODE_SPACING.with(|c| c.get());
    let edge_spacing = super::EDGE_SPACING;
    let group_spacing = node_spacing * 3.0;
    let default_size = (super::DEFAULT_NODE_WIDTH, super::DEFAULT_NODE_HEIGHT);
    let is_horizontal = matches!(direction, Direction::LR | Direction::RL);

    let group_of = |id: &str| -> usize {
        node_groups
            .and_then(|g| g.get(id).copied())
            .unwrap_or(usize::MAX)
    };

    let spacing_between = |prev_id: &str, curr_id: &str| -> f32 {
        let prev_dummy = dummy_nodes.contains(prev_id);
        let curr_dummy = dummy_nodes.contains(curr_id);
        match (prev_dummy, curr_dummy) {
            (true, true) => edge_spacing,
            (true, false) | (false, true) => (edge_spacing + node_spacing) / 2.0,
            (false, false) => node_spacing,
        }
    };

    let rows: Vec<Vec<String>> = layers.to_vec();

    let mut positions: HashMap<String, (f32, f32)> = HashMap::new();

    let mut max_main_per_row: Vec<f32> = Vec::new();
    let mut row_cross_widths: Vec<f32> = Vec::new();

    for row in &rows {
        let mut max_main: f32 = 0.0;
        let mut total_cross: f32 = 0.0;
        let mut prev_group: Option<usize> = None;
        let mut prev_id: Option<&str> = None;
        for id in row {
            let (w, h) = node_sizes.get(id).copied().unwrap_or(default_size);
            let (cross, main) = if is_horizontal { (h, w) } else { (w, h) };
            max_main = max_main.max(main);

            let g = group_of(id);
            if let Some(pg) = prev_group {
                if pg != g {
                    total_cross += group_spacing;
                } else {
                    total_cross += spacing_between(prev_id.unwrap_or(""), id);
                }
            }
            total_cross += cross;
            prev_group = Some(g);
            prev_id = Some(id);
        }
        max_main_per_row.push(max_main);
        row_cross_widths.push(total_cross);
    }

    let max_cross_width = row_cross_widths.iter().cloned().fold(0.0f32, f32::max);

    let mut main_cursor = 0.0f32;
    for (row_idx, row) in rows.iter().enumerate() {
        let row_cross = row_cross_widths[row_idx];
        let offset = (max_cross_width - row_cross) / 2.0;
        let row_main_size = max_main_per_row[row_idx];
        let main_pos = main_cursor + row_main_size / 2.0;

        let mut cross_cursor = offset;
        let mut prev_group: Option<usize> = None;
        let mut prev_id: Option<&str> = None;
        for id in row {
            let (w, h) = node_sizes.get(id).copied().unwrap_or(default_size);
            let cross_size = if is_horizontal { h } else { w };

            let g = group_of(id);
            if let Some(pg) = prev_group {
                if pg != g {
                    cross_cursor += group_spacing;
                } else {
                    cross_cursor += spacing_between(prev_id.unwrap_or(""), id);
                }
            }
            prev_group = Some(g);
            prev_id = Some(id);

            let cross_pos = cross_cursor + cross_size / 2.0;

            let (x, y) = match direction {
                Direction::TD | Direction::TB => (cross_pos, main_pos),
                Direction::BT => {
                    let total_main: f32 = max_main_per_row.iter().sum::<f32>()
                        + (rows.len().saturating_sub(1) as f32) * layer_spacing;
                    (cross_pos, total_main - main_pos)
                }
                Direction::LR => (main_pos, cross_pos),
                Direction::RL => {
                    let total_main: f32 = max_main_per_row.iter().sum::<f32>()
                        + (rows.len().saturating_sub(1) as f32) * layer_spacing;
                    (total_main - main_pos, cross_pos)
                }
            };

            positions.insert(id.clone(), (x, y));
            cross_cursor += cross_size;
        }

        main_cursor += row_main_size + layer_spacing;
    }

    positions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    fn edges_from_graph(g: &crate::diagram::DiagramGraph) -> Vec<SimpleEdge> {
        g.edges
            .iter()
            .map(|e| SimpleEdge {
                from: e.from.clone(),
                to: e.to.clone(),
            })
            .collect()
    }

    #[test]
    fn test_linear_layers() {
        let input = "graph TD\n    A --> B\n    B --> C\n    C --> D";
        let g = parser::mermaid::parse(input).unwrap();
        let node_ids: Vec<String> = g.nodes.keys().cloned().collect();
        let edges = edges_from_graph(&g);
        let edge_refs: Vec<&SimpleEdge> = edges.iter().collect();
        let (layers, reversed) = assign_layers(&node_ids, &edge_refs);
        assert!(reversed.is_empty());
        assert!(layers.len() >= 4);
    }

    #[test]
    fn test_diamond_layers() {
        let input = "graph TD\n    A --> B\n    A --> C\n    B --> D\n    C --> D";
        let g = parser::mermaid::parse(input).unwrap();
        let node_ids: Vec<String> = g.nodes.keys().cloned().collect();
        let edges = edges_from_graph(&g);
        let edge_refs: Vec<&SimpleEdge> = edges.iter().collect();
        let (layers, _) = assign_layers(&node_ids, &edge_refs);
        assert!(layers.len() >= 3);
    }

    #[test]
    fn test_cycle_breaking() {
        let input = "graph TD\n    A --> B\n    B --> C\n    C --> A";
        let g = parser::mermaid::parse(input).unwrap();
        let node_ids: Vec<String> = g.nodes.keys().cloned().collect();
        let edges = edges_from_graph(&g);
        let edge_refs: Vec<&SimpleEdge> = edges.iter().collect();
        let (layers, reversed) = assign_layers(&node_ids, &edge_refs);
        assert!(!reversed.is_empty(), "should have reversed an edge");
        assert!(!layers.is_empty());
    }

    #[test]
    fn test_coordinate_assignment() {
        let input = "graph TD\n    A --> B\n    A --> C";
        let g = parser::mermaid::parse(input).unwrap();
        let node_ids: Vec<String> = g.nodes.keys().cloned().collect();
        let edges = edges_from_graph(&g);
        let result = layout_nodes_grouped(
            &node_ids,
            &[("A".into(), (120.0, 40.0)), ("B".into(), (120.0, 40.0)), ("C".into(), (120.0, 40.0))]
                .into_iter()
                .collect(),
            &edges,
            Direction::TD,
            None,
        );
        assert_eq!(result.positions.len(), 3);
        for (_, (x, y)) in &result.positions {
            assert!(*x >= 0.0);
            assert!(*y >= 0.0);
        }
    }

    #[test]
    fn test_ns_linear() {
        let mut pg: DiGraph<String, ()> = DiGraph::new();
        let a = pg.add_node("A".into());
        let b = pg.add_node("B".into());
        let c = pg.add_node("C".into());
        let d = pg.add_node("D".into());
        pg.add_edge(a, b, ());
        pg.add_edge(b, c, ());
        pg.add_edge(c, d, ());
        let topo = petgraph::algo::toposort(&pg, None).unwrap();
        let rank = network_simplex_rank(&pg, &topo).unwrap();
        assert_eq!(rank[&a], 0);
        assert_eq!(rank[&b], 1);
        assert_eq!(rank[&c], 2);
        assert_eq!(rank[&d], 3);
    }

    #[test]
    fn test_ns_diamond() {
        let mut pg: DiGraph<String, ()> = DiGraph::new();
        let a = pg.add_node("A".into());
        let b = pg.add_node("B".into());
        let c = pg.add_node("C".into());
        let d = pg.add_node("D".into());
        pg.add_edge(a, b, ());
        pg.add_edge(a, c, ());
        pg.add_edge(b, d, ());
        pg.add_edge(c, d, ());
        let topo = petgraph::algo::toposort(&pg, None).unwrap();
        let rank = network_simplex_rank(&pg, &topo).unwrap();
        assert_eq!(rank[&a], 0);
        assert_eq!(rank[&b], 1);
        assert_eq!(rank[&c], 1);
        assert_eq!(rank[&d], 2);
    }

    #[test]
    fn test_ns_single_node() {
        let mut pg: DiGraph<String, ()> = DiGraph::new();
        let a = pg.add_node("A".into());
        let topo = vec![a];
        let rank = network_simplex_rank(&pg, &topo).unwrap();
        assert_eq!(rank[&a], 0);
    }

    #[test]
    fn test_ns_no_edges() {
        let mut pg: DiGraph<String, ()> = DiGraph::new();
        let a = pg.add_node("A".into());
        let b = pg.add_node("B".into());
        let topo = vec![a, b];
        let rank = network_simplex_rank(&pg, &topo).unwrap();
        assert_eq!(rank[&a], 0);
        assert_eq!(rank[&b], 0);
    }

    #[test]
    fn test_ns_all_nodes_present() {
        let input = "graph TD\n    A --> B\n    A --> C\n    B --> D\n    C --> D\n    D --> E";
        let g = parser::mermaid::parse(input).unwrap();
        let node_ids: Vec<String> = g.nodes.keys().cloned().collect();
        let edges = edges_from_graph(&g);
        let edge_refs: Vec<&SimpleEdge> = edges.iter().collect();
        let (layers, _) = assign_layers(&node_ids, &edge_refs);
        let all_laid_out: HashSet<&str> = layers.iter()
            .flat_map(|l| l.iter().map(|s| s.as_str()))
            .collect();
        for id in &node_ids {
            assert!(all_laid_out.contains(id.as_str()), "missing node: {id}");
        }
    }

    #[test]
    fn test_ns_edge_constraints() {
        let input = "graph TD\n    A --> B\n    B --> C\n    A --> C";
        let g = parser::mermaid::parse(input).unwrap();
        let node_ids: Vec<String> = g.nodes.keys().cloned().collect();
        let edges = edges_from_graph(&g);
        let edge_refs: Vec<&SimpleEdge> = edges.iter().collect();
        let (layers, _) = assign_layers(&node_ids, &edge_refs);
        let node_layer: HashMap<&str, usize> = layers.iter()
            .enumerate()
            .flat_map(|(li, l)| l.iter().map(move |s| (s.as_str(), li)))
            .collect();
        for e in &edges {
            assert!(
                node_layer[e.from.as_str()] < node_layer[e.to.as_str()],
                "edge {}->{} violates layer ordering", e.from, e.to,
            );
        }
    }

    #[test]
    fn test_bk_linear_chain() {
        let input = "graph TD\n    A --> B\n    B --> C\n    C --> D";
        let g = parser::mermaid::parse(input).unwrap();
        let node_ids: Vec<String> = g.nodes.keys().cloned().collect();
        let sizes: HashMap<String, (f32, f32)> = node_ids.iter()
            .map(|id| (id.clone(), (120.0, 40.0)))
            .collect();
        let edges = edges_from_graph(&g);
        let result = layout_nodes_grouped(&node_ids, &sizes, &edges, Direction::TD, None);
        let xs: Vec<f32> = ["A", "B", "C", "D"]
            .iter()
            .filter_map(|id| result.positions.get(*id).map(|p| p.0))
            .collect();
        assert_eq!(xs.len(), 4, "all nodes should be positioned");
        for i in 1..xs.len() {
            assert!(
                (xs[0] - xs[i]).abs() < 1.0,
                "linear chain nodes should be vertically aligned, got {:?}",
                xs,
            );
        }
    }

    #[test]
    fn test_bk_diamond() {
        let input = "graph TD\n    A --> B\n    A --> C\n    B --> D\n    C --> D";
        let g = parser::mermaid::parse(input).unwrap();
        let node_ids: Vec<String> = g.nodes.keys().cloned().collect();
        let sizes: HashMap<String, (f32, f32)> = node_ids.iter()
            .map(|id| (id.clone(), (120.0, 40.0)))
            .collect();
        let edges = edges_from_graph(&g);
        let result = layout_nodes_grouped(&node_ids, &sizes, &edges, Direction::TD, None);
        assert_eq!(result.positions.len(), 4);
        let ax = result.positions["A"].0;
        let bx = result.positions["B"].0;
        let cx = result.positions["C"].0;
        let dx = result.positions["D"].0;
        assert!(
            (bx - cx).abs() > 10.0,
            "B and C should be separated horizontally",
        );
        let mid = (bx + cx) / 2.0;
        assert!(
            (ax - mid).abs() < 50.0,
            "A should be near midpoint of B and C: A={ax}, mid={mid}",
        );
        assert!(
            (dx - mid).abs() < 50.0,
            "D should be near midpoint of B and C: D={dx}, mid={mid}",
        );
    }

    #[test]
    fn test_bk_all_directions() {
        let input = "graph TD\n    A --> B\n    B --> C";
        let g = parser::mermaid::parse(input).unwrap();
        let node_ids: Vec<String> = g.nodes.keys().cloned().collect();
        let sizes: HashMap<String, (f32, f32)> = node_ids.iter()
            .map(|id| (id.clone(), (120.0, 40.0)))
            .collect();
        let edges = edges_from_graph(&g);

        for dir in [Direction::TD, Direction::BT, Direction::LR, Direction::RL] {
            let result = layout_nodes_grouped(&node_ids, &sizes, &edges, dir, None);
            assert_eq!(result.positions.len(), 3, "all nodes present for {:?}", dir);
            for (id, (x, y)) in &result.positions {
                assert!(
                    !x.is_nan() && !y.is_nan() && !x.is_infinite() && !y.is_infinite(),
                    "invalid position for {} in {:?}: ({}, {})", id, dir, x, y,
                );
            }
        }
    }

    #[test]
    fn test_bk_single_node() {
        let input = "graph TD\n    A[Hello]";
        let g = parser::mermaid::parse(input).unwrap();
        let node_ids: Vec<String> = g.nodes.keys().cloned().collect();
        let sizes: HashMap<String, (f32, f32)> = node_ids.iter()
            .map(|id| (id.clone(), (120.0, 40.0)))
            .collect();
        let edges = edges_from_graph(&g);
        let result = layout_nodes_grouped(&node_ids, &sizes, &edges, Direction::TD, None);
        assert_eq!(result.positions.len(), 1);
    }

    #[test]
    fn test_bk_wide_graph() {
        let input = "graph TD\n    A --> B\n    A --> C\n    A --> D\n    A --> E\n    B --> F\n    C --> F\n    D --> F\n    E --> F";
        let g = parser::mermaid::parse(input).unwrap();
        let node_ids: Vec<String> = g.nodes.keys().cloned().collect();
        let sizes: HashMap<String, (f32, f32)> = node_ids.iter()
            .map(|id| (id.clone(), (120.0, 40.0)))
            .collect();
        let edges = edges_from_graph(&g);
        let result = layout_nodes_grouped(&node_ids, &sizes, &edges, Direction::TD, None);
        assert_eq!(result.positions.len(), 6);
        for (_, (x, y)) in &result.positions {
            assert!(!x.is_nan() && !y.is_nan());
        }
    }
}
