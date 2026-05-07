use std::collections::HashMap;
use std::path::Path;

use crate::diagram::{
    ArrowheadType, ClassField, ClassMethod, DiagramGraph, DiagramType, Direction, EdgeDef,
    EdgeType, FillPattern, NearPosition, NodeDef, NodeShape, SqlColumn, StyleProps, SubgraphDef,
};
use crate::theme;

pub fn parse(source: &str, base_dir: Option<&Path>) -> Result<DiagramGraph, Box<dyn std::error::Error>> {
    let mut stmts = parse_map(source)?;

    resolve_imports(&mut stmts, base_dir, 0)?;

    let mut vars = HashMap::new();
    collect_vars(&stmts, &mut vars);
    if !vars.is_empty() {
        substitute_vars(&mut stmts, &vars);
    }

    compile(&stmts)
}

// ---------------------------------------------------------------------------
// AST
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Stmt {
    Shape {
        path: Vec<String>,
        label: Option<String>,
        shape_type: Option<String>,
        style: StyleProps,
        body: Vec<Stmt>,
        classes: Vec<String>,
    },
    Edge {
        points: Vec<Vec<String>>,
        arrow_types: Vec<ArrowType>,
        label: Option<String>,
        style: StyleProps,
        #[allow(dead_code)]
        body: Vec<Stmt>,
        src_arrowhead: Option<ArrowheadType>,
        dst_arrowhead: Option<ArrowheadType>,
    },
    Direction(Direction),
    ClassesDef {
        classes: HashMap<String, ClassDef>,
    },
    Import {
        path: String,
        spread: bool,
        target_path: Vec<String>,
    },
}

#[derive(Debug, Clone)]
struct ClassDef {
    style: StyleProps,
    shape_type: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArrowType {
    Forward,  // ->
    Backward, // <-
    Both,     // <->
    Undirected, // --
}

// ---------------------------------------------------------------------------
// Tokenizer helpers
// ---------------------------------------------------------------------------

fn skip_whitespace(chars: &[char], pos: usize) -> usize {
    let mut i = pos;
    while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
        i += 1;
    }
    i
}

fn is_special(c: char) -> bool {
    matches!(c, '{' | '}' | ':' | ';' | '#' | '\n' | '\r')
}

fn read_double_quoted(chars: &[char], start: usize) -> Result<(String, usize), String> {
    let mut i = start + 1; // skip opening "
    let mut s = String::new();
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            match chars[i + 1] {
                'n' => s.push('\n'),
                't' => s.push('\t'),
                'r' => s.push('\r'),
                '\\' => s.push('\\'),
                '"' => s.push('"'),
                other => {
                    s.push('\\');
                    s.push(other);
                }
            }
            i += 2;
        } else if chars[i] == '"' {
            return Ok((s, i + 1));
        } else {
            s.push(chars[i]);
            i += 1;
        }
    }
    Err("Unterminated double-quoted string".into())
}

fn read_single_quoted(chars: &[char], start: usize) -> Result<(String, usize), String> {
    let mut i = start + 1;
    let mut s = String::new();
    while i < chars.len() {
        if chars[i] == '\'' {
            return Ok((s, i + 1));
        }
        s.push(chars[i]);
        i += 1;
    }
    Err("Unterminated single-quoted string".into())
}

fn read_unquoted(chars: &[char], start: usize) -> (String, usize) {
    let mut i = start;
    let mut s = String::new();
    while i < chars.len() {
        let c = chars[i];
        if is_special(c) || c == '.' {
            break;
        }
        // Stop at arrow-like sequences
        if c == '-' || c == '<' {
            if i + 1 < chars.len() && (chars[i + 1] == '-' || chars[i + 1] == '>') {
                break;
            }
        }
        s.push(c);
        i += 1;
    }
    let trimmed = s.trim_end().to_string();
    let trim_diff = s.len() - trimmed.len();
    (trimmed, i - trim_diff)
}

fn read_identifier(chars: &[char], pos: usize) -> Result<(String, usize), String> {
    let i = skip_whitespace(chars, pos);
    if i >= chars.len() {
        return Err("Expected identifier, got end of input".into());
    }
    match chars[i] {
        '"' => read_double_quoted(chars, i),
        '\'' => read_single_quoted(chars, i),
        _ => {
            let (s, end) = read_unquoted(chars, i);
            if s.is_empty() {
                Err(format!("Expected identifier at position {}", i))
            } else {
                Ok((s, end))
            }
        }
    }
}

fn read_key_path(chars: &[char], pos: usize) -> Result<(Vec<String>, usize), String> {
    let mut path = Vec::new();
    let (first, mut i) = read_identifier(chars, pos)?;
    path.push(first);
    while i < chars.len() && chars[i] == '.' {
        i += 1; // skip dot
        let (seg, end) = read_identifier(chars, i)?;
        path.push(seg);
        i = end;
    }
    Ok((path, i))
}

fn peek_arrow(chars: &[char], pos: usize) -> Option<(ArrowType, usize)> {
    let i = skip_whitespace(chars, pos);
    if i >= chars.len() {
        return None;
    }
    // <->
    if i + 2 < chars.len() && chars[i] == '<' && chars[i + 1] == '-' && chars[i + 2] == '>' {
        return Some((ArrowType::Both, i + 3));
    }
    // ->
    if i + 1 < chars.len() && chars[i] == '-' && chars[i + 1] == '>' {
        return Some((ArrowType::Forward, i + 2));
    }
    // <-
    if i + 1 < chars.len() && chars[i] == '<' && chars[i + 1] == '-' {
        return Some((ArrowType::Backward, i + 2));
    }
    // --
    if i + 1 < chars.len() && chars[i] == '-' && chars[i + 1] == '-' {
        return Some((ArrowType::Undirected, i + 2));
    }
    None
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

fn parse_map(source: &str) -> Result<Vec<Stmt>, Box<dyn std::error::Error>> {
    let chars: Vec<char> = source.chars().collect();
    let (stmts, _) = parse_block(&chars, 0, false)?;
    Ok(stmts)
}

fn parse_block(
    chars: &[char],
    start: usize,
    in_braces: bool,
) -> Result<(Vec<Stmt>, usize), Box<dyn std::error::Error>> {
    let mut stmts = Vec::new();
    let mut i = start;

    loop {
        i = skip_ws_and_newlines(chars, i);
        if i >= chars.len() {
            if in_braces {
                return Err("Unterminated block (missing closing brace)".into());
            }
            break;
        }
        if chars[i] == '}' {
            if in_braces {
                i += 1;
                break;
            } else {
                return Err("Unexpected closing brace".into());
            }
        }

        // Skip comments
        if chars[i] == '#' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        // Block comment """
        if i + 2 < chars.len() && chars[i] == '"' && chars[i + 1] == '"' && chars[i + 2] == '"' {
            i += 3;
            loop {
                if i + 2 < chars.len()
                    && chars[i] == '"'
                    && chars[i + 1] == '"'
                    && chars[i + 2] == '"'
                {
                    i += 3;
                    break;
                }
                if i >= chars.len() {
                    return Err("Unterminated block comment".into());
                }
                i += 1;
            }
            continue;
        }

        // Try to parse a statement
        let (stmt, end) = parse_statement(chars, i)?;
        if let Some(s) = stmt {
            stmts.push(s);
        }
        i = end;
    }

    Ok((stmts, i))
}

fn skip_ws_and_newlines(chars: &[char], pos: usize) -> usize {
    let mut i = pos;
    while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t' || chars[i] == '\n' || chars[i] == '\r' || chars[i] == ';') {
        i += 1;
    }
    i
}

fn parse_statement(
    chars: &[char],
    start: usize,
) -> Result<(Option<Stmt>, usize), Box<dyn std::error::Error>> {
    let i = skip_whitespace(chars, start);
    if i >= chars.len() || chars[i] == '}' || chars[i] == '\n' || chars[i] == '\r' || chars[i] == ';' {
        return Ok((None, if i < chars.len() { i + 1 } else { i }));
    }

    // Check for spread import: ...@./path/to/file.d2
    if i + 3 < chars.len() && chars[i] == '.' && chars[i + 1] == '.' && chars[i + 2] == '.' && chars[i + 3] == '@' {
        let path_start = i + 4;
        let mut end = path_start;
        while end < chars.len() && chars[end] != '\n' && chars[end] != '\r' && chars[end] != ';' && chars[end] != '#' {
            end += 1;
        }
        let import_path: String = chars[path_start..end].iter().collect::<String>().trim().to_string();
        return Ok((Some(Stmt::Import { path: import_path, spread: true, target_path: vec![] }), end));
    }

    // Read the first key path
    let (path, after_path) = match read_key_path(chars, i) {
        Ok(v) => v,
        Err(_) => return Ok((None, skip_to_eol(chars, i))),
    };

    // Check for "direction" keyword
    if path.len() == 1 && path[0] == "direction" {
        let j = skip_whitespace(chars, after_path);
        if j < chars.len() && chars[j] == ':' {
            let j2 = skip_whitespace(chars, j + 1);
            let (val, end) = read_identifier(chars, j2)?;
            let dir = match val.as_str() {
                "right" => Direction::LR,
                "left" => Direction::RL,
                "down" => Direction::TD,
                "up" => Direction::BT,
                _ => Direction::TD,
            };
            return Ok((Some(Stmt::Direction(dir)), skip_to_eol(chars, end)));
        }
    }

    // Check for "classes" block
    if path.len() == 1 && path[0] == "classes" {
        let j = skip_whitespace(chars, after_path);
        if j < chars.len() && chars[j] == ':' {
            let j2 = skip_whitespace(chars, j + 1);
            if j2 < chars.len() && chars[j2] == '{' {
                let (class_defs, end) = parse_classes_block(chars, j2 + 1)?;
                return Ok((Some(Stmt::ClassesDef { classes: class_defs }), end));
            }
        }
        if j < chars.len() && chars[j] == '{' {
            let (class_defs, end) = parse_classes_block(chars, j + 1)?;
            return Ok((Some(Stmt::ClassesDef { classes: class_defs }), end));
        }
    }

    let j = skip_whitespace(chars, after_path);

    // Check if this is an edge (connection)
    if let Some((arrow_type, after_arrow)) = peek_arrow(chars, j) {
        return parse_edge_chain(chars, path, arrow_type, after_arrow);
    }

    // Otherwise it's a shape/node
    parse_shape(chars, path, j)
}

fn parse_edge_chain(
    chars: &[char],
    first_path: Vec<String>,
    first_arrow: ArrowType,
    after_first_arrow: usize,
) -> Result<(Option<Stmt>, usize), Box<dyn std::error::Error>> {
    let mut points: Vec<Vec<String>> = vec![first_path];
    let mut arrows: Vec<ArrowType> = vec![first_arrow];
    let mut i = after_first_arrow;

    loop {
        let i2 = skip_whitespace(chars, i);
        let (next_path, after_path) = read_key_path(chars, i2)?;
        points.push(next_path);
        i = after_path;

        // Check for another arrow (chained connections)
        let j = skip_whitespace(chars, i);
        if let Some((arrow, after)) = peek_arrow(chars, j) {
            arrows.push(arrow);
            i = after;
        } else {
            break;
        }
    }

    // Check for label or body
    let j = skip_whitespace(chars, i);
    let mut label = None;
    let mut style = StyleProps::default();
    let mut body = Vec::new();
    let mut src_arrowhead = None;
    let mut dst_arrowhead = None;

    if j < chars.len() && chars[j] == ':' {
        let k = skip_whitespace(chars, j + 1);
        if k < chars.len() && chars[k] == '{' {
            let (stmts, end) = parse_block(chars, k + 1, true)?;
            for s in &stmts {
                if let Stmt::Shape { path, label: l, body: child_body, .. } = s {
                    if path.len() == 1 {
                        match path[0].as_str() {
                            "label" => label = l.clone(),
                            "source-arrowhead" | "src-arrowhead" => {
                                src_arrowhead = extract_arrowhead_type(l, child_body);
                            }
                            "target-arrowhead" | "dst-arrowhead" => {
                                dst_arrowhead = extract_arrowhead_type(l, child_body);
                            }
                            _ => {}
                        }
                    }
                }
            }
            extract_style_from_stmts(&stmts, &mut style);
            body = stmts;
            i = end;
        } else {
            let (lbl, end) = read_value_to_eol(chars, k);
            if !lbl.is_empty() {
                label = Some(lbl);
            }
            i = end;
        }
    }

    Ok((
        Some(Stmt::Edge {
            points,
            arrow_types: arrows,
            label,
            style,
            body,
            src_arrowhead,
            dst_arrowhead,
        }),
        skip_to_eol(chars, i),
    ))
}

fn parse_shape(
    chars: &[char],
    path: Vec<String>,
    after_path: usize,
) -> Result<(Option<Stmt>, usize), Box<dyn std::error::Error>> {
    let mut label = None;
    let mut shape_type = None;
    let mut style = StyleProps::default();
    let mut body = Vec::new();
    let mut classes = Vec::new();
    let mut i = after_path;

    // Check for colon (label or value)
    if i < chars.len() && chars[i] == ':' {
        let k = skip_whitespace(chars, i + 1);

        // Element import: key: @./path/to/file.d2
        if k < chars.len() && chars[k] == '@' {
            let path_start = k + 1;
            let mut end = path_start;
            while end < chars.len() && chars[end] != '\n' && chars[end] != '\r' && chars[end] != ';' && chars[end] != '#' {
                end += 1;
            }
            let import_path: String = chars[path_start..end].iter().collect::<String>().trim().to_string();
            return Ok((Some(Stmt::Import { path: import_path, spread: false, target_path: path }), end));
        }

        if k < chars.len() && chars[k] == '{' {
            // Block body
            let (stmts, end) = parse_block(chars, k + 1, true)?;
            extract_shape_props(&stmts, &mut label, &mut shape_type, &mut style, &mut classes);
            body = stmts;
            i = end;
        } else if k < chars.len() && chars[k] != '\n' && chars[k] != '\r' && chars[k] != ';' {
            // Check if there's a label followed by a block
            let (val, val_end) = read_value_to_eol_or_brace(chars, k);
            if !val.is_empty() {
                label = Some(val);
            }
            let j = skip_whitespace(chars, val_end);
            if j < chars.len() && chars[j] == '{' {
                let (stmts, end) = parse_block(chars, j + 1, true)?;
                extract_shape_props(&stmts, &mut label, &mut shape_type, &mut style, &mut classes);
                body = stmts;
                i = end;
            } else {
                i = val_end;
            }
        } else {
            i = k;
        }
    } else if i < chars.len() && chars[i] == '{' {
        let (stmts, end) = parse_block(chars, i + 1, true)?;
        extract_shape_props(&stmts, &mut label, &mut shape_type, &mut style, &mut classes);
        body = stmts;
        i = end;
    }

    // Handle "style" path specially - if the path starts with "style", it's a style assignment
    // within a parent context, not a shape. We handle this differently.
    if path.len() >= 2 && path[0] == "style" {
        apply_style_path(&path[1..], &label, &mut style);
        return Ok((Some(Stmt::Shape {
            path: vec!["style".to_string()],
            label: None,
            shape_type: None,
            style,
            body: Vec::new(),
            classes: Vec::new(),
        }), skip_to_eol(chars, i)));
    }

    Ok((
        Some(Stmt::Shape {
            path,
            label,
            shape_type,
            style,
            body,
            classes,
        }),
        skip_to_eol(chars, i),
    ))
}

fn extract_shape_props(
    stmts: &[Stmt],
    label: &mut Option<String>,
    shape_type: &mut Option<String>,
    style: &mut StyleProps,
    classes: &mut Vec<String>,
) {
    for s in stmts {
        match s {
            Stmt::Shape {
                path,
                label: val,
                style: child_style,
                body,
                ..
            } => {
                if path.len() == 1 {
                    match path[0].as_str() {
                        "shape" => {
                            if let Some(v) = val {
                                *shape_type = Some(v.clone());
                            }
                        }
                        "label" => {
                            if let Some(v) = val {
                                *label = Some(v.clone());
                            }
                        }
                        "class" => {
                            if let Some(v) = val {
                                for c in v.split(';') {
                                    let c = c.trim();
                                    if !c.is_empty() {
                                        classes.push(c.to_string());
                                    }
                                }
                            }
                        }
                        "style" => {
                            merge_style(style, child_style);
                            // Also extract style properties from the body block
                            for bs in body {
                                if let Stmt::Shape { path: bp, label: bv, .. } = bs {
                                    if bp.len() == 1 {
                                        apply_style_path(bp, bv, style);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                } else if path.len() >= 2 && path[0] == "style" {
                    apply_style_path(&path[1..], val, style);
                }
            }
            _ => {}
        }
    }
}

fn apply_style_path(path: &[String], value: &Option<String>, style: &mut StyleProps) {
    if path.is_empty() {
        return;
    }
    let val = match value {
        Some(v) => v.as_str(),
        None => return,
    };
    match path[0].as_str() {
        "fill" => style.fill = parse_color(val),
        "stroke" => style.stroke = parse_color(val),
        "stroke-width" => style.stroke_width = val.parse().ok(),
        "stroke-dash" => style.stroke_dash = val.parse().ok(),
        "border-radius" => style.border_radius = val.parse().ok(),
        "opacity" => style.opacity = val.parse().ok(),
        "shadow" => style.shadow = parse_bool(val),
        "3d" => style.three_d = parse_bool(val),
        "multiple" => style.multiple = parse_bool(val),
        "double-border" => style.double_border = parse_bool(val),
        "font-size" => style.font_size = val.parse().ok(),
        "font-color" => style.color = parse_color(val),
        "bold" => style.bold = parse_bool(val),
        "italic" => style.italic = parse_bool(val),
        "fill-pattern" => style.fill_pattern = parse_fill_pattern(val),
        "animated" => style.animated = parse_bool(val),
        _ => {}
    }
}

fn extract_style_from_stmts(stmts: &[Stmt], style: &mut StyleProps) {
    for s in stmts {
        if let Stmt::Shape { path, label, style: child_style, .. } = s {
            if path.len() == 1 && path[0] == "style" {
                merge_style(style, child_style);
            } else if path.len() >= 2 && path[0] == "style" {
                apply_style_path(&path[1..], label, style);
            }
        }
    }
}

fn merge_style(target: &mut StyleProps, source: &StyleProps) {
    if source.fill.is_some() { target.fill = source.fill; }
    if source.stroke.is_some() { target.stroke = source.stroke; }
    if source.stroke_width.is_some() { target.stroke_width = source.stroke_width; }
    if source.color.is_some() { target.color = source.color; }
    if source.border_radius.is_some() { target.border_radius = source.border_radius; }
    if source.opacity.is_some() { target.opacity = source.opacity; }
    if source.stroke_dash.is_some() { target.stroke_dash = source.stroke_dash; }
    if source.shadow.is_some() { target.shadow = source.shadow; }
    if source.three_d.is_some() { target.three_d = source.three_d; }
    if source.multiple.is_some() { target.multiple = source.multiple; }
    if source.double_border.is_some() { target.double_border = source.double_border; }
    if source.font_size.is_some() { target.font_size = source.font_size; }
    if source.bold.is_some() { target.bold = source.bold; }
    if source.italic.is_some() { target.italic = source.italic; }
    if source.fill_pattern.is_some() { target.fill_pattern = source.fill_pattern; }
    if source.animated.is_some() { target.animated = source.animated; }
}

fn parse_classes_block(
    chars: &[char],
    start: usize,
) -> Result<(HashMap<String, ClassDef>, usize), Box<dyn std::error::Error>> {
    let mut classes = HashMap::new();
    let mut i = start;

    loop {
        i = skip_ws_and_newlines(chars, i);
        if i >= chars.len() {
            return Err("Unterminated classes block".into());
        }
        if chars[i] == '}' {
            i += 1;
            break;
        }
        if chars[i] == '#' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        let (name, after_name) = read_identifier(chars, i)?;
        let j = skip_whitespace(chars, after_name);

        let mut class_style = StyleProps::default();
        let mut class_shape = None;

        if j < chars.len() && chars[j] == ':' {
            let k = skip_whitespace(chars, j + 1);
            if k < chars.len() && chars[k] == '{' {
                let (stmts, end) = parse_block(chars, k + 1, true)?;
                let mut dummy_label = None;
                let mut dummy_classes = Vec::new();
                extract_shape_props(&stmts, &mut dummy_label, &mut class_shape, &mut class_style, &mut dummy_classes);
                i = end;
            } else {
                i = skip_to_eol(chars, j + 1);
            }
        } else if j < chars.len() && chars[j] == '{' {
            let (stmts, end) = parse_block(chars, j + 1, true)?;
            let mut dummy_label = None;
            let mut dummy_classes = Vec::new();
            extract_shape_props(&stmts, &mut dummy_label, &mut class_shape, &mut class_style, &mut dummy_classes);
            i = end;
        } else {
            i = skip_to_eol(chars, j);
        }

        classes.insert(name, ClassDef { style: class_style, shape_type: class_shape });
    }

    Ok((classes, i))
}

fn read_value_to_eol(chars: &[char], start: usize) -> (String, usize) {
    let i = start;
    if i < chars.len() && chars[i] == '"' {
        if let Ok((s, end)) = read_double_quoted(chars, i) {
            return (s, end);
        }
    }
    if i < chars.len() && chars[i] == '\'' {
        if let Ok((s, end)) = read_single_quoted(chars, i) {
            return (s, end);
        }
    }
    let mut end = i;
    while end < chars.len() && chars[end] != '\n' && chars[end] != '\r' && chars[end] != ';' && chars[end] != '#' {
        end += 1;
    }
    let val = chars[i..end].iter().collect::<String>().trim().to_string();
    (val, end)
}

fn read_value_to_eol_or_brace(chars: &[char], start: usize) -> (String, usize) {
    let i = start;
    if i < chars.len() && chars[i] == '"' {
        if let Ok((s, end)) = read_double_quoted(chars, i) {
            return (s, end);
        }
    }
    if i < chars.len() && chars[i] == '\'' {
        if let Ok((s, end)) = read_single_quoted(chars, i) {
            return (s, end);
        }
    }
    let mut end = i;
    while end < chars.len()
        && chars[end] != '\n'
        && chars[end] != '\r'
        && chars[end] != ';'
        && chars[end] != '#'
    {
        if chars[end] == '$' && end + 1 < chars.len() && chars[end + 1] == '{' {
            end += 2;
            while end < chars.len() && chars[end] != '}' {
                end += 1;
            }
            if end < chars.len() {
                end += 1;
            }
            continue;
        }
        if chars[end] == '{' || chars[end] == '}' {
            break;
        }
        end += 1;
    }
    let val = chars[i..end].iter().collect::<String>().trim().to_string();
    (val, end)
}

fn skip_to_eol(chars: &[char], pos: usize) -> usize {
    let mut i = pos;
    while i < chars.len() && chars[i] != '\n' && chars[i] != '\r' && chars[i] != ';' && chars[i] != '}' {
        i += 1;
    }
    i
}

fn parse_color(s: &str) -> Option<[u8; 3]> {
    let s = s.trim().trim_matches('"').trim_matches('\'');
    if s.starts_with('#') {
        let hex = &s[1..];
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some([r, g, b]);
        }
        if hex.len() == 3 {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            return Some([r, g, b]);
        }
    }
    // Named colors
    match s.to_lowercase().as_str() {
        "red" => Some([255, 0, 0]),
        "green" => Some([0, 128, 0]),
        "blue" => Some([0, 0, 255]),
        "yellow" => Some([255, 255, 0]),
        "orange" => Some([255, 165, 0]),
        "purple" => Some([128, 0, 128]),
        "black" => Some([0, 0, 0]),
        "white" => Some([255, 255, 255]),
        "gray" | "grey" => Some([128, 128, 128]),
        "cyan" => Some([0, 255, 255]),
        "magenta" => Some([255, 0, 255]),
        "pink" => Some([255, 192, 203]),
        "brown" => Some([165, 42, 42]),
        "navy" => Some([0, 0, 128]),
        "teal" => Some([0, 128, 128]),
        "lime" => Some([0, 255, 0]),
        _ => None,
    }
}

fn parse_bool(s: &str) -> Option<bool> {
    match s.trim().to_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn parse_fill_pattern(s: &str) -> Option<FillPattern> {
    match s.trim().to_lowercase().as_str() {
        "dots" => Some(FillPattern::Dots),
        "lines" => Some(FillPattern::Lines),
        "grain" => Some(FillPattern::Grain),
        "paper" => Some(FillPattern::Paper),
        _ => None,
    }
}

fn arrowhead_from_keyword(keyword: &str) -> Option<ArrowheadType> {
    match keyword.trim().to_lowercase().as_str() {
        "none" => Some(ArrowheadType::None),
        "arrow" => Some(ArrowheadType::Arrow),
        "triangle" => Some(ArrowheadType::Triangle),
        "unfilled-triangle" => Some(ArrowheadType::UnfilledTriangle),
        "diamond" => Some(ArrowheadType::Diamond),
        "filled-diamond" => Some(ArrowheadType::FilledDiamond),
        "circle" => Some(ArrowheadType::Circle),
        "filled-circle" => Some(ArrowheadType::FilledCircle),
        "cross" => Some(ArrowheadType::Cross),
        "box" => Some(ArrowheadType::Box),
        "filled-box" => Some(ArrowheadType::FilledBox),
        "line" => Some(ArrowheadType::Line),
        "cf-one" => Some(ArrowheadType::CfOne),
        "cf-many" => Some(ArrowheadType::CfMany),
        "cf-one-required" => Some(ArrowheadType::CfOneRequired),
        "cf-many-required" => Some(ArrowheadType::CfManyRequired),
        _ => None,
    }
}

fn extract_arrowhead_type(label: &Option<String>, body: &[Stmt]) -> Option<ArrowheadType> {
    // Check label for a shape keyword
    if let Some(l) = label {
        if let Some(ah) = arrowhead_from_keyword(l) {
            return Some(ah);
        }
    }
    // Check body for shape property
    for s in body {
        if let Stmt::Shape { path, label: val, .. } = s {
            if path.len() == 1 && path[0] == "shape" {
                if let Some(v) = val {
                    if let Some(ah) = arrowhead_from_keyword(v) {
                        return Some(ah);
                    }
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Compiler: AST -> DiagramGraph
// ---------------------------------------------------------------------------

fn compile(stmts: &[Stmt]) -> Result<DiagramGraph, Box<dyn std::error::Error>> {
    let mut graph = DiagramGraph {
        diagram_type: DiagramType::Flowchart,
        direction: Direction::TD,
        nodes: HashMap::new(),
        edges: Vec::new(),
        subgraphs: Vec::new(),
        styles: HashMap::new(),
        class_defs: HashMap::new(),
        layer_spacing: None,
        node_spacing: None,
        seq_activations: Vec::new(),
    };

    let mut class_defs: HashMap<String, ClassDef> = HashMap::new();

    // First pass: collect class definitions
    collect_classes(stmts, &mut class_defs);

    // Second pass: compile shapes and edges
    compile_stmts(stmts, &[], &mut graph, &class_defs);

    // Apply theme defaults (ID 0 = Neutral Default)
    let palette = theme::get_theme(0);

    let subgraph_node_ids: std::collections::HashSet<String> = graph.subgraphs
        .iter()
        .flat_map(|sg| sg.node_ids.iter().cloned())
        .collect();

    for (node_id, _node) in &graph.nodes {
        let style = graph.styles.entry(node_id.clone()).or_default();
        let is_container = subgraph_node_ids.contains(node_id);
        theme::apply_theme_to_node(style, &palette, is_container);
    }

    for edge in &mut graph.edges {
        theme::apply_theme_to_edge(&mut edge.style, &palette);
    }

    Ok(graph)
}

fn collect_classes(stmts: &[Stmt], class_defs: &mut HashMap<String, ClassDef>) {
    for stmt in stmts {
        if let Stmt::ClassesDef { classes } = stmt {
            for (name, def) in classes {
                class_defs.insert(name.clone(), def.clone());
            }
        }
    }
}

fn compile_stmts(
    stmts: &[Stmt],
    parent_path: &[String],
    graph: &mut DiagramGraph,
    class_defs: &HashMap<String, ClassDef>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::Direction(dir) => {
                graph.direction = *dir;
            }
            Stmt::ClassesDef { .. } => {
                // Already handled in first pass
            }
            Stmt::Shape {
                path,
                label,
                shape_type,
                style,
                body,
                classes,
            } => {
                if path.len() == 1 && matches!(path[0].as_str(), "style" | "shape" | "label" | "class" | "classes" | "vars" | "grid-rows" | "grid-columns" | "grid-gap" | "tooltip" | "link") {
                    continue;
                }

                let full_path = make_full_path(parent_path, path);
                let node_id = full_path.join(".");

                // Resolve shape type
                let shape = resolve_shape(shape_type.as_deref(), classes, class_defs);

                // Resolve style (class styles + inline styles)
                let mut resolved_style = StyleProps::default();
                for class_name in classes {
                    if let Some(cdef) = class_defs.get(class_name) {
                        merge_style(&mut resolved_style, &cdef.style);
                    }
                }
                merge_style(&mut resolved_style, style);

                let node_label = label.clone().unwrap_or_else(|| {
                    path.last().unwrap_or(&String::new()).clone()
                });

                // Class and SqlTable shapes have child entries but are leaf nodes
                let is_structured_shape = matches!(shape, NodeShape::Class | NodeShape::SqlTable);

                let has_children = !is_structured_shape && body.iter().any(|s| matches!(s,
                    Stmt::Shape { path: p, .. } if !(p.len() == 1 && matches!(p[0].as_str(), "style" | "shape" | "label" | "class" | "classes" | "vars" | "grid-rows" | "grid-columns" | "grid-gap" | "tooltip" | "link" | "near"))
                ) || matches!(s, Stmt::Edge { .. }));

                let has_style = resolved_style.fill.is_some()
                    || resolved_style.stroke.is_some()
                    || resolved_style.stroke_width.is_some()
                    || resolved_style.color.is_some()
                    || resolved_style.border_radius.is_some()
                    || resolved_style.opacity.is_some()
                    || resolved_style.stroke_dash.is_some()
                    || resolved_style.shadow.is_some()
                    || resolved_style.three_d.is_some()
                    || resolved_style.multiple.is_some()
                    || resolved_style.double_border.is_some()
                    || resolved_style.font_size.is_some()
                    || resolved_style.bold.is_some()
                    || resolved_style.italic.is_some();

                if has_children {
                    // Recurse into the container body
                    compile_stmts(body, &full_path, graph, class_defs);

                    let (grid_rows, grid_columns, grid_gap) = extract_grid_props(body);

                    let child_ids: Vec<String> = collect_child_node_ids(body, &full_path);
                    if !child_ids.is_empty() {
                        graph.subgraphs.push(SubgraphDef {
                            title: node_label,
                            node_ids: child_ids,
                            grid_rows,
                            grid_columns,
                            grid_gap,
                            branches: Vec::new(),
                        });
                    }
                } else {
                    // Leaf node — add to graph
                    let (class_fields, class_methods, sql_columns) =
                        if shape == NodeShape::Class {
                            parse_class_body(body)
                        } else if shape == NodeShape::SqlTable {
                            let cols = parse_sql_table_body(body);
                            (Vec::new(), Vec::new(), cols)
                        } else {
                            (Vec::new(), Vec::new(), Vec::new())
                        };

                    let near = extract_near(body);
                    let (tooltip, link) = extract_tooltip_link(body);

                    graph.nodes.insert(
                        node_id.clone(),
                        NodeDef {
                            label: node_label,
                            shape,
                            classes: classes.clone(),
                            class_fields,
                            class_methods,
                            sql_columns,
                            near,
                            tooltip,
                            link,
                        },
                    );

                    if has_style {
                        graph.styles.insert(node_id.clone(), resolved_style);
                    }
                }
            }
            Stmt::Edge {
                points,
                arrow_types,
                label,
                style: edge_style,
                src_arrowhead,
                dst_arrowhead,
                ..
            } => {
                for i in 0..points.len() - 1 {
                    let src_path = make_full_path(parent_path, &points[i]);
                    let dst_path = make_full_path(parent_path, &points[i + 1]);
                    let src_id = src_path.join(".");
                    let dst_id = dst_path.join(".");

                    let arrow = arrow_types.get(i).copied().unwrap_or(ArrowType::Forward);

                    ensure_node(graph, &src_id, &points[i]);
                    ensure_node(graph, &dst_id, &points[i + 1]);

                    let edge_type = match arrow {
                        ArrowType::Forward => EdgeType::Arrow,
                        ArrowType::Backward => EdgeType::Arrow,
                        ArrowType::Both => EdgeType::BidiArrow,
                        ArrowType::Undirected => EdgeType::Line,
                    };

                    let (from, to) = match arrow {
                        ArrowType::Backward => (dst_id, src_id),
                        _ => (src_id, dst_id),
                    };

                    let edge_label = if i == 0 { label.clone() } else { None };

                    graph.edges.push(EdgeDef {
                        from,
                        to,
                        edge_type,
                        label: edge_label,
                        src_arrowhead: *src_arrowhead,
                        dst_arrowhead: *dst_arrowhead,
                        style: edge_style.clone(),
                    });
                }
            }
            Stmt::Import { .. } => {}
        }
    }
}

fn make_full_path(parent: &[String], path: &[String]) -> Vec<String> {
    let mut full = parent.to_vec();
    full.extend_from_slice(path);
    full
}

fn ensure_node(graph: &mut DiagramGraph, id: &str, path: &[String]) {
    if !graph.nodes.contains_key(id) {
        graph.nodes.insert(
            id.to_string(),
            NodeDef {
                label: path.last().unwrap_or(&String::new()).clone(),
                shape: NodeShape::Rounded,
                classes: Vec::new(),
                class_fields: Vec::new(),
                class_methods: Vec::new(),
                sql_columns: Vec::new(),
                near: None,
                tooltip: None,
                link: None,
            },
        );
    }
}

fn extract_near(body: &[Stmt]) -> Option<NearPosition> {
    for s in body {
        if let Stmt::Shape { path, label: Some(val), .. } = s {
            if path.len() == 1 && path[0] == "near" {
                return match val.as_str() {
                    "top-left" => Some(NearPosition::TopLeft),
                    "top-center" => Some(NearPosition::TopCenter),
                    "top-right" => Some(NearPosition::TopRight),
                    "center-left" => Some(NearPosition::CenterLeft),
                    "center-right" => Some(NearPosition::CenterRight),
                    "bottom-left" => Some(NearPosition::BottomLeft),
                    "bottom-center" => Some(NearPosition::BottomCenter),
                    "bottom-right" => Some(NearPosition::BottomRight),
                    _ => None,
                };
            }
        }
    }
    None
}

fn extract_tooltip_link(body: &[Stmt]) -> (Option<String>, Option<String>) {
    let mut tooltip = None;
    let mut link = None;
    for s in body {
        if let Stmt::Shape { path, label: Some(val), .. } = s {
            if path.len() == 1 {
                match path[0].as_str() {
                    "tooltip" => tooltip = Some(val.clone()),
                    "link" => link = Some(val.clone()),
                    _ => {}
                }
            }
        }
    }
    (tooltip, link)
}

fn extract_grid_props(body: &[Stmt]) -> (Option<usize>, Option<usize>, Option<f32>) {
    let mut rows = None;
    let mut cols = None;
    let mut gap = None;
    for s in body {
        if let Stmt::Shape { path, label: Some(val), .. } = s {
            if path.len() == 1 {
                match path[0].as_str() {
                    "grid-rows" => rows = val.parse().ok(),
                    "grid-columns" => cols = val.parse().ok(),
                    "grid-gap" => gap = val.parse().ok(),
                    _ => {}
                }
            }
        }
    }
    (rows, cols, gap)
}

fn resolve_shape(
    shape_type: Option<&str>,
    classes: &[String],
    class_defs: &HashMap<String, ClassDef>,
) -> NodeShape {
    // Check explicit shape type first
    if let Some(st) = shape_type {
        if let Some(shape) = shape_from_keyword(st) {
            return shape;
        }
    }
    // Check class-defined shape
    for class_name in classes {
        if let Some(cdef) = class_defs.get(class_name) {
            if let Some(ref st) = cdef.shape_type {
                if let Some(shape) = shape_from_keyword(st) {
                    return shape;
                }
            }
        }
    }
    NodeShape::Rounded // D2 default
}

fn shape_from_keyword(keyword: &str) -> Option<NodeShape> {
    match keyword.to_lowercase().as_str() {
        "rectangle" => Some(NodeShape::Rect),
        "square" => Some(NodeShape::Rect),
        "page" => Some(NodeShape::Page),
        "parallelogram" => Some(NodeShape::Parallelogram),
        "document" => Some(NodeShape::Document),
        "cylinder" => Some(NodeShape::Cylinder),
        "queue" => Some(NodeShape::Queue),
        "package" => Some(NodeShape::Package),
        "step" => Some(NodeShape::Step),
        "callout" => Some(NodeShape::Callout),
        "stored_data" => Some(NodeShape::StoredData),
        "person" => Some(NodeShape::Person),
        "c4-person" => Some(NodeShape::Person),
        "diamond" => Some(NodeShape::Diamond),
        "oval" => Some(NodeShape::Oval),
        "circle" => Some(NodeShape::Circle),
        "hexagon" => Some(NodeShape::Hexagon),
        "cloud" => Some(NodeShape::Cloud),
        "text" => Some(NodeShape::Text),
        "class" => Some(NodeShape::Class),
        "sql_table" => Some(NodeShape::SqlTable),
        _ => None,
    }
}

fn parse_class_body(body: &[Stmt]) -> (Vec<ClassField>, Vec<ClassMethod>, Vec<SqlColumn>) {
    let mut fields = Vec::new();
    let mut methods = Vec::new();

    for stmt in body {
        if let Stmt::Shape { path, label, .. } = stmt {
            if path.len() == 1 && !matches!(path[0].as_str(), "shape" | "style" | "label" | "class" | "classes") {
                let name = &path[0];
                let type_str = label.clone().unwrap_or_default();

                let (vis, clean_name) = extract_visibility(name);

                if clean_name.contains('(') {
                    methods.push(ClassMethod {
                        visibility: vis,
                        name: clean_name,
                        return_type: type_str,
                    });
                } else {
                    fields.push(ClassField {
                        visibility: vis,
                        name: clean_name,
                        type_str,
                    });
                }
            }
        }
    }

    (fields, methods, Vec::new())
}

fn parse_sql_table_body(body: &[Stmt]) -> Vec<SqlColumn> {
    let mut columns = Vec::new();

    for stmt in body {
        if let Stmt::Shape { path, label, body: col_body, .. } = stmt {
            if path.len() == 1 && !matches!(path[0].as_str(), "shape" | "style" | "label" | "class" | "classes") {
                let name = path[0].clone();
                let type_str = label.clone().unwrap_or_default();

                let mut constraint = String::new();
                for bs in col_body {
                    if let Stmt::Shape { path: bp, label: bv, .. } = bs {
                        if bp.len() == 1 && bp[0] == "constraint" {
                            constraint = bv.clone().unwrap_or_default();
                        }
                    }
                }

                columns.push(SqlColumn {
                    name,
                    type_str,
                    constraint,
                });
            }
        }
    }

    columns
}

fn extract_visibility(name: &str) -> (char, String) {
    let first = name.chars().next().unwrap_or(' ');
    match first {
        '+' | '-' | '#' => (first, name[1..].to_string()),
        _ => (' ', name.to_string()),
    }
}

const MAX_IMPORT_DEPTH: usize = 10;

fn resolve_imports(stmts: &mut Vec<Stmt>, base_dir: Option<&Path>, depth: usize) -> Result<(), Box<dyn std::error::Error>> {
    if depth > MAX_IMPORT_DEPTH {
        return Err("Import depth exceeded (circular import?)".into());
    }
    let base = match base_dir {
        Some(d) => d.to_path_buf(),
        None => return Ok(()),
    };

    let mut i = 0;
    while i < stmts.len() {
        match &stmts[i] {
            Stmt::Import { path, spread, target_path } => {
                let resolved = base.join(path);
                let source = std::fs::read_to_string(&resolved)
                    .map_err(|e| format!("Import {}: {}", path, e))?;
                let import_base = resolved.parent().map(|p| p.to_path_buf());
                let mut imported = parse_map(&source)?;
                resolve_imports(&mut imported, import_base.as_deref(), depth + 1)?;

                if *spread {
                    stmts.remove(i);
                    for (j, s) in imported.into_iter().enumerate() {
                        stmts.insert(i + j, s);
                    }
                } else {
                    let tp = target_path.clone();
                    let mut label = None;
                    let mut shape_type = None;
                    let mut style = StyleProps::default();
                    let mut classes = vec![];
                    extract_shape_props(&imported, &mut label, &mut shape_type, &mut style, &mut classes);
                    stmts[i] = Stmt::Shape {
                        path: tp,
                        label,
                        shape_type,
                        style,
                        body: imported,
                        classes,
                    };
                    i += 1;
                }
            }
            Stmt::Shape { body, .. } if !body.is_empty() => {
                if let Stmt::Shape { body, .. } = &mut stmts[i] {
                    resolve_imports(body, Some(&base), depth)?;
                }
                i += 1;
            }
            _ => { i += 1; }
        }
    }
    Ok(())
}

fn collect_vars(stmts: &[Stmt], vars: &mut HashMap<String, String>) {
    for stmt in stmts {
        if let Stmt::Shape { path, body, .. } = stmt {
            if path.len() == 1 && path[0] == "vars" {
                for s in body {
                    if let Stmt::Shape { path: p, label: Some(val), .. } = s {
                        let key = p.join(".");
                        vars.insert(key, val.clone());
                    }
                }
            }
        }
    }
}

fn substitute_vars(stmts: &mut [Stmt], vars: &HashMap<String, String>) {
    for stmt in stmts.iter_mut() {
        match stmt {
            Stmt::Shape { label, body, .. } => {
                if let Some(l) = label {
                    *l = apply_var_substitution(l, vars);
                }
                substitute_vars(body, vars);
            }
            Stmt::Edge { label, .. } => {
                if let Some(l) = label {
                    *l = apply_var_substitution(l, vars);
                }
            }
            _ => {}
        }
    }
}

fn apply_var_substitution(s: &str, vars: &HashMap<String, String>) -> String {
    let mut result = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '$' && chars[i + 1] == '{' {
            let start = i + 2;
            if let Some(end) = chars[start..].iter().position(|&c| c == '}') {
                let key: String = chars[start..start + end].iter().collect();
                if let Some(val) = vars.get(&key) {
                    result.push_str(val);
                } else {
                    result.push_str(&format!("${{{}}}", key));
                }
                i = start + end + 1;
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }
    result
}

fn collect_child_node_ids(stmts: &[Stmt], parent_path: &[String]) -> Vec<String> {
    let mut ids = Vec::new();
    for stmt in stmts {
        match stmt {
            Stmt::Shape { path, .. } => {
                if path.len() == 1 && matches!(path[0].as_str(), "style" | "shape" | "label" | "class" | "classes" | "vars" | "grid-rows" | "grid-columns" | "grid-gap" | "tooltip" | "link") {
                    continue;
                }
                let full = make_full_path(parent_path, path);
                ids.push(full.join("."));
            }
            Stmt::Edge { points, .. } => {
                for p in points {
                    let full = make_full_path(parent_path, p);
                    let id = full.join(".");
                    if !ids.contains(&id) {
                        ids.push(id);
                    }
                }
            }
            _ => {}
        }
    }
    ids
}
