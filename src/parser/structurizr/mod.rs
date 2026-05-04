pub mod ast;

use ast::*;
use crate::diagram::*;
use std::collections::HashMap;


fn strip_comments(source: &str) -> String {
    let mut result = String::new();
    let mut in_block_comment = false;
    for line in source.lines() {
        if in_block_comment {
            if let Some(pos) = line.find("*/") {
                in_block_comment = false;
                result.push_str(&line[pos + 2..]);
                result.push('\n');
            } else {
                result.push('\n');
            }
            continue;
        }
        if let Some(pos) = find_block_comment_start(line) {
            let before = &line[..pos];
            let after = &line[pos + 2..];
            if let Some(end) = after.find("*/") {
                result.push_str(before);
                result.push_str(&after[end + 2..]);
                result.push('\n');
            } else {
                result.push_str(before);
                result.push('\n');
                in_block_comment = true;
            }
            continue;
        }
        let effective = strip_line_comment(line);
        result.push_str(effective);
        result.push('\n');
    }
    result
}

fn find_block_comment_start(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut in_quote = false;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            if in_quote {
                if i > 0 && bytes[i - 1] == b'\\' {
                    i += 1;
                    continue;
                }
                in_quote = false;
            } else {
                in_quote = true;
            }
        } else if !in_quote && bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn strip_line_comment(line: &str) -> &str {
    let mut in_quote = false;
    let mut seen_non_whitespace = false;
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            seen_non_whitespace = true;
            if in_quote {
                if i > 0 && bytes[i - 1] == b'\\' {
                    i += 1;
                    continue;
                }
                in_quote = false;
            } else {
                in_quote = true;
            }
        } else if !in_quote {
            if bytes[i] == b'#' && !seen_non_whitespace {
                return &line[..i];
            }
            if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                return &line[..i];
            }
            if !bytes[i].is_ascii_whitespace() {
                seen_non_whitespace = true;
            }
        }
        i += 1;
    }
    line
}

fn tokenize_line(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return tokens;
    }
    let mut chars = trimmed.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        if c == '"' {
            chars.next();
            let mut s = String::new();
            while let Some(&ch) = chars.peek() {
                if ch == '\\' {
                    chars.next();
                    if let Some(&next) = chars.peek() {
                        s.push(next);
                        chars.next();
                    }
                } else if ch == '"' {
                    chars.next();
                    break;
                } else {
                    s.push(ch);
                    chars.next();
                }
            }
            tokens.push(format!("\"{}\"", s));
        } else if c == '{' {
            tokens.push("{".to_string());
            chars.next();
        } else if c == '}' {
            tokens.push("}".to_string());
            chars.next();
        } else if c == '*' {
            tokens.push("*".to_string());
            chars.next();
        } else if c == '-' {
            chars.next();
            if let Some(&'>') = chars.peek() {
                chars.next();
                tokens.push("->".to_string());
            } else {
                let mut word = String::from('-');
                while let Some(&ch) = chars.peek() {
                    if ch.is_whitespace() || ch == '{' || ch == '}' || ch == '"' {
                        break;
                    }
                    word.push(ch);
                    chars.next();
                }
                tokens.push(word);
            }
        } else if c == '=' {
            tokens.push("=".to_string());
            chars.next();
        } else {
            let mut word = String::new();
            while let Some(&ch) = chars.peek() {
                if ch.is_whitespace() || ch == '{' || ch == '}' || ch == '"' || ch == '=' {
                    break;
                }
                word.push(ch);
                chars.next();
            }
            tokens.push(word);
        }
    }
    tokens
}

fn unquote(s: &str) -> String {
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

fn is_quoted(s: &str) -> bool {
    s.starts_with('"') && s.ends_with('"')
}

fn is_element_keyword(s: &str) -> bool {
    let lower = s.to_lowercase();
    matches!(
        lower.as_str(),
        "person" | "softwaresystem" | "container" | "component"
    )
}

fn element_kind_from_keyword(s: &str) -> Option<ElementKind> {
    match s.to_lowercase().as_str() {
        "person" => Some(ElementKind::Person),
        "softwaresystem" => Some(ElementKind::SoftwareSystem),
        "container" => Some(ElementKind::Container),
        "component" => Some(ElementKind::Component),
        _ => None,
    }
}

fn default_tags_for_kind(kind: &ElementKind) -> Vec<String> {
    match kind {
        ElementKind::Person => vec!["Element".into(), "Person".into()],
        ElementKind::SoftwareSystem => vec!["Element".into(), "Software System".into()],
        ElementKind::Container => vec!["Element".into(), "Container".into()],
        ElementKind::Component => vec!["Element".into(), "Component".into()],
    }
}

fn sanitize_id(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect()
}

pub fn parse_workspace_v2(source: &str) -> Result<Workspace, Box<dyn std::error::Error>> {
    let cleaned = strip_comments(source);
    let lines: Vec<Vec<String>> = cleaned
        .lines()
        .map(tokenize_line)
        .filter(|t| !t.is_empty())
        .collect();

    let mut pos = 0;

    // Find workspace
    while pos < lines.len() {
        if lines[pos].first().map(|t| t.to_lowercase()) == Some("workspace".into()) {
            break;
        }
        pos += 1;
    }
    if pos >= lines.len() {
        return Err("no workspace found".into());
    }

    // Skip workspace line and opening brace
    if !lines[pos].contains(&"{".to_string()) {
        pos += 1;
        if pos < lines.len() && lines[pos].first() == Some(&"{".to_string()) {
            pos += 1;
        }
    } else {
        pos += 1;
    }

    let mut model = Model {
        elements: Vec::new(),
        relationships: Vec::new(),
    };
    let mut views = Views {
        view_defs: Vec::new(),
        styles: StylesDef::default(),
    };

    while pos < lines.len() {
        if lines[pos].first() == Some(&"}".to_string()) {
            break;
        }
        match lines[pos].first().map(|t| t.to_lowercase()).as_deref() {
            Some("model") => {
                pos = parse_model_v2(&lines, pos, &mut model)?;
            }
            Some("views") => {
                pos = parse_views_v2(&lines, pos, &mut views)?;
            }
            _ => {
                pos = skip_block_or_line_v2(&lines, pos);
            }
        }
    }

    Ok(Workspace { model, views })
}

fn parse_model_v2(
    lines: &[Vec<String>],
    start: usize,
    model: &mut Model,
) -> Result<usize, Box<dyn std::error::Error>> {
    let mut pos = start;
    let has_brace = lines[pos].contains(&"{".to_string());
    pos += 1;
    if !has_brace {
        if pos < lines.len() && lines[pos].first() == Some(&"{".to_string()) {
            pos += 1;
        }
    }

    pos = parse_element_body_v2(lines, pos, &mut model.elements, &mut model.relationships, None)?;
    Ok(pos)
}

fn parse_element_body_v2(
    lines: &[Vec<String>],
    mut pos: usize,
    elements: &mut Vec<Element>,
    relationships: &mut Vec<Relationship>,
    parent_id: Option<&str>,
) -> Result<usize, Box<dyn std::error::Error>> {
    while pos < lines.len() {
        let tokens = &lines[pos];
        if tokens.first() == Some(&"}".to_string()) {
            pos += 1;
            return Ok(pos);
        }

        // Relationship: IDENT -> IDENT ...
        if tokens.len() >= 3 && tokens[1] == "->" && !is_element_keyword(&tokens[0]) {
            let (rel, new_pos) = parse_relationship_v2(lines, pos, parent_id)?;
            relationships.push(rel);
            pos = new_pos;
            continue;
        }

        // Element with id assignment: IDENT = keyword ...
        let (id, keyword_idx) = if tokens.len() >= 3
            && tokens[1] == "="
            && is_element_keyword(&tokens[2])
        {
            (Some(tokens[0].clone()), 2)
        } else if is_element_keyword(&tokens[0]) {
            (None, 0)
        } else {
            pos = skip_block_or_line_v2(lines, pos);
            continue;
        };

        let kind = element_kind_from_keyword(&tokens[keyword_idx]).unwrap();
        let (elem, new_pos) = parse_element_def_v2(lines, pos, id, kind, relationships)?;
        elements.push(elem);
        pos = new_pos;
    }
    Ok(pos)
}

fn parse_element_def_v2(
    lines: &[Vec<String>],
    pos: usize,
    assigned_id: Option<String>,
    kind: ElementKind,
    relationships: &mut Vec<Relationship>,
) -> Result<(Element, usize), Box<dyn std::error::Error>> {
    let tokens = &lines[pos];

    let kw_idx = tokens
        .iter()
        .position(|t| element_kind_from_keyword(t) == Some(kind.clone()))
        .ok_or("keyword not found")?;

    let after_kw: Vec<&str> = tokens[kw_idx + 1..]
        .iter()
        .filter(|t| is_quoted(t))
        .map(|t| t.as_str())
        .collect();

    let has_block = tokens.contains(&"{".to_string());

    let (name, description, technology, tags_str) = match kind {
        ElementKind::Person | ElementKind::SoftwareSystem => {
            let name = after_kw.first().map(|s| unquote(s)).unwrap_or_default();
            let desc = after_kw.get(1).map(|s| unquote(s));
            let tags = after_kw.get(2).map(|s| unquote(s));
            (name, desc, None, tags)
        }
        ElementKind::Container | ElementKind::Component => {
            let name = after_kw.first().map(|s| unquote(s)).unwrap_or_default();
            let desc = after_kw.get(1).map(|s| unquote(s));
            let tech = after_kw.get(2).map(|s| unquote(s));
            let tags = after_kw.get(3).map(|s| unquote(s));
            (name, desc, tech, tags)
        }
    };

    let id = assigned_id.unwrap_or_else(|| sanitize_id(&name));

    let mut tags = default_tags_for_kind(&kind);
    if let Some(custom) = tags_str {
        for t in custom.split(',') {
            let t = t.trim();
            if !t.is_empty() {
                tags.push(t.to_string());
            }
        }
    }

    let mut next_pos = pos + 1;
    let mut children = Vec::new();

    if has_block {
        // Check if brace is on next line
        next_pos =
            parse_element_body_v2(lines, next_pos, &mut children, relationships, Some(&id))?;
    } else if next_pos < lines.len() && lines[next_pos].first() == Some(&"{".to_string()) {
        next_pos += 1;
        next_pos =
            parse_element_body_v2(lines, next_pos, &mut children, relationships, Some(&id))?;
    }

    Ok((
        Element {
            id,
            kind,
            name,
            description,
            technology,
            tags,
            children,
        },
        next_pos,
    ))
}

fn parse_relationship_v2(
    lines: &[Vec<String>],
    pos: usize,
    parent_id: Option<&str>,
) -> Result<(Relationship, usize), Box<dyn std::error::Error>> {
    let tokens = &lines[pos];
    let arrow_idx = tokens
        .iter()
        .position(|t| t == "->")
        .ok_or("no -> in relationship")?;

    let source = if arrow_idx == 0 {
        parent_id
            .ok_or("relationship without source and no parent")?
            .to_string()
    } else {
        tokens[0].clone()
    };

    let target = tokens
        .get(arrow_idx + 1)
        .ok_or("no target in relationship")?
        .clone();

    let strings: Vec<String> = tokens[arrow_idx + 2..]
        .iter()
        .filter(|t| is_quoted(t))
        .map(|t| unquote(t))
        .collect();

    let description = strings.first().cloned();
    let technology = strings.get(1).cloned();

    let mut next_pos = pos + 1;
    if tokens.contains(&"{".to_string()) {
        next_pos = skip_until_close_brace_v2(lines, next_pos);
    }

    Ok((
        Relationship {
            source_id: source,
            target_id: target,
            description,
            technology,
        },
        next_pos,
    ))
}

fn parse_views_v2(
    lines: &[Vec<String>],
    start: usize,
    views: &mut Views,
) -> Result<usize, Box<dyn std::error::Error>> {
    let mut pos = start;
    let has_brace = lines[pos].contains(&"{".to_string());
    pos += 1;
    if !has_brace {
        if pos < lines.len() && lines[pos].first() == Some(&"{".to_string()) {
            pos += 1;
        }
    }

    while pos < lines.len() {
        let tokens = &lines[pos];
        if tokens.first() == Some(&"}".to_string()) {
            pos += 1;
            return Ok(pos);
        }

        match tokens.first().map(|t| t.to_lowercase()).as_deref() {
            Some("systemcontext") => {
                let (vd, new_pos) = parse_view_def_v2(lines, pos, ViewKind::SystemContext)?;
                views.view_defs.push(vd);
                pos = new_pos;
            }
            Some("container") => {
                let (vd, new_pos) = parse_view_def_v2(lines, pos, ViewKind::Container)?;
                views.view_defs.push(vd);
                pos = new_pos;
            }
            Some("component") => {
                let (vd, new_pos) = parse_view_def_v2(lines, pos, ViewKind::Component)?;
                views.view_defs.push(vd);
                pos = new_pos;
            }
            Some("systemlandscape") => {
                let (vd, new_pos) = parse_view_def_v2(lines, pos, ViewKind::SystemLandscape)?;
                views.view_defs.push(vd);
                pos = new_pos;
            }
            Some("styles") => {
                let (sd, new_pos) = parse_styles_block_v2(lines, pos)?;
                views.styles = sd;
                pos = new_pos;
            }
            _ => {
                pos = skip_block_or_line_v2(lines, pos);
            }
        }
    }
    Ok(pos)
}

fn parse_view_def_v2(
    lines: &[Vec<String>],
    start: usize,
    kind: ViewKind,
) -> Result<(ViewDef, usize), Box<dyn std::error::Error>> {
    let tokens = &lines[start];

    let target_id = if kind == ViewKind::SystemLandscape {
        None
    } else {
        tokens
            .get(1)
            .filter(|t| !is_quoted(t) && *t != "{")
            .cloned()
    };

    let key = tokens.iter().find(|t| is_quoted(t)).map(|t| unquote(t));

    let has_brace = tokens.contains(&"{".to_string());
    let mut pos = start + 1;
    if !has_brace {
        if pos < lines.len() && lines[pos].first() == Some(&"{".to_string()) {
            pos += 1;
        }
    }

    let mut auto_layout = None;

    while pos < lines.len() {
        let tokens = &lines[pos];
        if tokens.first() == Some(&"}".to_string()) {
            pos += 1;
            break;
        }
        match tokens.first().map(|t| t.to_lowercase()).as_deref() {
            Some("autolayout") => {
                let direction = tokens
                    .get(1)
                    .filter(|t| !t.parse::<u32>().is_ok())
                    .cloned();
                let rank_sep = tokens.iter().skip(1).find_map(|t| t.parse::<u32>().ok());
                let node_sep = tokens.iter().skip(2).find_map(|t| t.parse::<u32>().ok());
                auto_layout = Some(AutoLayout {
                    direction,
                    rank_sep,
                    node_sep,
                });
                pos += 1;
            }
            _ => {
                pos += 1;
            }
        }
    }

    Ok((
        ViewDef {
            kind,
            target_id,
            key,
            auto_layout,
        },
        pos,
    ))
}

fn parse_styles_block_v2(
    lines: &[Vec<String>],
    start: usize,
) -> Result<(StylesDef, usize), Box<dyn std::error::Error>> {
    let has_brace = lines[start].contains(&"{".to_string());
    let mut pos = start + 1;
    if !has_brace {
        if pos < lines.len() && lines[pos].first() == Some(&"{".to_string()) {
            pos += 1;
        }
    }

    let mut element_styles = Vec::new();
    let mut relationship_styles = Vec::new();

    while pos < lines.len() {
        let tokens = &lines[pos];
        if tokens.first() == Some(&"}".to_string()) {
            pos += 1;
            break;
        }
        match tokens.first().map(|t| t.to_lowercase()).as_deref() {
            Some("element") => {
                let (es, new_pos) = parse_element_style_v2(lines, pos)?;
                element_styles.push(es);
                pos = new_pos;
            }
            Some("relationship") => {
                let (rs, new_pos) = parse_relationship_style_v2(lines, pos)?;
                relationship_styles.push(rs);
                pos = new_pos;
            }
            _ => {
                pos = skip_block_or_line_v2(lines, pos);
            }
        }
    }

    Ok((
        StylesDef {
            element_styles,
            relationship_styles,
        },
        pos,
    ))
}

fn parse_element_style_v2(
    lines: &[Vec<String>],
    start: usize,
) -> Result<(ElementStyleDef, usize), Box<dyn std::error::Error>> {
    let tokens = &lines[start];
    let tag = tokens
        .iter()
        .find(|t| is_quoted(t))
        .map(|t| unquote(t))
        .ok_or("element style missing tag")?;

    let has_brace = tokens.contains(&"{".to_string());
    let mut pos = start + 1;
    if !has_brace {
        if pos < lines.len() && lines[pos].first() == Some(&"{".to_string()) {
            pos += 1;
        }
    }

    let mut background = None;
    let mut color = None;
    let mut shape = None;

    while pos < lines.len() {
        let tokens = &lines[pos];
        if tokens.first() == Some(&"}".to_string()) {
            pos += 1;
            break;
        }
        match tokens.first().map(|t| t.to_lowercase()).as_deref() {
            Some("background") => {
                background = tokens.get(1).cloned();
            }
            Some("color") | Some("colour") => {
                color = tokens.get(1).cloned();
            }
            Some("shape") => {
                shape = tokens.get(1).cloned();
            }
            _ => {}
        }
        pos += 1;
    }

    Ok((
        ElementStyleDef {
            tag,
            background,
            color,
            shape,
        },
        pos,
    ))
}

fn parse_relationship_style_v2(
    lines: &[Vec<String>],
    start: usize,
) -> Result<(RelationshipStyleDef, usize), Box<dyn std::error::Error>> {
    let tokens = &lines[start];
    let tag = tokens
        .iter()
        .find(|t| is_quoted(t))
        .map(|t| unquote(t))
        .ok_or("relationship style missing tag")?;

    let has_brace = tokens.contains(&"{".to_string());
    let mut pos = start + 1;
    if !has_brace {
        if pos < lines.len() && lines[pos].first() == Some(&"{".to_string()) {
            pos += 1;
        }
    }

    let mut color = None;
    let mut dashed = None;
    let mut thickness = None;

    while pos < lines.len() {
        let tokens = &lines[pos];
        if tokens.first() == Some(&"}".to_string()) {
            pos += 1;
            break;
        }
        match tokens.first().map(|t| t.to_lowercase()).as_deref() {
            Some("color") | Some("colour") => {
                color = tokens.get(1).cloned();
            }
            Some("dashed") => {
                dashed = tokens.get(1).map(|v| v.to_lowercase() == "true");
            }
            Some("thickness") => {
                thickness = tokens.get(1).and_then(|v| v.parse().ok());
            }
            _ => {}
        }
        pos += 1;
    }

    Ok((
        RelationshipStyleDef {
            tag,
            color,
            dashed,
            thickness,
        },
        pos,
    ))
}

fn skip_block_or_line_v2(lines: &[Vec<String>], pos: usize) -> usize {
    if pos >= lines.len() {
        return pos;
    }
    let has_brace = lines[pos].contains(&"{".to_string());
    let mut next = pos + 1;
    if has_brace {
        next = skip_until_close_brace_v2(lines, next);
    } else if next < lines.len() && lines[next].first() == Some(&"{".to_string()) {
        next += 1;
        next = skip_until_close_brace_v2(lines, next);
    }
    next
}

fn skip_until_close_brace_v2(lines: &[Vec<String>], mut pos: usize) -> usize {
    let mut depth = 1;
    while pos < lines.len() && depth > 0 {
        for t in &lines[pos] {
            if t == "{" {
                depth += 1;
            } else if t == "}" {
                depth -= 1;
                if depth == 0 {
                    return pos + 1;
                }
            }
        }
        pos += 1;
    }
    pos
}

// ---- View Resolution & DiagramGraph Conversion ----

fn build_id_map(elements: &[Element]) -> HashMap<String, &Element> {
    let mut map = HashMap::new();
    for elem in elements {
        map.insert(elem.id.clone(), elem);
        build_id_map_inner(&elem.children, &mut map);
    }
    map
}

fn build_id_map_inner<'a>(elements: &'a [Element], map: &mut HashMap<String, &'a Element>) {
    for elem in elements {
        map.insert(elem.id.clone(), elem);
        build_id_map_inner(&elem.children, map);
    }
}

fn find_element<'a>(elements: &'a [Element], id: &str) -> Option<&'a Element> {
    for elem in elements {
        if elem.id == id {
            return Some(elem);
        }
        if let Some(found) = find_element(&elem.children, id) {
            return Some(found);
        }
    }
    None
}

fn find_parent_system<'a>(elements: &'a [Element], child_id: &str) -> Option<&'a Element> {
    for elem in elements {
        if elem.kind == ElementKind::SoftwareSystem {
            if elem.children.iter().any(|c| c.id == child_id) {
                return Some(elem);
            }
            for container in &elem.children {
                if container.children.iter().any(|c| c.id == child_id) {
                    return Some(elem);
                }
            }
        }
    }
    None
}

fn all_child_ids(elem: &Element) -> Vec<&str> {
    let mut ids = Vec::new();
    for child in &elem.children {
        ids.push(child.id.as_str());
        ids.extend(all_child_ids(child));
    }
    ids
}

fn is_connected_to_set(
    elem_id: &str,
    elem: &Element,
    target_ids: &[&str],
    relationships: &[Relationship],
) -> bool {
    let ids: Vec<&str> = std::iter::once(elem_id)
        .chain(all_child_ids(elem))
        .collect();
    relationships.iter().any(|r| {
        (ids.iter().any(|id| *id == r.source_id) && target_ids.contains(&r.target_id.as_str()))
            || (ids.iter().any(|id| *id == r.target_id)
                && target_ids.contains(&r.source_id.as_str()))
    })
}

fn resolve_system_context(
    focal: &Element,
    model: &Model,
) -> (Vec<String>, Vec<(String, String, Option<String>, Option<String>)>) {
    let mut visible = vec![focal.id.clone()];
    let focal_ids: Vec<&str> = std::iter::once(focal.id.as_str())
        .chain(all_child_ids(focal))
        .collect();

    for elem in &model.elements {
        if elem.id == focal.id {
            continue;
        }
        match elem.kind {
            ElementKind::Person => {
                if is_connected_to_set(&elem.id, elem, &focal_ids, &model.relationships) {
                    visible.push(elem.id.clone());
                }
            }
            ElementKind::SoftwareSystem => {
                if is_connected_to_set(&elem.id, elem, &focal_ids, &model.relationships) {
                    visible.push(elem.id.clone());
                }
            }
            _ => {}
        }
    }

    let edges = resolve_edges(&visible, &focal_ids, model);
    (visible, edges)
}

fn resolve_container(
    focal: &Element,
    model: &Model,
) -> (
    Vec<String>,
    Vec<(String, String, Option<String>, Option<String>)>,
    Option<(String, String)>,
) {
    let mut visible: Vec<String> = focal.children.iter().map(|c| c.id.clone()).collect();

    let container_ids: Vec<&str> = focal
        .children
        .iter()
        .flat_map(|c| {
            std::iter::once(c.id.as_str()).chain(all_child_ids(c))
        })
        .collect();

    for elem in &model.elements {
        if elem.id == focal.id {
            continue;
        }
        match elem.kind {
            ElementKind::Person => {
                if is_connected_to_set(&elem.id, elem, &container_ids, &model.relationships) {
                    visible.push(elem.id.clone());
                }
            }
            ElementKind::SoftwareSystem => {
                if is_connected_to_set(&elem.id, elem, &container_ids, &model.relationships) {
                    visible.push(elem.id.clone());
                }
            }
            _ => {}
        }
    }

    let edges = resolve_edges(&visible, &container_ids, model);
    let boundary = Some((focal.id.clone(), focal.name.clone()));
    (visible, edges, boundary)
}

fn resolve_component(
    focal_container: &Element,
    focal_system: &Element,
    model: &Model,
) -> (
    Vec<String>,
    Vec<(String, String, Option<String>, Option<String>)>,
    Option<(String, String)>,
) {
    let mut visible: Vec<String> = focal_container.children.iter().map(|c| c.id.clone()).collect();

    let component_ids: Vec<&str> = focal_container
        .children
        .iter()
        .map(|c| c.id.as_str())
        .collect();

    // Other containers in the same system
    for container in &focal_system.children {
        if container.id == focal_container.id {
            continue;
        }
        if is_connected_to_set(&container.id, container, &component_ids, &model.relationships) {
            visible.push(container.id.clone());
        }
    }

    // People and external systems
    for elem in &model.elements {
        if elem.id == focal_system.id {
            continue;
        }
        match elem.kind {
            ElementKind::Person | ElementKind::SoftwareSystem => {
                if is_connected_to_set(&elem.id, elem, &component_ids, &model.relationships) {
                    visible.push(elem.id.clone());
                }
            }
            _ => {}
        }
    }

    let edges = resolve_edges(&visible, &component_ids, model);
    let boundary = Some((focal_container.id.clone(), focal_container.name.clone()));
    (visible, edges, boundary)
}

fn resolve_edges(
    visible: &[String],
    focal_child_ids: &[&str],
    model: &Model,
) -> Vec<(String, String, Option<String>, Option<String>)> {
    let mut edges = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for rel in &model.relationships {
        let src = propagate_to_visible(&rel.source_id, visible, focal_child_ids, &model.elements);
        let tgt = propagate_to_visible(&rel.target_id, visible, focal_child_ids, &model.elements);

        if let (Some(s), Some(t)) = (src, tgt) {
            if s != t {
                let key = (s.clone(), t.clone());
                if seen.insert(key) {
                    edges.push((s, t, rel.description.clone(), rel.technology.clone()));
                }
            }
        }
    }
    edges
}

fn propagate_to_visible(
    id: &str,
    visible: &[String],
    focal_child_ids: &[&str],
    elements: &[Element],
) -> Option<String> {
    if visible.contains(&id.to_string()) {
        return Some(id.to_string());
    }

    // Walk up the hierarchy to find a visible ancestor
    if let Some(parent) = find_parent_of(id, elements) {
        if visible.contains(&parent) {
            return Some(parent);
        }
        return propagate_to_visible(&parent, visible, focal_child_ids, elements);
    }
    None
}

fn find_parent_of(child_id: &str, elements: &[Element]) -> Option<String> {
    for elem in elements {
        for child in &elem.children {
            if child.id == child_id {
                return Some(elem.id.clone());
            }
            if let Some(parent) = find_parent_of(child_id, &elem.children) {
                return Some(parent);
            }
        }
    }
    None
}

fn parse_hex_color(hex: &str) -> Option<[u8; 3]> {
    let hex = hex.trim_start_matches('#');
    if hex.len() < 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some([r, g, b])
}

fn map_shape(shape_name: &str) -> NodeShape {
    match shape_name.to_lowercase().as_str() {
        "person" => NodeShape::Rounded,
        "cylinder" => NodeShape::Rect,
        "hexagon" => NodeShape::Diamond,
        "roundedbox" => NodeShape::Rounded,
        "webbrowser" => NodeShape::Rect,
        "circle" => NodeShape::Circle,
        "diamond" => NodeShape::Diamond,
        _ => NodeShape::Rounded,
    }
}

fn direction_from_auto_layout(al: &Option<AutoLayout>) -> Direction {
    match al.as_ref().and_then(|a| a.direction.as_deref()) {
        Some("lr") => Direction::LR,
        Some("rl") => Direction::RL,
        Some("bt") => Direction::BT,
        _ => Direction::TB,
    }
}

fn build_element_label(elem: &Element) -> String {
    let mut parts = vec![elem.name.clone()];
    if let Some(tech) = &elem.technology {
        parts.push(format!("[{}]", tech));
    }
    if let Some(desc) = &elem.description {
        if desc.len() <= 60 {
            parts.push(desc.clone());
        } else {
            parts.push(format!("{}...", &desc[..57]));
        }
    }
    parts.join("\n")
}

fn build_edge_label(desc: &Option<String>, tech: &Option<String>) -> Option<String> {
    match (desc, tech) {
        (Some(d), Some(t)) => Some(format!("{}\n[{}]", d, t)),
        (Some(d), None) => Some(d.clone()),
        (None, Some(t)) => Some(format!("[{}]", t)),
        (None, None) => None,
    }
}

fn resolve_styles_for_element(
    elem: &Element,
    styles: &StylesDef,
) -> (Option<[u8; 3]>, Option<[u8; 3]>, Option<[u8; 3]>, NodeShape) {
    // C4 defaults
    let (mut bg, mut text_color) = match elem.kind {
        ElementKind::Person => (Some([0x08, 0x42, 0x7b]), Some([0xff, 0xff, 0xff])),
        ElementKind::SoftwareSystem => (Some([0x11, 0x68, 0xbd]), Some([0xff, 0xff, 0xff])),
        ElementKind::Container => (Some([0x43, 0x8d, 0xd5]), Some([0xff, 0xff, 0xff])),
        ElementKind::Component => (Some([0x85, 0xbb, 0xf0]), Some([0x00, 0x00, 0x00])),
    };
    let mut stroke: Option<[u8; 3]> = None;
    let mut shape = NodeShape::Rounded;

    for tag in &elem.tags {
        for style in &styles.element_styles {
            if style.tag == *tag {
                if let Some(ref hex) = style.background {
                    if let Some(c) = parse_hex_color(hex) {
                        bg = Some(c);
                    }
                }
                if let Some(ref hex) = style.color {
                    if let Some(c) = parse_hex_color(hex) {
                        text_color = Some(c);
                    }
                }
                if let Some(ref s) = style.shape {
                    shape = map_shape(s);
                }
            }
        }
    }

    // Derive stroke from background if not set
    if stroke.is_none() {
        if let Some(bg_c) = bg {
            stroke = Some([
                (bg_c[0] as f32 * 0.8) as u8,
                (bg_c[1] as f32 * 0.8) as u8,
                (bg_c[2] as f32 * 0.8) as u8,
            ]);
        }
    }

    (bg, stroke, text_color, shape)
}

pub fn to_diagram_graph(
    workspace: &Workspace,
    view_index: usize,
) -> Result<DiagramGraph, Box<dyn std::error::Error>> {
    if workspace.views.view_defs.is_empty() {
        return Err("no views defined in workspace".into());
    }

    let view_idx = view_index.min(workspace.views.view_defs.len() - 1);
    let view = &workspace.views.view_defs[view_idx];

    let id_map = build_id_map(&workspace.model.elements);

    let direction = direction_from_auto_layout(&view.auto_layout);

    let (visible_ids, edges, boundary) = match view.kind {
        ViewKind::SystemContext => {
            let focal_id = view
                .target_id
                .as_ref()
                .ok_or("systemContext view missing target")?;
            let focal = find_element(&workspace.model.elements, focal_id)
                .ok_or_else(|| format!("focal element '{}' not found", focal_id))?;
            let (vis, edges) = resolve_system_context(focal, &workspace.model);
            (vis, edges, None)
        }
        ViewKind::Container => {
            let focal_id = view
                .target_id
                .as_ref()
                .ok_or("container view missing target")?;
            let focal = find_element(&workspace.model.elements, focal_id)
                .ok_or_else(|| format!("focal element '{}' not found", focal_id))?;
            let (vis, edges, boundary) = resolve_container(focal, &workspace.model);
            (vis, edges, boundary)
        }
        ViewKind::Component => {
            let container_id = view
                .target_id
                .as_ref()
                .ok_or("component view missing target")?;
            let container = find_element(&workspace.model.elements, container_id)
                .ok_or_else(|| format!("focal container '{}' not found", container_id))?;
            let system = find_parent_system(&workspace.model.elements, container_id)
                .ok_or_else(|| format!("parent system for '{}' not found", container_id))?;
            let (vis, edges, boundary) =
                resolve_component(container, system, &workspace.model);
            (vis, edges, boundary)
        }
        ViewKind::SystemLandscape => {
            let mut visible = Vec::new();
            for elem in &workspace.model.elements {
                match elem.kind {
                    ElementKind::Person | ElementKind::SoftwareSystem => {
                        visible.push(elem.id.clone());
                    }
                    _ => {}
                }
            }
            let all_ids: Vec<&str> = visible.iter().map(|s| s.as_str()).collect();
            let edges = resolve_edges(&visible, &all_ids, &workspace.model);
            (visible, edges, None)
        }
    };

    let mut nodes = HashMap::new();
    let mut styles_map = HashMap::new();

    for id in &visible_ids {
        if let Some(elem) = id_map.get(id.as_str()) {
            let label = build_element_label(elem);
            let (bg, stroke, text_color, shape) =
                resolve_styles_for_element(elem, &workspace.views.styles);

            let node_def = NodeDef {
                label,
                shape,
                classes: elem.tags.clone(),
                class_fields: Vec::new(),
                class_methods: Vec::new(),
                sql_columns: Vec::new(),
                near: None,
                tooltip: None,
                link: None,
            };
            nodes.insert(id.clone(), node_def);

            let mut sp = StyleProps::default();
            sp.fill = bg;
            sp.stroke = stroke;
            sp.color = text_color;
            sp.stroke_width = Some(2.0);
            styles_map.insert(id.clone(), sp);
        }
    }

    let diagram_edges: Vec<EdgeDef> = edges
        .iter()
        .map(|(src, tgt, desc, tech)| EdgeDef {
            from: src.clone(),
            to: tgt.clone(),
            edge_type: EdgeType::DottedArrow,
            label: build_edge_label(desc, tech),
            src_arrowhead: None,
            dst_arrowhead: None,
            style: StyleProps::default(),
        })
        .collect();

    let subgraphs = if let Some((boundary_id, boundary_title)) = boundary {
        let child_ids: Vec<String> = visible_ids
            .iter()
            .filter(|id| {
                if id_map.contains_key(id.as_str()) {
                    if let Some(parent) = find_parent_of(id, &workspace.model.elements) {
                        return parent == boundary_id;
                    }
                }
                false
            })
            .cloned()
            .collect();

        if child_ids.is_empty() {
            Vec::new()
        } else {
            vec![SubgraphDef {
                title: boundary_title,
                node_ids: child_ids,
                grid_rows: None,
                grid_columns: None,
                grid_gap: None,
            }]
        }
    } else {
        Vec::new()
    };

    let layer_spacing = view.auto_layout.as_ref()
        .and_then(|al| al.rank_sep)
        .map(|v| v as f32);
    let node_spacing = view.auto_layout.as_ref()
        .and_then(|al| al.node_sep)
        .map(|v| v as f32);

    Ok(DiagramGraph {
        direction,
        nodes,
        edges: diagram_edges,
        subgraphs,
        styles: styles_map,
        class_defs: HashMap::new(),
        layer_spacing,
        node_spacing,
    })
}

pub fn find_view_for_element(workspace: &Workspace, element_id: &str, current_view_index: usize) -> Option<usize> {
    let current_depth = workspace.views.view_defs.get(current_view_index)
        .map(|v| view_depth(&v.kind))
        .unwrap_or(0);

    workspace
        .views
        .view_defs
        .iter()
        .enumerate()
        .position(|(i, vd)| {
            i != current_view_index
                && vd.target_id.as_deref() == Some(element_id)
                && view_depth(&vd.kind) > current_depth
        })
}

fn view_depth(kind: &ViewKind) -> u8 {
    match kind {
        ViewKind::SystemLandscape => 0,
        ViewKind::SystemContext => 1,
        ViewKind::Container => 2,
        ViewKind::Component => 3,
    }
}

pub fn view_label(workspace: &Workspace, view_index: usize) -> String {
    let view = match workspace.views.view_defs.get(view_index) {
        Some(v) => v,
        None => return format!("View {}", view_index),
    };

    let kind_label = match view.kind {
        ViewKind::SystemContext => "System Context",
        ViewKind::Container => "Containers",
        ViewKind::Component => "Components",
        ViewKind::SystemLandscape => "System Landscape",
    };

    if let Some(target_id) = &view.target_id {
        if let Some(elem) = find_element(&workspace.model.elements, target_id) {
            return format!("{}: {}", kind_label, elem.name);
        }
    }

    kind_label.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_line() {
        let tokens = tokenize_line(r#"trainer = softwareSystem "Kubeflow Trainer" "Description" {"#);
        assert_eq!(tokens[0], "trainer");
        assert_eq!(tokens[1], "=");
        assert_eq!(tokens[2], "softwareSystem");
        assert_eq!(tokens[3], "\"Kubeflow Trainer\"");
        assert_eq!(tokens[4], "\"Description\"");
        assert_eq!(tokens[5], "{");
    }

    #[test]
    fn test_tokenize_relationship() {
        let tokens = tokenize_line(r#"user -> system "Uses" "HTTPS/443""#);
        assert_eq!(tokens[0], "user");
        assert_eq!(tokens[1], "->");
        assert_eq!(tokens[2], "system");
        assert_eq!(tokens[3], "\"Uses\"");
        assert_eq!(tokens[4], "\"HTTPS/443\"");
    }

    #[test]
    fn test_strip_comments() {
        let input = r#"
workspace { // comment
    model {
        # another comment
        user = person "User" "A user"
    }
}
"#;
        let cleaned = strip_comments(input);
        assert!(!cleaned.contains("// comment"));
        assert!(!cleaned.contains("# another"));
        assert!(cleaned.contains("person"));
    }

    #[test]
    fn test_parse_minimal_workspace() {
        let input = r#"
workspace {
    model {
        user = person "Alice" "A user"
        sys = softwareSystem "MySystem" "A system"
        user -> sys "Uses"
    }
    views {
        systemContext sys "Context" {
            include *
            autoLayout
        }
        styles {
            element "Person" {
                background #08427b
                color #ffffff
            }
        }
    }
}
"#;
        let ws = parse_workspace_v2(input).unwrap();
        assert_eq!(ws.model.elements.len(), 2);
        assert_eq!(ws.model.relationships.len(), 1);
        assert_eq!(ws.views.view_defs.len(), 1);
        assert_eq!(ws.views.styles.element_styles.len(), 1);
    }

    #[test]
    fn test_parse_nested_elements() {
        let input = r#"
workspace {
    model {
        sys = softwareSystem "System" "Desc" {
            web = container "Web App" "Frontend" "React"
            api = container "API" "Backend" "Go" {
                auth = component "Auth" "Authentication" "JWT"
            }
            web -> api "Calls" "HTTPS"
        }
    }
    views {
        container sys "Containers" {
            include *
            autoLayout
        }
        styles {
        }
    }
}
"#;
        let ws = parse_workspace_v2(input).unwrap();
        let sys = &ws.model.elements[0];
        assert_eq!(sys.children.len(), 2);
        assert_eq!(sys.children[1].children.len(), 1);
        assert_eq!(ws.model.relationships.len(), 1);
        assert_eq!(ws.model.relationships[0].source_id, "web");
    }

    #[test]
    fn test_to_diagram_graph_system_context() {
        let input = r#"
workspace {
    model {
        user = person "Alice" "A user"
        sys = softwareSystem "MySystem" "A system"
        ext = softwareSystem "External" "Another" "External"
        user -> sys "Uses" "HTTPS"
        sys -> ext "Calls" "REST"
    }
    views {
        systemContext sys "Context" {
            include *
            autoLayout
        }
        styles {
            element "Person" {
                background #08427b
                color #ffffff
            }
            element "External" {
                background #999999
            }
        }
    }
}
"#;
        let ws = parse_workspace_v2(input).unwrap();
        let graph = to_diagram_graph(&ws, 0).unwrap();
        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.edges.len(), 2);
        assert!(graph.nodes.contains_key("user"));
        assert!(graph.nodes.contains_key("sys"));
        assert!(graph.nodes.contains_key("ext"));
    }

    #[test]
    fn test_to_diagram_graph_container_view() {
        let input = r#"
workspace {
    model {
        user = person "Alice" "A user"
        sys = softwareSystem "MySystem" "A system" {
            web = container "Web" "Frontend" "React"
            api = container "API" "Backend" "Go"
            web -> api "Calls" "HTTPS"
        }
        user -> web "Uses" "HTTPS"
    }
    views {
        container sys "Containers" {
            include *
            autoLayout
        }
        styles {
        }
    }
}
"#;
        let ws = parse_workspace_v2(input).unwrap();
        let graph = to_diagram_graph(&ws, 0).unwrap();
        // Should show: web, api, user (connected to web)
        assert!(graph.nodes.contains_key("web"));
        assert!(graph.nodes.contains_key("api"));
        assert!(graph.nodes.contains_key("user"));
        assert!(!graph.nodes.contains_key("sys")); // focal system is boundary, not a node
    }

    #[test]
    fn test_tags_and_styles() {
        let input = r#"
workspace {
    model {
        ext = softwareSystem "Prometheus" "Metrics" "External"
    }
    views {
        systemContext ext "Context" {
            include *
            autoLayout
        }
        styles {
            element "Software System" {
                background #1168bd
                color #ffffff
            }
            element "External" {
                background #999999
                color #ffffff
            }
        }
    }
}
"#;
        let ws = parse_workspace_v2(input).unwrap();
        let elem = &ws.model.elements[0];
        assert!(elem.tags.contains(&"External".to_string()));
        assert!(elem.tags.contains(&"Software System".to_string()));

        let (bg, _, _, _) = resolve_styles_for_element(elem, &ws.views.styles);
        // "External" tag style should override "Software System"
        assert_eq!(bg, Some([0x99, 0x99, 0x99]));
    }

    #[test]
    fn test_direction_from_auto_layout() {
        let al = Some(AutoLayout {
            direction: Some("lr".to_string()),
            rank_sep: None,
            node_sep: None,
        });
        assert_eq!(direction_from_auto_layout(&al), Direction::LR);

        let al_none = None;
        assert_eq!(direction_from_auto_layout(&al_none), Direction::TB);
    }

    #[test]
    fn test_block_comment_in_string() {
        let input = r#"
workspace {
    model {
        sys = softwareSystem "System" "Exposes /v1/* API" {
            api = container "API" "REST /api/*" "Go"
        }
    }
    views {
        container sys "Containers" {
            include *
            autoLayout
        }
        styles {
        }
    }
}
"#;
        let ws = parse_workspace_v2(input).unwrap();
        assert_eq!(ws.model.elements.len(), 1);
        assert_eq!(ws.model.elements[0].children.len(), 1);
        assert_eq!(ws.views.view_defs.len(), 1);
    }
}
