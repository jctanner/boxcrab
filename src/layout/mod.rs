mod sugiyama;

use crate::diagram::{ArrowheadType, ClassField, ClassMethod, DiagramGraph, Direction, EdgeType, NearPosition, NodeShape, SqlColumn, StyleProps};
use std::collections::HashMap;
use sugiyama::SimpleEdge;

#[derive(Debug, Clone)]
pub struct LayoutNode {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub label: String,
    pub shape: NodeShape,
    pub style: StyleProps,
    pub class_fields: Vec<ClassField>,
    pub class_methods: Vec<ClassMethod>,
    pub sql_columns: Vec<SqlColumn>,
    pub tooltip: Option<String>,
    pub link: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LayoutEdge {
    pub points: Vec<[f32; 2]>,
    pub control_points: Option<[[f32; 2]; 2]>,
    pub edge_type: EdgeType,
    pub label: Option<String>,
    pub label_pos: Option<[f32; 2]>,
    pub reversed: bool,
    pub src_arrowhead: Option<ArrowheadType>,
    pub dst_arrowhead: Option<ArrowheadType>,
    pub style: StyleProps,
}

#[derive(Debug, Clone)]
pub struct LayoutSubgraph {
    pub title: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone)]
pub struct LayoutResult {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub subgraphs: Vec<LayoutSubgraph>,
    pub total_width: f32,
    pub total_height: f32,
}

const DEFAULT_NODE_WIDTH: f32 = 120.0;
const DEFAULT_NODE_HEIGHT: f32 = 40.0;
const NODE_PADDING_H: f32 = 24.0;
const NODE_PADDING_V: f32 = 16.0;
thread_local! {
    pub(crate) static LAYER_SPACING: std::cell::Cell<f32> = const { std::cell::Cell::new(80.0) };
    pub(crate) static NODE_SPACING: std::cell::Cell<f32> = const { std::cell::Cell::new(150.0) };
}
const EDGE_SPACING: f32 = 30.0;
const SUBGRAPH_PADDING: f32 = 20.0;
const SUBGRAPH_TITLE_HEIGHT: f32 = 20.0;


pub fn compute_layout(
    graph: &DiagramGraph,
    measured_sizes: Option<&HashMap<String, egui::Vec2>>,
) -> Result<LayoutResult, String> {
    use std::collections::HashSet;

    let has_edge_labels = graph.edges.iter().any(|e| e.label.is_some());
    let default_layer = if has_edge_labels { 140.0 } else { 80.0 };
    LAYER_SPACING.with(|c| c.set(graph.layer_spacing.unwrap_or(default_layer)));
    NODE_SPACING.with(|c| c.set(graph.node_spacing.unwrap_or(150.0)));

    let mut all_node_ids: Vec<String> = graph.nodes.keys().cloned().collect();
    all_node_ids.sort();
    if all_node_ids.is_empty() {
        return Ok(LayoutResult {
            nodes: Vec::new(),
            edges: Vec::new(),
            subgraphs: Vec::new(),
            total_width: 0.0,
            total_height: 0.0,
        });
    }

    let node_sizes = compute_node_sizes(graph, measured_sizes);
    let all_edges: Vec<SimpleEdge> = graph
        .edges
        .iter()
        .map(|e| SimpleEdge {
            from: e.from.clone(),
            to: e.to.clone(),
        })
        .collect();

    // Build subgraph hierarchy
    let n_sg = graph.subgraphs.len();
    let node_sets: Vec<HashSet<&str>> = graph
        .subgraphs
        .iter()
        .map(|sg| sg.node_ids.iter().map(|s| s.as_str()).collect())
        .collect();

    // Find parent of each subgraph (smallest superset)
    let mut sg_parent: Vec<Option<usize>> = vec![None; n_sg];
    for i in 0..n_sg {
        let mut best: Option<usize> = None;
        let mut best_size = usize::MAX;
        for j in 0..n_sg {
            if i != j
                && node_sets[j].is_superset(&node_sets[i])
                && node_sets[j].len() > node_sets[i].len()
                && node_sets[j].len() < best_size
            {
                best_size = node_sets[j].len();
                best = Some(j);
            }
        }
        sg_parent[i] = best;
    }

    let mut sg_children: Vec<Vec<usize>> = vec![Vec::new(); n_sg];
    let mut root_sgs: Vec<usize> = Vec::new();
    for i in 0..n_sg {
        match sg_parent[i] {
            Some(p) => sg_children[p].push(i),
            None => root_sgs.push(i),
        }
    }

    // Direct nodes of each subgraph (not in any child subgraph)
    let mut sg_direct_nodes: Vec<Vec<String>> = Vec::with_capacity(n_sg);
    for i in 0..n_sg {
        let child_nodes: HashSet<&str> = sg_children[i]
            .iter()
            .flat_map(|&c| node_sets[c].iter().copied())
            .collect();
        let direct: Vec<String> = graph.subgraphs[i]
            .node_ids
            .iter()
            .filter(|nid| !child_nodes.contains(nid.as_str()))
            .cloned()
            .collect();
        sg_direct_nodes.push(direct);
    }

    // Nodes not in any subgraph
    let all_sg_nodes: HashSet<&str> = graph
        .subgraphs
        .iter()
        .flat_map(|sg| sg.node_ids.iter().map(|s| s.as_str()))
        .collect();
    let root_loose: Vec<String> = all_node_ids
        .iter()
        .filter(|id| !all_sg_nodes.contains(id.as_str()))
        .cloned()
        .collect();

    // Bottom-up: layout each subgraph, leaves first
    let topo_order = {
        let mut order = Vec::new();
        let mut visited = HashSet::new();
        fn visit(
            idx: usize,
            children: &[Vec<usize>],
            visited: &mut HashSet<usize>,
            order: &mut Vec<usize>,
        ) {
            if visited.contains(&idx) {
                return;
            }
            visited.insert(idx);
            for &c in &children[idx] {
                visit(c, children, visited, order);
            }
            order.push(idx);
        }
        for &r in &root_sgs {
            visit(r, &sg_children, &mut visited, &mut order);
        }
        order
    };

    let mut sg_layouts: Vec<Option<sugiyama::SubgraphLayout>> = vec![None; n_sg];
    let mut sg_compound_sizes: HashMap<String, (f32, f32)> = HashMap::new();

    let compound_id = |idx: usize| format!("__sg_{idx}");

    for &sg_idx in &topo_order {
        let mut level_ids: Vec<String> = sg_direct_nodes[sg_idx].clone();
        let mut level_sizes: HashMap<String, (f32, f32)> = HashMap::new();

        for id in &sg_direct_nodes[sg_idx] {
            if let Some(&sz) = node_sizes.get(id) {
                level_sizes.insert(id.clone(), sz);
            }
        }

        // Add child subgraphs as compound nodes
        for &child_idx in &sg_children[sg_idx] {
            let cid = compound_id(child_idx);
            if let Some(&sz) = sg_compound_sizes.get(&cid) {
                level_ids.push(cid.clone());
                level_sizes.insert(cid.clone(), sz);
            }
        }

        // Collect edges at this level
        let level_edges = collect_level_edges(
            &all_edges,
            &sg_direct_nodes[sg_idx],
            &sg_children[sg_idx],
            &graph.subgraphs,
        );

        let sg_def = &graph.subgraphs[sg_idx];
        let is_grid = sg_def.grid_rows.is_some() || sg_def.grid_columns.is_some();

        let sub_layout = if is_grid {
            layout_grid(
                &level_ids,
                &level_sizes,
                sg_def.grid_rows,
                sg_def.grid_columns,
                sg_def.grid_gap.unwrap_or(20.0),
            )
        } else {
            sugiyama::layout_nodes_grouped(
                &level_ids,
                &level_sizes,
                &level_edges,
                graph.direction,
                None,
            )
        };

        let cid = compound_id(sg_idx);
        let mut max_x = 0.0f32;
        let mut max_y = 0.0f32;
        for id in &level_ids {
            if let Some(&(x, y)) = sub_layout.positions.get(id.as_str()) {
                let (w, h) = level_sizes.get(id).copied().unwrap_or((DEFAULT_NODE_WIDTH, DEFAULT_NODE_HEIGHT));
                max_x = max_x.max(x + w / 2.0);
                max_y = max_y.max(y + h / 2.0);
            }
        }
        let sg_w = max_x + 2.0 * SUBGRAPH_PADDING;
        let sg_h = max_y + 2.0 * SUBGRAPH_PADDING + SUBGRAPH_TITLE_HEIGHT;
        sg_compound_sizes.insert(cid, (sg_w, sg_h));
        sg_layouts[sg_idx] = Some(sub_layout);
    }

    // Root-level layout
    let mut root_ids: Vec<String> = root_loose.clone();
    let mut root_sizes: HashMap<String, (f32, f32)> = HashMap::new();

    for id in &root_loose {
        if let Some(&sz) = node_sizes.get(id) {
            root_sizes.insert(id.clone(), sz);
        }
    }

    for &sg_idx in &root_sgs {
        let cid = compound_id(sg_idx);
        if let Some(&sz) = sg_compound_sizes.get(&cid) {
            root_ids.push(cid.clone());
            root_sizes.insert(cid.clone(), sz);
        }
    }

    let root_edges = collect_level_edges(&all_edges, &root_loose, &root_sgs, &graph.subgraphs);
    let root_layout = sugiyama::layout_nodes_grouped(
        &root_ids,
        &root_sizes,
        &root_edges,
        graph.direction,
        None,
    );

    // Top-down: resolve absolute positions
    let mut final_positions: HashMap<String, (f32, f32)> = HashMap::new();
    let mut subgraph_boxes: Vec<LayoutSubgraph> = Vec::new();
    let mut all_waypoints: HashMap<(String, String), Vec<(f32, f32)>> = HashMap::new();

    for id in &root_loose {
        if let Some(&pos) = root_layout.positions.get(id.as_str()) {
            final_positions.insert(id.clone(), pos);
        }
    }

    for ((from, to), wps) in &root_layout.edge_waypoints {
        all_waypoints.insert((from.clone(), to.clone()), wps.clone());
    }

    fn place_sg(
        sg_idx: usize,
        offset_x: f32,
        offset_y: f32,
        sg_direct_nodes: &[Vec<String>],
        sg_children: &[Vec<usize>],
        sg_layouts: &[Option<sugiyama::SubgraphLayout>],
        sg_compound_sizes: &HashMap<String, (f32, f32)>,
        final_positions: &mut HashMap<String, (f32, f32)>,
        subgraph_boxes: &mut Vec<LayoutSubgraph>,
        all_waypoints: &mut HashMap<(String, String), Vec<(f32, f32)>>,
        subgraphs: &[crate::diagram::SubgraphDef],
    ) {
        let layout = match &sg_layouts[sg_idx] {
            Some(l) => l,
            None => return,
        };

        for id in &sg_direct_nodes[sg_idx] {
            if let Some(&(rx, ry)) = layout.positions.get(id.as_str()) {
                final_positions.insert(id.clone(), (rx + offset_x, ry + offset_y));
            }
        }

        for ((from, to), wps) in &layout.edge_waypoints {
            let offset_wps: Vec<(f32, f32)> = wps
                .iter()
                .map(|&(wx, wy)| (wx + offset_x, wy + offset_y))
                .collect();
            all_waypoints.insert((from.clone(), to.clone()), offset_wps);
        }

        for &child_idx in &sg_children[sg_idx] {
            let cid = format!("__sg_{child_idx}");
            if let Some(&(cx, cy)) = layout.positions.get(cid.as_str()) {
                let (cw, ch) = sg_compound_sizes[&cid];
                let child_ox = offset_x + cx - cw / 2.0 + SUBGRAPH_PADDING;
                let child_oy = offset_y + cy - ch / 2.0 + SUBGRAPH_PADDING + SUBGRAPH_TITLE_HEIGHT;

                place_sg(
                    child_idx,
                    child_ox,
                    child_oy,
                    sg_direct_nodes,
                    sg_children,
                    sg_layouts,
                    sg_compound_sizes,
                    final_positions,
                    subgraph_boxes,
                    all_waypoints,
                    subgraphs,
                );

                subgraph_boxes.push(LayoutSubgraph {
                    title: subgraphs[child_idx].title.clone(),
                    x: offset_x + cx - cw / 2.0,
                    y: offset_y + cy - ch / 2.0,
                    width: cw,
                    height: ch,
                });
            }
        }
    }

    for &sg_idx in &root_sgs {
        let cid = compound_id(sg_idx);
        if let Some(&(cx, cy)) = root_layout.positions.get(cid.as_str()) {
            let (sw, sh) = sg_compound_sizes[&cid];
            let inner_x = cx - sw / 2.0 + SUBGRAPH_PADDING;
            let inner_y = cy - sh / 2.0 + SUBGRAPH_PADDING + SUBGRAPH_TITLE_HEIGHT;

            place_sg(
                sg_idx,
                inner_x,
                inner_y,
                &sg_direct_nodes,
                &sg_children,
                &sg_layouts,
                &sg_compound_sizes,
                &mut final_positions,
                &mut subgraph_boxes,
                &mut all_waypoints,
                &graph.subgraphs,
            );

            subgraph_boxes.push(LayoutSubgraph {
                title: graph.subgraphs[sg_idx].title.clone(),
                x: cx - sw / 2.0,
                y: cy - sh / 2.0,
                width: sw,
                height: sh,
            });
        }
    }

    // Build layout nodes
    let mut layout_nodes = Vec::new();
    for (id, node) in &graph.nodes {
        if let Some(&(x, y)) = final_positions.get(id) {
            let (w, h) = node_sizes[id];
            let style = resolve_style(graph, id);
            layout_nodes.push(LayoutNode {
                id: id.clone(),
                x,
                y,
                width: w,
                height: h,
                label: node.label.clone(),
                shape: node.shape,
                style,
                class_fields: node.class_fields.clone(),
                class_methods: node.class_methods.clone(),
                sql_columns: node.sql_columns.clone(),
                tooltip: node.tooltip.clone(),
                link: node.link.clone(),
            });
        }
    }

    // Build layout edges with port spreading
    let node_shapes: HashMap<String, NodeShape> = graph.nodes.iter()
        .map(|(id, node)| (id.clone(), node.shape))
        .collect();
    let layout_edges = build_edges_with_port_spreading(
        &graph.edges,
        &final_positions,
        &node_sizes,
        &node_shapes,
        graph.direction,
        &all_waypoints,
    );

    let mut total_width = layout_nodes
        .iter()
        .map(|n| n.x + n.width / 2.0)
        .fold(0.0f32, f32::max)
        + 50.0;
    let mut total_height = layout_nodes
        .iter()
        .map(|n| n.y + n.height / 2.0)
        .fold(0.0f32, f32::max)
        + 50.0;

    for sg in &subgraph_boxes {
        total_width = total_width.max(sg.x + sg.width + 50.0);
        total_height = total_height.max(sg.y + sg.height + 50.0);
    }

    let margin = 20.0;
    for node in &mut layout_nodes {
        if let Some(near) = graph.nodes.get(&node.id).and_then(|n| n.near) {
            let (x, y) = match near {
                NearPosition::TopLeft => (margin + node.width / 2.0, margin + node.height / 2.0),
                NearPosition::TopCenter => (total_width / 2.0, margin + node.height / 2.0),
                NearPosition::TopRight => (total_width - margin - node.width / 2.0, margin + node.height / 2.0),
                NearPosition::CenterLeft => (margin + node.width / 2.0, total_height / 2.0),
                NearPosition::CenterRight => (total_width - margin - node.width / 2.0, total_height / 2.0),
                NearPosition::BottomLeft => (margin + node.width / 2.0, total_height - margin - node.height / 2.0),
                NearPosition::BottomCenter => (total_width / 2.0, total_height - margin - node.height / 2.0),
                NearPosition::BottomRight => (total_width - margin - node.width / 2.0, total_height - margin - node.height / 2.0),
            };
            node.x = x;
            node.y = y;
        }
    }

    Ok(LayoutResult {
        nodes: layout_nodes,
        edges: layout_edges,
        subgraphs: subgraph_boxes,
        total_width,
        total_height,
    })
}

fn node_entity_at_level(
    node_id: &str,
    direct_nodes: &[String],
    children: &[usize],
    subgraphs: &[crate::diagram::SubgraphDef],
) -> Option<String> {
    if direct_nodes.iter().any(|n| n == node_id) {
        return Some(node_id.to_string());
    }
    for &child_idx in children {
        if subgraphs[child_idx].node_ids.iter().any(|n| n == node_id) {
            return Some(format!("__sg_{child_idx}"));
        }
    }
    None
}

fn collect_level_edges(
    all_edges: &[SimpleEdge],
    direct_nodes: &[String],
    children: &[usize],
    subgraphs: &[crate::diagram::SubgraphDef],
) -> Vec<SimpleEdge> {
    let mut seen = std::collections::HashSet::new();
    let mut edges = Vec::new();
    for edge in all_edges {
        let from_ent = node_entity_at_level(&edge.from, direct_nodes, children, subgraphs);
        let to_ent = node_entity_at_level(&edge.to, direct_nodes, children, subgraphs);
        if let (Some(from), Some(to)) = (from_ent, to_ent) {
            if from != to && seen.insert((from.clone(), to.clone())) {
                edges.push(SimpleEdge { from, to });
            }
        }
    }
    edges
}

fn layout_grid(
    ids: &[String],
    sizes: &HashMap<String, (f32, f32)>,
    grid_rows: Option<usize>,
    grid_columns: Option<usize>,
    gap: f32,
) -> sugiyama::SubgraphLayout {
    let n = ids.len();
    if n == 0 {
        return sugiyama::SubgraphLayout {
            positions: HashMap::new(),
            edge_waypoints: HashMap::new(),
        };
    }

    let cols = if let Some(c) = grid_columns {
        c.max(1)
    } else if let Some(r) = grid_rows {
        ((n + r - 1) / r).max(1)
    } else {
        (n as f32).sqrt().ceil() as usize
    };

    let rows = (n + cols - 1) / cols;

    let mut col_widths = vec![0.0f32; cols];
    let mut row_heights = vec![0.0f32; rows];

    for (idx, id) in ids.iter().enumerate() {
        let r = idx / cols;
        let c = idx % cols;
        let (w, h) = sizes.get(id).copied().unwrap_or((DEFAULT_NODE_WIDTH, DEFAULT_NODE_HEIGHT));
        col_widths[c] = col_widths[c].max(w);
        row_heights[r] = row_heights[r].max(h);
    }

    let mut positions = HashMap::new();
    for (idx, id) in ids.iter().enumerate() {
        let r = idx / cols;
        let c = idx % cols;
        let x: f32 = col_widths[..c].iter().sum::<f32>() + c as f32 * gap + col_widths[c] / 2.0;
        let y: f32 = row_heights[..r].iter().sum::<f32>() + r as f32 * gap + row_heights[r] / 2.0;
        positions.insert(id.clone(), (x, y));
    }

    sugiyama::SubgraphLayout { positions, edge_waypoints: HashMap::new() }
}

fn compute_node_sizes(
    graph: &DiagramGraph,
    measured_sizes: Option<&HashMap<String, egui::Vec2>>,
) -> HashMap<String, (f32, f32)> {
    let mut sizes = HashMap::new();
    for (id, node) in &graph.nodes {
        let (w, h) = if let Some(ms) = measured_sizes {
            if let Some(sz) = ms.get(id) {
                (sz.x + NODE_PADDING_H, sz.y + NODE_PADDING_V)
            } else {
                (DEFAULT_NODE_WIDTH, DEFAULT_NODE_HEIGHT)
            }
        } else {
            let lines: Vec<&str> = node.label.split('\n').collect();
            let max_line = lines.iter().map(|l| l.len()).max().unwrap_or(0);
            let estimated_w = (max_line as f32 * 9.0 + NODE_PADDING_H).max(DEFAULT_NODE_WIDTH);
            let estimated_h =
                (lines.len() as f32 * 18.0 + NODE_PADDING_V).max(DEFAULT_NODE_HEIGHT);
            (estimated_w, estimated_h)
        };
        let (w, h) = match node.shape {
            NodeShape::Diamond => (w * 1.4, h * 1.4),
            NodeShape::Circle => {
                let d = w.max(h);
                (d, d)
            }
            NodeShape::Class => {
                let row_h = 18.0;
                let header_h = h.max(30.0);
                let fields_h = node.class_fields.len() as f32 * row_h;
                let methods_h = node.class_methods.len() as f32 * row_h;
                let total_h = header_h + fields_h + methods_h + 8.0;
                let max_field_w = node.class_fields.iter()
                    .map(|f| (f.name.len() + f.type_str.len() + 4) as f32 * 8.0)
                    .fold(0.0f32, f32::max);
                let max_method_w = node.class_methods.iter()
                    .map(|m| (m.name.len() + m.return_type.len() + 4) as f32 * 8.0)
                    .fold(0.0f32, f32::max);
                let total_w = w.max(max_field_w + NODE_PADDING_H).max(max_method_w + NODE_PADDING_H);
                (total_w, total_h)
            }
            NodeShape::SqlTable => {
                let row_h = 18.0;
                let header_h = h.max(30.0);
                let cols_h = node.sql_columns.len() as f32 * row_h;
                let total_h = header_h + cols_h + 8.0;
                let max_col_w = node.sql_columns.iter()
                    .map(|c| (c.name.len() + c.type_str.len() + c.constraint.len() + 6) as f32 * 8.0)
                    .fold(0.0f32, f32::max);
                let total_w = w.max(max_col_w + NODE_PADDING_H);
                (total_w, total_h)
            }
            _ => (w, h),
        };
        sizes.insert(id.clone(), (w, h));
    }
    sizes
}

fn is_against_flow(fx: f32, fy: f32, tx: f32, ty: f32, direction: Direction) -> bool {
    match direction {
        Direction::TD | Direction::TB => fy > ty,
        Direction::BT => fy < ty,
        Direction::LR => fx > tx,
        Direction::RL => fx < tx,
    }
}

fn intersect_rect(cx: f32, cy: f32, w: f32, h: f32, px: f32, py: f32) -> [f32; 2] {
    let dx = px - cx;
    let dy = py - cy;
    if dx.abs() < 1e-6 && dy.abs() < 1e-6 {
        return [cx, cy];
    }
    let hw = w / 2.0;
    let hh = h / 2.0;
    if dy.abs() * hw > dx.abs() * hh {
        let sy = hh * dy.signum();
        let sx = sy * dx / dy;
        [cx + sx, cy + sy]
    } else {
        let sx = hw * dx.signum();
        let sy = sx * dy / dx;
        [cx + sx, cy + sy]
    }
}

fn intersect_diamond(cx: f32, cy: f32, w: f32, h: f32, px: f32, py: f32) -> [f32; 2] {
    let dx = px - cx;
    let dy = py - cy;
    if dx.abs() < 1e-6 && dy.abs() < 1e-6 {
        return [cx, cy];
    }
    let hw = w / 2.0;
    let hh = h / 2.0;
    let t = 1.0 / (dx.abs() / hw + dy.abs() / hh);
    [cx + t * dx, cy + t * dy]
}

fn intersect_circle(cx: f32, cy: f32, w: f32, h: f32, px: f32, py: f32) -> [f32; 2] {
    let dx = px - cx;
    let dy = py - cy;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist < 1e-6 {
        return [cx, cy];
    }
    let radius = w.min(h) / 2.0;
    [cx + radius * dx / dist, cy + radius * dy / dist]
}

fn intersect_node(
    cx: f32, cy: f32, w: f32, h: f32,
    shape: NodeShape,
    px: f32, py: f32,
) -> [f32; 2] {
    match shape {
        NodeShape::Rect | NodeShape::Rounded | NodeShape::Flag
        | NodeShape::Page | NodeShape::Document | NodeShape::Package
        | NodeShape::Parallelogram | NodeShape::Step | NodeShape::Callout
        | NodeShape::StoredData | NodeShape::Queue | NodeShape::Text
        | NodeShape::Class | NodeShape::SqlTable => {
            intersect_rect(cx, cy, w, h, px, py)
        }
        NodeShape::Diamond => intersect_diamond(cx, cy, w, h, px, py),
        NodeShape::Circle | NodeShape::Oval | NodeShape::Cloud
        | NodeShape::Cylinder | NodeShape::Person => {
            intersect_circle(cx, cy, w, h, px, py)
        }
        NodeShape::Hexagon => {
            intersect_rect(cx, cy, w * 0.85, h, px, py)
        }
    }
}

fn build_edges_with_port_spreading(
    edge_defs: &[crate::diagram::EdgeDef],
    positions: &HashMap<String, (f32, f32)>,
    node_sizes: &HashMap<String, (f32, f32)>,
    node_shapes: &HashMap<String, NodeShape>,
    direction: Direction,
    edge_waypoints: &HashMap<(String, String), Vec<(f32, f32)>>,
) -> Vec<LayoutEdge> {
    let is_horizontal = matches!(direction, Direction::LR | Direction::RL);
    let port_margin = 8.0;

    struct EdgeInfo {
        def_idx: usize,
        from_id: String,
        to_id: String,
        reversed: bool,
    }

    let mut infos: Vec<EdgeInfo> = Vec::new();
    for (idx, ed) in edge_defs.iter().enumerate() {
        if let (Some(&(fx, fy)), Some(&(tx, ty))) =
            (positions.get(&ed.from), positions.get(&ed.to))
        {
            let reversed = is_against_flow(fx, fy, tx, ty, direction);
            let (from_id, to_id) = if reversed {
                (ed.to.clone(), ed.from.clone())
            } else {
                (ed.from.clone(), ed.to.clone())
            };
            infos.push(EdgeInfo {
                def_idx: idx,
                from_id,
                to_id,
                reversed,
            });
        }
    }

    let mut outgoing: HashMap<&str, Vec<(usize, f32)>> = HashMap::new();
    let mut incoming: HashMap<&str, Vec<(usize, f32)>> = HashMap::new();

    for (i, info) in infos.iter().enumerate() {
        let (_, _) = positions[&info.from_id];
        let (tx, ty) = positions[&info.to_id];
        let (fx, fy) = positions[&info.from_id];
        let target_cross = if is_horizontal { ty } else { tx };
        let source_cross = if is_horizontal { fy } else { fx };
        outgoing
            .entry(info.from_id.as_str())
            .or_default()
            .push((i, target_cross));
        incoming
            .entry(info.to_id.as_str())
            .or_default()
            .push((i, source_cross));
    }

    for edges in outgoing.values_mut() {
        edges.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    }
    for edges in incoming.values_mut() {
        edges.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    }

    let mut start_ports: Vec<[f32; 2]> = vec![[0.0; 2]; infos.len()];
    let mut end_ports: Vec<[f32; 2]> = vec![[0.0; 2]; infos.len()];

    for (node_id, edges) in &outgoing {
        let (nx, ny) = positions[*node_id];
        let (nw, nh) = node_sizes
            .get(*node_id)
            .copied()
            .unwrap_or((DEFAULT_NODE_WIDTH, DEFAULT_NODE_HEIGHT));
        let n = edges.len();
        let cross_size = if is_horizontal { nh } else { nw };
        let usable = (cross_size - 2.0 * port_margin).max(0.0);
        let spread_start =
            if is_horizontal { ny } else { nx } - usable / 2.0;

        for (rank, &(info_idx, _)) in edges.iter().enumerate() {
            let t = if n == 1 {
                0.5
            } else {
                rank as f32 / (n - 1) as f32
            };
            let cross_pos = spread_start + t * usable;

            start_ports[info_idx] = match direction {
                Direction::TD | Direction::TB => [cross_pos, ny + nh / 2.0],
                Direction::BT => [cross_pos, ny - nh / 2.0],
                Direction::LR => [nx + nw / 2.0, cross_pos],
                Direction::RL => [nx - nw / 2.0, cross_pos],
            };
        }
    }

    for (node_id, edges) in &incoming {
        let (nx, ny) = positions[*node_id];
        let (nw, nh) = node_sizes
            .get(*node_id)
            .copied()
            .unwrap_or((DEFAULT_NODE_WIDTH, DEFAULT_NODE_HEIGHT));
        let n = edges.len();
        let cross_size = if is_horizontal { nh } else { nw };
        let usable = (cross_size - 2.0 * port_margin).max(0.0);
        let spread_start =
            if is_horizontal { ny } else { nx } - usable / 2.0;

        for (rank, &(info_idx, _)) in edges.iter().enumerate() {
            let t = if n == 1 {
                0.5
            } else {
                rank as f32 / (n - 1) as f32
            };
            let cross_pos = spread_start + t * usable;

            end_ports[info_idx] = match direction {
                Direction::TD | Direction::TB => [cross_pos, ny - nh / 2.0],
                Direction::BT => [cross_pos, ny + nh / 2.0],
                Direction::LR => [nx - nw / 2.0, cross_pos],
                Direction::RL => [nx + nw / 2.0, cross_pos],
            };
        }
    }

    let mut layout_edges = Vec::new();
    for (i, info) in infos.iter().enumerate() {
        let start = start_ports[i];
        let end = end_ports[i];
        let ed = &edge_defs[info.def_idx];

        let wps = edge_waypoints
            .get(&(ed.from.clone(), ed.to.clone()))
            .or_else(|| edge_waypoints.get(&(ed.to.clone(), ed.from.clone())));

        if let Some(waypoints) = wps {
            let mut points = Vec::with_capacity(waypoints.len() + 2);
            points.push(start);
            if info.reversed {
                for &(wx, wy) in waypoints.iter().rev() {
                    points.push([wx, wy]);
                }
            } else {
                for &(wx, wy) in waypoints {
                    points.push([wx, wy]);
                }
            }
            points.push(end);

            let mid_idx = points.len() / 2;
            let label_pos = ed.label.as_ref().map(|_| points[mid_idx]);

            layout_edges.push(LayoutEdge {
                points,
                control_points: None,
                edge_type: ed.edge_type,
                label: ed.label.clone(),
                label_pos,
                reversed: info.reversed,
                src_arrowhead: ed.src_arrowhead,
                dst_arrowhead: ed.dst_arrowhead,
                style: ed.style.clone(),
            });
        } else {
            let cp = compute_bezier_control_points(start, end, direction);
            let mid = bezier_midpoint(start, cp, end);
            let label_pos = ed.label.as_ref().map(|_| mid);

            layout_edges.push(LayoutEdge {
                points: vec![start, end],
                control_points: Some(cp),
                edge_type: ed.edge_type,
                label: ed.label.clone(),
                label_pos,
                reversed: info.reversed,
                src_arrowhead: ed.src_arrowhead,
                dst_arrowhead: ed.dst_arrowhead,
                style: ed.style.clone(),
            });
        }
    }

    for (i, info) in infos.iter().enumerate() {
        let edge = &mut layout_edges[i];
        if edge.points.len() < 2 {
            continue;
        }
        let last = edge.points.len() - 1;
        let p1_ref = if edge.points.len() > 2 { edge.points[1] } else { edge.points[last] };
        let p2_ref = if edge.points.len() > 2 { edge.points[last - 1] } else { edge.points[0] };

        if let Some(&(fcx, fcy)) = positions.get(&info.from_id) {
            let (fw, fh) = node_sizes.get(&info.from_id).copied().unwrap_or((DEFAULT_NODE_WIDTH, DEFAULT_NODE_HEIGHT));
            let fshape = node_shapes.get(&info.from_id).copied().unwrap_or(NodeShape::Rect);
            edge.points[0] = intersect_node(fcx, fcy, fw, fh, fshape, p1_ref[0], p1_ref[1]);
        }

        if let Some(&(tcx, tcy)) = positions.get(&info.to_id) {
            let (tw, th) = node_sizes.get(&info.to_id).copied().unwrap_or((DEFAULT_NODE_WIDTH, DEFAULT_NODE_HEIGHT));
            let tshape = node_shapes.get(&info.to_id).copied().unwrap_or(NodeShape::Rect);
            edge.points[last] = intersect_node(tcx, tcy, tw, th, tshape, p2_ref[0], p2_ref[1]);
        }
    }

    layout_edges
}

fn compute_bezier_control_points(
    start: [f32; 2],
    end: [f32; 2],
    direction: Direction,
) -> [[f32; 2]; 2] {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let offset = 0.4;

    match direction {
        Direction::TD | Direction::TB => {
            let oy = dy.abs() * offset;
            [
                [start[0], start[1] + oy],
                [end[0], end[1] - oy],
            ]
        }
        Direction::BT => {
            let oy = dy.abs() * offset;
            [
                [start[0], start[1] - oy],
                [end[0], end[1] + oy],
            ]
        }
        Direction::LR => {
            let ox = dx.abs() * offset;
            [
                [start[0] + ox, start[1]],
                [end[0] - ox, end[1]],
            ]
        }
        Direction::RL => {
            let ox = dx.abs() * offset;
            [
                [start[0] - ox, start[1]],
                [end[0] + ox, end[1]],
            ]
        }
    }
}

fn bezier_midpoint(start: [f32; 2], cp: [[f32; 2]; 2], end: [f32; 2]) -> [f32; 2] {
    let t = 0.5;
    let mt = 1.0 - t;
    let x = mt * mt * mt * start[0]
        + 3.0 * mt * mt * t * cp[0][0]
        + 3.0 * mt * t * t * cp[1][0]
        + t * t * t * end[0];
    let y = mt * mt * mt * start[1]
        + 3.0 * mt * mt * t * cp[0][1]
        + 3.0 * mt * t * t * cp[1][1]
        + t * t * t * end[1];
    [x, y]
}

fn resolve_style(graph: &DiagramGraph, node_id: &str) -> StyleProps {
    let mut style = StyleProps::default();

    if let Some(node) = graph.nodes.get(node_id) {
        for class_name in &node.classes {
            if let Some(class_style) = graph.class_defs.get(class_name) {
                merge_style(&mut style, class_style);
            }
        }
    }

    if let Some(node_style) = graph.styles.get(node_id) {
        merge_style(&mut style, node_style);
    }

    style
}

fn merge_style(target: &mut StyleProps, source: &StyleProps) {
    if source.fill.is_some() {
        target.fill = source.fill;
    }
    if source.stroke.is_some() {
        target.stroke = source.stroke;
    }
    if source.stroke_width.is_some() {
        target.stroke_width = source.stroke_width;
    }
    if source.color.is_some() {
        target.color = source.color;
    }
    if source.border_radius.is_some() {
        target.border_radius = source.border_radius;
    }
    if source.opacity.is_some() {
        target.opacity = source.opacity;
    }
    if source.stroke_dash.is_some() {
        target.stroke_dash = source.stroke_dash;
    }
    if source.shadow.is_some() {
        target.shadow = source.shadow;
    }
    if source.three_d.is_some() {
        target.three_d = source.three_d;
    }
    if source.multiple.is_some() {
        target.multiple = source.multiple;
    }
    if source.double_border.is_some() {
        target.double_border = source.double_border;
    }
    if source.font_size.is_some() {
        target.font_size = source.font_size;
    }
    if source.bold.is_some() {
        target.bold = source.bold;
    }
    if source.italic.is_some() {
        target.italic = source.italic;
    }
    if source.fill_pattern.is_some() {
        target.fill_pattern = source.fill_pattern;
    }
    if source.animated.is_some() {
        target.animated = source.animated;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: [f32; 2], b: [f32; 2]) -> bool {
        (a[0] - b[0]).abs() < 0.01 && (a[1] - b[1]).abs() < 0.01
    }

    #[test]
    fn test_intersect_rect_top() {
        let r = intersect_rect(100.0, 100.0, 60.0, 40.0, 100.0, 0.0);
        assert!(approx_eq(r, [100.0, 80.0]));
    }

    #[test]
    fn test_intersect_rect_bottom() {
        let r = intersect_rect(100.0, 100.0, 60.0, 40.0, 100.0, 200.0);
        assert!(approx_eq(r, [100.0, 120.0]));
    }

    #[test]
    fn test_intersect_rect_right() {
        let r = intersect_rect(100.0, 100.0, 60.0, 40.0, 200.0, 100.0);
        assert!(approx_eq(r, [130.0, 100.0]));
    }

    #[test]
    fn test_intersect_rect_diagonal() {
        let r = intersect_rect(100.0, 100.0, 100.0, 50.0, 200.0, 200.0);
        assert!((r[0] - 100.0).abs() <= 50.0);
        assert!((r[1] - 100.0).abs() <= 25.0);
    }

    #[test]
    fn test_intersect_rect_degenerate() {
        let r = intersect_rect(100.0, 100.0, 60.0, 40.0, 100.0, 100.0);
        assert!(approx_eq(r, [100.0, 100.0]));
    }

    #[test]
    fn test_intersect_diamond_top() {
        let r = intersect_diamond(100.0, 100.0, 60.0, 40.0, 100.0, 0.0);
        assert!(approx_eq(r, [100.0, 80.0]));
    }

    #[test]
    fn test_intersect_diamond_right() {
        let r = intersect_diamond(100.0, 100.0, 60.0, 40.0, 200.0, 100.0);
        assert!(approx_eq(r, [130.0, 100.0]));
    }

    #[test]
    fn test_intersect_diamond_diagonal() {
        let r = intersect_diamond(100.0, 100.0, 60.0, 40.0, 200.0, 200.0);
        let dist_x = (r[0] - 100.0).abs() / 30.0;
        let dist_y = (r[1] - 100.0).abs() / 20.0;
        assert!((dist_x + dist_y - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_intersect_circle_right() {
        let r = intersect_circle(100.0, 100.0, 60.0, 60.0, 200.0, 100.0);
        assert!(approx_eq(r, [130.0, 100.0]));
    }

    #[test]
    fn test_intersect_circle_diagonal() {
        let r = intersect_circle(100.0, 100.0, 60.0, 60.0, 200.0, 200.0);
        let dx = r[0] - 100.0;
        let dy = r[1] - 100.0;
        let dist = (dx * dx + dy * dy).sqrt();
        assert!((dist - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_intersect_node_dispatches() {
        let rect = intersect_node(0.0, 0.0, 100.0, 50.0, NodeShape::Rect, 0.0, -100.0);
        assert!(approx_eq(rect, [0.0, -25.0]));
        let diamond = intersect_node(0.0, 0.0, 100.0, 50.0, NodeShape::Diamond, 0.0, -100.0);
        assert!(approx_eq(diamond, [0.0, -25.0]));
        let circle = intersect_node(0.0, 0.0, 50.0, 50.0, NodeShape::Circle, 50.0, 0.0);
        assert!(approx_eq(circle, [25.0, 0.0]));
    }
}
