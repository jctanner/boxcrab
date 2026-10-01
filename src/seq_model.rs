//! Editing operations on sequence-diagram graphs.
//!
//! A sequence diagram is stored in a `DiagramGraph` as follows:
//! - participants are nodes keyed `seq_NNN_name`; the `NNN` prefix is their
//!   left-to-right order and `name` is the identifier used in Mermaid source;
//! - `graph.edges` is the ordered timeline. Edge index == "slot" index, and a
//!   note occupies a slot too (an edge from a participant to a `__note_N` node);
//! - activations and groups (`alt`/`loop`/...) refer to slot indices.
//!
//! The helpers here keep all of those slot references consistent when the
//! timeline changes.

use crate::diagram::*;
use std::collections::HashMap;

pub const NOTE_PREFIX: &str = "__note_";

/// (menu name, edge type, destination arrowhead) for each Mermaid arrow style.
pub const ARROW_STYLES: &[(&str, EdgeType, Option<ArrowheadType>)] = &[
    ("Solid arrow ->>", EdgeType::Arrow, None),
    ("Dashed arrow -->>", EdgeType::DottedArrow, None),
    ("Solid line ->", EdgeType::Line, None),
    ("Dashed line -->", EdgeType::DottedLine, None),
    ("Solid cross -x", EdgeType::Arrow, Some(ArrowheadType::Cross)),
    ("Dashed cross --x", EdgeType::DottedArrow, Some(ArrowheadType::Cross)),
    ("Solid async -)", EdgeType::Arrow, Some(ArrowheadType::Arrow)),
    ("Dashed async --)", EdgeType::DottedArrow, Some(ArrowheadType::Arrow)),
];

/// Replace Mermaid line-break tags (`<br/>`, `<br>`, `<br />`, any case)
/// with newlines.
pub fn normalize_br(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if lower[i..].starts_with("<br") {
            // Accept "<br>", "<br/>", "<br />".
            let rest = &lower[i + 3..];
            let trimmed = rest.trim_start_matches(' ');
            let skipped = rest.len() - trimmed.len();
            let tag_len = if trimmed.starts_with("/>") {
                Some(skipped + 2)
            } else if trimmed.starts_with('>') {
                Some(skipped + 1)
            } else {
                None
            };
            if let Some(len) = tag_len {
                out.push('\n');
                i += 3 + len;
                continue;
            }
        }
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Inverse of `normalize_br` for writing Mermaid source.
pub fn to_br(s: &str) -> String {
    s.replace('\n', "<br/>")
}

pub fn is_note(id: &str) -> bool {
    id.starts_with(NOTE_PREFIX)
}

/// Is this edge a note attachment rather than a message?
pub fn is_note_edge(edge: &EdgeDef) -> bool {
    is_note(&edge.to)
}

/// Participant ids in left-to-right order.
pub fn participant_ids(g: &DiagramGraph) -> Vec<String> {
    let mut v: Vec<String> = g.nodes.keys().filter(|id| !is_note(id)).cloned().collect();
    v.sort();
    v
}

/// The Mermaid identifier of a participant id (`seq_001_Bob` -> `Bob`).
pub fn name_of(id: &str) -> &str {
    id.splitn(3, '_').nth(2).unwrap_or(id)
}

fn make_id(index: usize, name: &str) -> String {
    format!("seq_{index:03}_{name}")
}

/// Arrow style index into `ARROW_STYLES` for an edge, if it matches one.
pub fn arrow_style_index(edge: &EdgeDef) -> Option<usize> {
    ARROW_STYLES
        .iter()
        .position(|(_, t, h)| *t == edge.edge_type && *h == edge.dst_arrowhead)
}

/// Mermaid arrow token for an edge.
pub fn arrow_token(edge: &EdgeDef) -> &'static str {
    match (edge.edge_type, edge.dst_arrowhead) {
        (EdgeType::Arrow, None) => "->>",
        (EdgeType::DottedArrow, None) => "-->>",
        (EdgeType::Line, None) => "->",
        (EdgeType::DottedLine, None) => "-->",
        (EdgeType::Arrow, Some(ArrowheadType::Cross)) => "-x",
        (EdgeType::DottedArrow, Some(ArrowheadType::Cross)) => "--x",
        (EdgeType::Arrow, Some(ArrowheadType::Arrow)) => "-)",
        (EdgeType::DottedArrow, Some(ArrowheadType::Arrow)) => "--)",
        (EdgeType::Line, _) => "->",
        (EdgeType::DottedLine, _) => "-->",
        (EdgeType::DottedArrow | EdgeType::BidiDottedArrow, _) => "-->>",
        _ => "->>",
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NoteInfo {
    /// "left", "right" or "over"
    pub position: String,
    /// Mermaid participant names the note refers to.
    pub names: Vec<String>,
}

pub fn note_info(def: &NodeDef) -> NoteInfo {
    let tip = def.tooltip.as_deref().unwrap_or("");
    let mut parts = tip.strip_prefix("note:").unwrap_or("").splitn(2, ':');
    let position = parts.next().filter(|p| !p.is_empty()).unwrap_or("right").to_string();
    let names = parts
        .next()
        .unwrap_or("")
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    NoteInfo { position, names }
}

pub fn set_note_info(def: &mut NodeDef, info: &NoteInfo) {
    def.tooltip = Some(format!("note:{}:{}", info.position, info.names.join(",")));
}

fn new_node(label: &str, shape: NodeShape, tooltip: Option<String>) -> NodeDef {
    NodeDef {
        label: label.to_string(),
        shape,
        classes: Vec::new(),
        class_fields: Vec::new(),
        class_methods: Vec::new(),
        sql_columns: Vec::new(),
        near: None,
        tooltip,
        link: None,
    }
}

pub fn new_sequence_graph() -> DiagramGraph {
    DiagramGraph {
        diagram_type: DiagramType::Sequence,
        direction: Direction::TD,
        nodes: HashMap::new(),
        edges: Vec::new(),
        subgraphs: Vec::new(),
        styles: HashMap::new(),
        class_defs: HashMap::new(),
        layer_spacing: None,
        node_spacing: None,
        seq_activations: Vec::new(),
    }
}

/// Rewrite participant ids so that `order` becomes the left-to-right order,
/// updating every reference (edges, activations, styles).
fn renumber(g: &mut DiagramGraph, order: &[String]) {
    let map: HashMap<String, String> = order
        .iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), make_id(i, name_of(id))))
        .collect();
    let remap = |id: &mut String| {
        if let Some(n) = map.get(id.as_str()) {
            *id = n.clone();
        }
    };

    let old_nodes = std::mem::take(&mut g.nodes);
    for (mut id, def) in old_nodes {
        remap(&mut id);
        g.nodes.insert(id, def);
    }
    for e in &mut g.edges {
        remap(&mut e.from);
        remap(&mut e.to);
    }
    for a in &mut g.seq_activations {
        remap(&mut a.0);
    }
    let old_styles = std::mem::take(&mut g.styles);
    for (mut id, s) in old_styles {
        remap(&mut id);
        g.styles.insert(id, s);
    }
}

fn unique_name(g: &DiagramGraph) -> (String, usize) {
    let names: Vec<String> = participant_ids(g).iter().map(|id| name_of(id).to_string()).collect();
    let mut n = 1;
    loop {
        let candidate = format!("P{n}");
        if !names.contains(&candidate) {
            return (candidate, n);
        }
        n += 1;
    }
}

/// Insert a new participant at left-to-right position `index`.
pub fn add_participant(g: &mut DiagramGraph, index: usize, shape: NodeShape) -> String {
    let (name, n) = unique_name(g);
    let label = if shape == NodeShape::Person {
        format!("Actor {n}")
    } else {
        format!("Participant {n}")
    };
    let mut order = participant_ids(g);
    let index = index.min(order.len());
    let temp_id = make_id(index, &name);
    g.nodes.insert(temp_id.clone(), new_node(&label, shape, None));
    order.insert(index, temp_id);
    renumber(g, &order);
    make_id(index, &name)
}

/// Move a participant to a new left-to-right position; returns its new id.
pub fn move_participant(g: &mut DiagramGraph, id: &str, new_index: usize) -> String {
    let mut order = participant_ids(g);
    let Some(pos) = order.iter().position(|p| p == id) else {
        return id.to_string();
    };
    let item = order.remove(pos);
    let new_index = new_index.min(order.len());
    order.insert(new_index, item);
    renumber(g, &order);
    make_id(new_index, name_of(id))
}

/// Remove a participant together with its messages and notes.
pub fn remove_participant(g: &mut DiagramGraph, id: &str) {
    let name = name_of(id).to_string();
    let slots: Vec<usize> = g
        .edges
        .iter()
        .enumerate()
        .filter(|(_, e)| e.from == id || e.to == id)
        .map(|(i, _)| i)
        .collect();
    for s in slots.into_iter().rev() {
        remove_slot(g, s);
    }
    g.nodes.remove(id);
    g.styles.remove(id);
    g.seq_activations.retain(|a| a.0 != id);

    // Drop references from multi-participant notes ("over A,B").
    for (nid, def) in g.nodes.iter_mut() {
        if is_note(nid) {
            let mut info = note_info(def);
            if info.names.contains(&name) {
                info.names.retain(|n| *n != name);
                set_note_info(def, &info);
            }
        }
    }
    let order = participant_ids(g);
    renumber(g, &order);
}

/// Insert an edge (message or note attachment) at timeline slot `slot`.
///
/// Slot references are shifted so existing activations/groups keep covering
/// the same messages. Inserting exactly on a boundary lands outside the group
/// or activation; use `swap_slots` to move it across.
pub fn insert_slot(g: &mut DiagramGraph, slot: usize, edge: EdgeDef) {
    let slot = slot.min(g.edges.len());
    g.edges.insert(slot, edge);
    for a in &mut g.seq_activations {
        if a.1 >= slot {
            a.1 += 1;
        }
        if a.2 > slot {
            a.2 += 1;
        }
        a.2 = a.2.max(a.1);
    }
    for sg in &mut g.subgraphs {
        if let Some(s) = sg.grid_rows.as_mut() {
            if *s >= slot {
                *s += 1;
            }
        }
        if let Some(e) = sg.grid_columns.as_mut() {
            if *e > slot {
                *e += 1;
            }
        }
        for b in &mut sg.branches {
            if b.0 >= slot {
                b.0 += 1;
            }
        }
    }
}

/// Remove the edge at `slot` (and its note node, if it is a note).
pub fn remove_slot(g: &mut DiagramGraph, slot: usize) {
    if slot >= g.edges.len() {
        return;
    }
    let e = g.edges.remove(slot);
    if is_note(&e.to) {
        g.nodes.remove(&e.to);
    }
    let dec = |v: &mut usize| {
        if *v > slot {
            *v -= 1;
        }
    };
    g.seq_activations.retain_mut(|a| {
        let had_span = a.2 > a.1;
        dec(&mut a.1);
        dec(&mut a.2);
        !(had_span && a.2 <= a.1)
    });
    for sg in &mut g.subgraphs {
        if let Some(s) = sg.grid_rows.as_mut() {
            dec(s);
        }
        if let Some(e) = sg.grid_columns.as_mut() {
            dec(e);
        }
        for b in &mut sg.branches {
            dec(&mut b.0);
        }
    }
}

/// Swap two adjacent timeline entries without touching activation or group
/// references (so the entries effectively move across any boundary).
pub fn swap_slots(g: &mut DiagramGraph, a: usize, b: usize) {
    if a < g.edges.len() && b < g.edges.len() {
        g.edges.swap(a, b);
    }
}

pub fn add_message(
    g: &mut DiagramGraph,
    slot: usize,
    from: &str,
    to: &str,
    style: usize,
) {
    let (_, edge_type, head) = ARROW_STYLES[style.min(ARROW_STYLES.len() - 1)];
    let label = if from == to { "Self" } else { "Message" };
    insert_slot(
        g,
        slot,
        EdgeDef {
            from: from.to_string(),
            to: to.to_string(),
            edge_type,
            label: Some(label.to_string()),
            src_arrowhead: None,
            dst_arrowhead: head,
            style: StyleProps::default(),
        },
    );
}

/// Insert a note attached to `participant` at `slot`; returns the note id.
pub fn add_note(g: &mut DiagramGraph, slot: usize, participant: &str, position: &str) -> String {
    let next = g
        .nodes
        .keys()
        .filter_map(|k| k.strip_prefix(NOTE_PREFIX)?.parse::<usize>().ok())
        .max()
        .map_or(0, |m| m + 1);
    let note_id = format!("{NOTE_PREFIX}{next}");
    let info = NoteInfo {
        position: position.to_string(),
        names: vec![name_of(participant).to_string()],
    };
    let mut def = new_node("Note", NodeShape::Text, None);
    set_note_info(&mut def, &info);
    g.nodes.insert(note_id.clone(), def);
    insert_slot(
        g,
        slot,
        EdgeDef {
            from: participant.to_string(),
            to: note_id.clone(),
            edge_type: EdgeType::DottedLine,
            label: None,
            src_arrowhead: None,
            dst_arrowhead: None,
            style: StyleProps::default(),
        },
    );
    note_id
}

/// Slot (edge index) of the note node `note_id`.
pub fn slot_of_note(g: &DiagramGraph, note_id: &str) -> Option<usize> {
    g.edges.iter().position(|e| e.to == note_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn br_tags_round_trip() {
        assert_eq!(normalize_br("a<br/>b<BR>c<br />d<br  />e"), "a\nb\nc\nd\ne");
        assert_eq!(normalize_br("x <bridge> y"), "x <bridge> y");
        assert_eq!(normalize_br("héllo<br/>wörld"), "héllo\nwörld");
        assert_eq!(to_br("a\nb"), "a<br/>b");
    }

    fn two_participants() -> (DiagramGraph, String, String) {
        let mut g = new_sequence_graph();
        let a = add_participant(&mut g, 0, NodeShape::Rect);
        let b = add_participant(&mut g, 1, NodeShape::Rect);
        (g, a, b)
    }

    #[test]
    fn add_and_order_participants() {
        let (mut g, a, b) = two_participants();
        assert_eq!(participant_ids(&g), vec![a.clone(), b.clone()]);
        // Insert in the middle: ids are renumbered, existing ones keep names.
        let mid = add_participant(&mut g, 1, NodeShape::Person);
        let order: Vec<_> = participant_ids(&g).iter().map(|i| name_of(i).to_string()).collect();
        assert_eq!(order, vec!["P1", "P3", "P2"]);
        assert_eq!(g.nodes[&mid].shape, NodeShape::Person);
    }

    #[test]
    fn renumber_updates_edges() {
        let (mut g, a, b) = two_participants();
        add_message(&mut g, 0, &a, &b, 0);
        let _ = add_participant(&mut g, 0, NodeShape::Rect);
        let e = &g.edges[0];
        assert!(g.nodes.contains_key(&e.from) && g.nodes.contains_key(&e.to));
        assert_eq!(name_of(&e.from), "P1");
        assert_eq!(name_of(&e.to), "P2");
    }

    #[test]
    fn move_participant_reorders() {
        let (mut g, a, _b) = two_participants();
        let new = move_participant(&mut g, &a, 1);
        let order = participant_ids(&g);
        assert_eq!(order[1], new);
        assert_eq!(name_of(&order[0]), "P2");
    }

    #[test]
    fn remove_participant_drops_messages_and_notes() {
        let (mut g, a, b) = two_participants();
        add_message(&mut g, 0, &a, &b, 0);
        add_note(&mut g, 1, &b, "right");
        add_message(&mut g, 2, &a, &a, 0);
        remove_participant(&mut g, &b);
        assert_eq!(g.edges.len(), 1);
        assert_eq!(g.edges[0].from, g.edges[0].to);
        assert!(!g.nodes.keys().any(|k| is_note(k)));
        assert_eq!(participant_ids(&g).len(), 1);
    }

    #[test]
    fn insert_and_remove_shift_references() {
        let (mut g, a, b) = two_participants();
        for _ in 0..3 {
            let end = g.edges.len();
            add_message(&mut g, end, &a, &b, 0);
        }
        g.seq_activations.push((b.clone(), 1, 2));
        g.subgraphs.push(SubgraphDef {
            title: "loop x".into(),
            node_ids: Vec::new(),
            grid_rows: Some(1),
            grid_columns: Some(3),
            grid_gap: None,
            branches: vec![(2, "else".into())],
        });

        // Inserting before everything shifts all references.
        add_message(&mut g, 0, &a, &b, 0);
        assert_eq!(g.seq_activations[0].1, 2);
        assert_eq!(g.seq_activations[0].2, 3);
        assert_eq!(g.subgraphs[0].grid_rows, Some(2));
        assert_eq!(g.subgraphs[0].grid_columns, Some(4));
        assert_eq!(g.subgraphs[0].branches[0].0, 3);

        // Appending on the group's end boundary stays outside it.
        add_message(&mut g, 4, &a, &b, 0);
        assert_eq!(g.subgraphs[0].grid_columns, Some(4));

        remove_slot(&mut g, 4);
        remove_slot(&mut g, 0);
        assert_eq!(g.seq_activations[0].1, 1);
        assert_eq!(g.subgraphs[0].grid_rows, Some(1));
        assert_eq!(g.subgraphs[0].grid_columns, Some(3));
        assert_eq!(g.subgraphs[0].branches[0].0, 2);
    }

    #[test]
    fn removing_activation_message_drops_empty_activation() {
        let (mut g, a, b) = two_participants();
        add_message(&mut g, 0, &a, &b, 0);
        add_message(&mut g, 1, &b, &a, 1);
        g.seq_activations.push((b.clone(), 0, 1));
        remove_slot(&mut g, 1);
        remove_slot(&mut g, 0);
        assert!(g.seq_activations.is_empty());
    }

    #[test]
    fn arrow_tokens_match_styles() {
        let (mut g, a, b) = two_participants();
        for i in 0..ARROW_STYLES.len() {
            add_message(&mut g, i, &a, &b, i);
            assert_eq!(arrow_style_index(&g.edges[i]), Some(i));
        }
        let tokens: Vec<_> = g.edges.iter().map(arrow_token).collect();
        assert_eq!(tokens, vec!["->>", "-->>", "->", "-->", "-x", "--x", "-)", "--)"]);
    }

    #[test]
    fn note_info_round_trip() {
        let (mut g, a, _b) = two_participants();
        let id = add_note(&mut g, 0, &a, "over");
        let mut info = note_info(&g.nodes[&id]);
        assert_eq!(info.position, "over");
        assert_eq!(info.names, vec!["P1".to_string()]);
        info.names.push("P2".into());
        set_note_info(g.nodes.get_mut(&id).unwrap(), &info);
        assert_eq!(note_info(&g.nodes[&id]).names.len(), 2);
        assert_eq!(slot_of_note(&g, &id), Some(0));
    }
}
