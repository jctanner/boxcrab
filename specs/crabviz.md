# Crabviz Rendering Specification

A language-agnostic specification of how Crabviz transforms LSP-sourced code
structure into interactive call graph visualizations. Written for implementors
building a similar system in Rust or other languages.

---

## Table of Contents

1. [Overview](#1-overview)
2. [High-Level Pipeline](#2-high-level-pipeline)
3. [Data Model](#3-data-model)
4. [Code Analysis (LSP Integration)](#4-code-analysis-lsp-integration)
5. [Language-Specific Filtering](#5-language-specific-filtering)
6. [Graph Generation](#6-graph-generation)
7. [Graph-to-DOT Conversion](#7-graph-to-dot-conversion)
8. [Node Rendering](#8-node-rendering)
9. [Edge Rendering](#9-edge-rendering)
10. [Cluster (Directory) Rendering](#10-cluster-directory-rendering)
11. [Layout](#11-layout)
12. [SVG Post-Processing](#12-svg-post-processing)
13. [Color System and Theming](#13-color-system-and-theming)
14. [Interactive Features](#14-interactive-features)
15. [Export Formats](#15-export-formats)
16. [Coordinate System and Viewport](#16-coordinate-system-and-viewport)

---

## 1. Overview

Crabviz is an LSP-based call graph generator that produces interactive,
zoomable visualizations of code relationships. It uses the Language Server
Protocol to discover symbols and call hierarchies from source code in any
LSP-supported language, then renders the result as an interactive SVG.

**Architecture:**
- **Core** — Rust library compiled to WebAssembly. Receives LSP data,
  builds graph model, filters symbols by language.
- **Editor Integration** — TypeScript VS Code extension. Orchestrates LSP
  requests and hosts the webview.
- **Webview UI** — Solid.js + TypeScript. Converts graph to Graphviz DOT,
  renders SVG via WASM Graphviz, adds interactivity.

**Supported languages:** Go, Rust, JavaScript/TypeScript, plus any
language with an LSP server (via generic fallback).

The fundamental rendering flow:

```
LSP Requests → Graph Model → Graphviz DOT → SVG
  → Post-Process → Interactive Visualization
```

---

## 2. High-Level Pipeline

### 2.1 Pipeline Stages

```
1. Gather    — Query LSP for document symbols, call hierarchy,
               and interface implementations
2. Build     — Accumulate files, symbols, and relations into a Graph
3. Filter    — Apply language-specific symbol filtering
4. Convert   — Transform Graph into Graphviz VizGraph structure
5. Layout    — Run Graphviz DOT algorithm to produce positioned SVG
6. Post-proc — Rewrite SVG: inject CSS classes, replace shapes,
               add interaction targets
7. Interact  — Mount pan/zoom, wire click handlers, enable
               selection/highlighting
```

### 2.2 Data Flow

```
LSP Server
  │
  ├─ documentSymbol    → Vec<DocumentSymbol> per file
  ├─ callHierarchy     → incoming/outgoing calls per function
  └─ implementation    → implementing types per interface
  │
  ▼
GraphGenerator (Rust/WASM)
  │
  ├─ add_file(path, symbols)
  ├─ add_incoming_calls(path, pos, calls)
  ├─ add_outgoing_calls(path, pos, calls)
  ├─ add_interface_implementations(path, pos, locations)
  └─ gen_graph() → Graph { files, relations }
  │
  ▼
convert(graph, root, collapse) → VizGraph
  │
  ▼
@viz-js/viz (Graphviz WASM)
  │  renderSVGElement(vizGraph)
  ▼
Raw SVG
  │
  ▼
renderSVG(vizGraph, focus) → Post-processed SVG DOM
  │
  ▼
CallGraph class → Interactive SVG with pan/zoom/selection
```

---

## 3. Data Model

### 3.1 Graph

The top-level structure produced by the Rust core:

```
Graph:
    files:      Vec<File>       // All source files in the graph
    relations:  Vec<Relation>   // All edges between symbols
```

### 3.2 File

```
File:
    id:         u32             // Sequential unique identifier
    path:       String          // Relative file path from project root
    symbols:    Vec<Symbol>     // Top-level symbols (recursive)
```

### 3.3 Symbol

Hierarchical — symbols can contain child symbols (e.g., methods in a class).

```
Symbol:
    name:       String          // Symbol name as reported by LSP
    kind:       SymbolKind      // LSP symbol kind (see 3.5)
    range:      Range           // Full source range of the symbol
    children:   Vec<Symbol>     // Nested symbols (methods, fields, etc.)
```

### 3.4 Relation

A directed edge between two code locations.

```
Relation:
    from:       GlobalPosition  // Source location
    to:         GlobalPosition  // Target location
    kind:       RelationKind    // Type of relationship

GlobalPosition:
    file_id:    u32             // Which file (matches File.id)
    line:       u32             // Zero-based line number
    character:  u32             // Zero-based character offset
```

### 3.5 Enumerations

```
RelationKind:
    Call    = 0     // Function/method invocation
    Impl    = 1     // Type implements interface/trait
    Inherit = 2     // Type inheritance

SymbolKind (from LSP spec):
    File = 1, Module = 2, Namespace = 3, Package = 4,
    Class = 5, Method = 6, Property = 7, Field = 8,
    Constructor = 9, Enum = 10, Interface = 11,
    Function = 12, Variable = 13, Constant = 14,
    String = 15, Number = 16, Boolean = 17, Array = 18,
    Object = 19, Key = 20, Null = 21, EnumMember = 22,
    Struct = 23, Event = 24, Operator = 25,
    TypeParameter = 26
```

---

## 4. Code Analysis (LSP Integration)

### 4.1 LSP Requests

Three LSP request types are used to gather graph data:

**1. Document Symbols** (`textDocument/documentSymbol`)

Returns the full symbol tree for a file. Each symbol has a name, kind,
range, and recursive children. This provides the node structure.

```
Request:  textDocument/documentSymbol { uri: file_uri }
Response: DocumentSymbol[]
```

Retry logic: up to 5 attempts with 600ms delays to wait for LSP server
startup.

**2. Call Hierarchy** (`callHierarchy/prepare` + `incomingCalls`/`outgoingCalls`)

For each function/method symbol, prepare a call hierarchy item, then
query for incoming callers and outgoing callees.

```
Step 1: callHierarchy/prepare { uri, position } → CallHierarchyItem[]
Step 2: callHierarchy/incomingCalls { item } → CallHierarchyIncomingCall[]
Step 3: callHierarchy/outgoingCalls { item } → CallHierarchyOutgoingCall[]
```

For function-level graphs, the call hierarchy is traversed recursively
with cycle detection via a visited set.

**3. Implementations** (`textDocument/implementation`)

For each interface/trait symbol, query for implementing types.

```
Request:  textDocument/implementation { uri, position }
Response: Location[] | LocationLink[]
```

### 4.2 Symbol-to-Position Mapping

Symbols are globally identified by their `GlobalPosition`:
`(file_id, selection_range.start.line, selection_range.start.character)`.
This position serves as the key for linking call hierarchy results back
to symbol nodes.

---

## 5. Language-Specific Filtering

### 5.1 Filter Interface

```
trait Language:
    should_filter_out_file(path: &str) -> bool
    filter_symbol(symbol: &DocumentSymbol, parent: Option<&DocumentSymbol>) -> bool
```

When `filter_symbol` returns true, the symbol is excluded from the graph.

### 5.2 Default Filtering

The default language handler excludes:
- `Constant`, `Variable`, `EnumMember` — low-signal symbols
- `Field` and `Property` when the parent is NOT an interface

This keeps the graph focused on callable and structural symbols.

### 5.3 Language-Specific Handlers

| Language | File Filter | Symbol Filter |
|----------|------------|---------------|
| Default | None | Constants, Variables, EnumMembers, non-interface Fields |
| Rust | None | Above + test modules, constants |
| Go | None | Custom Go-specific filtering |
| JS/TS | None | Specialized handling for JS/TS patterns |

---

## 6. Graph Generation

### 6.1 Generator State

```
GraphGenerator:
    lang:             Box<dyn Language>
    file_id_map:      HashMap<String, u32>     // path → file ID
    files:            HashMap<String, Vec<DocumentSymbol>>
    incoming_calls:   HashMap<GlobalPosition, Vec<CallHierarchyIncomingCall>>
    outgoing_calls:   HashMap<GlobalPosition, Vec<CallHierarchyOutgoingCall>>
    interfaces:       HashMap<GlobalPosition, Vec<GlobalPosition>>
    filter:           bool
```

### 6.2 Graph Building Process

```
gen_graph():
    1. Collect all files and their symbols
       - Apply language-specific symbol filtering
       - Assign sequential file IDs
    
    2. Process call relationships
       - For each incoming/outgoing call entry, create a Relation
       - Filter relations where either endpoint doesn't match a known symbol
    
    3. Handle missing nested symbols
       - Some LSP servers don't report nested function definitions
       - If a call target position falls within a symbol's range but
         doesn't match any known child, insert a synthetic symbol
    
    4. Process interface implementations
       - Create Impl relations from interface positions to implementor positions
    
    5. Deduplicate relations
       - Use HashSet to remove duplicate (from, to, kind) triples
    
    6. Return Graph { files, relations }
```

### 6.3 Graph Modes

Two rendering modes determine how the graph is presented:

**Expanded Mode (collapse = false):**
- Each file node shows individual symbols as rows
- Edges connect specific symbols (via ports)
- Full detail of which function calls which function

**Collapsed Mode (collapse = true):**
- Each file node shows only the filename
- Edges are aggregated to file-to-file level
- One edge per unique (source_file, target_file) pair
- Impl edges remain distinguished from Call edges

---

## 7. Graph-to-DOT Conversion

### 7.1 Graphviz Graph Attributes

```
digraph {
    rankdir = "LR"          // Left-to-right layout
    ranksep = 2.0            // Spacing between ranks
    fontsize = 16
    fontname = "Arial"

    node [
        fontsize = 16
        shape = plaintext     // HTML label mode (no default shape)
        style = filled
    ]

    edge [
        arrowsize = 1.5
        label = " "           // Single space (no visible label)
    ]
}
```

### 7.2 Conversion Algorithm

```
convert(graph, root, collapse):
    nodes = []
    edges = []
    subgraphs = []
    
    // Build directory tree for subgraph clustering
    dir_tree = build_directory_hierarchy(graph.files, root)
    
    // Convert each file to a Graphviz node
    for file in graph.files:
        node = file2node(file, collapse)
        nodes.push(node)
    
    // Organize nodes into directory-based subgraphs
    subgraphs = dir_tree_to_clusters(dir_tree, nodes)
    
    // Convert relations to edges
    edges = collectEdges(graph.relations, graph.files, collapse)
    
    return VizGraph { nodes, edges, subgraphs, attributes }
```

### 7.3 File-to-Node Conversion

Each file becomes a node with an HTML table label:

**Expanded mode:**
```
Node:
    id:    file.id (as string)
    label: HTML table
        Row 0: File name (clickable, links to file path)
        Row 1..N: Symbol cells (one per top-level symbol)
            Each cell: icon + name, with PORT for edge connections
            Nested symbols: sub-table within the cell
```

**Collapsed mode:**
```
Node:
    id:    file.id (as string)
    label: HTML table
        Row 0: File name only
```

### 7.4 Symbol Cell Structure

Each symbol in expanded mode renders as an HTML table cell:

```
<TD PORT="line_character" ID="fileId:line_character" HREF="symbolKindNumber">
    [ICON] symbol_name
</TD>
```

- **PORT**: `{line}_{character}` — used as edge endpoint
- **ID**: `{fileId}:{line}_{character}` — unique identifier
- **HREF**: SymbolKind number — parsed during post-processing to assign
  CSS classes

Symbol icons:
| Icon | Kind |
|------|------|
| C | Class |
| S | Struct |
| E | Enum |
| T | TypeParameter |
| f | Field |
| p | Property |

### 7.5 Edge Generation

**Expanded mode:**

```
for each relation in graph.relations:
    edge = {
        tail:     relation.from.file_id
        head:     relation.to.file_id
        tailport: "{from.line}_{from.character}"
        headport: "{to.line}_{to.character}"
        id:       "{from.file_id}:{from.line}_{from.char}-{to.file_id}:{to.line}_{to.char}"
        class:    "impl" if relation.kind == Impl else ""
    }
```

**Collapsed mode:**

```
seen = Map<(from_file, to_file, is_impl), bool>

for each relation in graph.relations:
    key = (relation.from.file_id, relation.to.file_id, relation.kind == Impl)
    if seen.has(key): continue
    seen.set(key, true)
    
    edge = {
        tail: relation.from.file_id
        head: relation.to.file_id
        class: "impl" if relation.kind == Impl else ""
    }
```

---

## 8. Node Rendering

### 8.1 File Node Anatomy

Expanded file node:

```
╭──────────────────────────╮
│      src/main.rs         │  ← file path header (clickable)
├──────────────────────────┤
│ ● main()                 │  ← function symbol (port)
│ ● parse_args()           │  ← function symbol (port)
│ S Config                 │  ← struct symbol
│   ● new()                │  ← nested method (child port)
│   ● validate()           │  ← nested method (child port)
╰──────────────────────────╯
```

Collapsed file node:

```
╭──────────────────────────╮
│      src/main.rs         │
╰──────────────────────────╯
```

### 8.2 Node Shape

Nodes use `shape=plaintext` with HTML labels. After Graphviz renders the
SVG, the polygon shapes generated by Graphviz are replaced with `<rect>`
elements:

```
replace polygon with rect:
    points = polygon.getAttribute("points")
    bounds = compute_bounding_box(points)
    rect.x = bounds.min_x
    rect.y = bounds.min_y
    rect.width = bounds.max_x - bounds.min_x
    rect.height = bounds.max_y - bounds.min_y
    rect.rx = 20    // rounded corners
```

### 8.3 Node Styling

```css
.node > rect {
    rx: 20px
    fill: var(--node-bg-color)       /* #f4f5f1 off-white */
    filter: url(#shadow)             /* drop shadow */
}

.node.selected > rect {
    stroke: var(--selected-color)    /* #4fe1f4 cyan */
    stroke-width: 3.2
}
```

### 8.4 Symbol Cell Styling

Each symbol cell gets a CSS class from its SymbolKind. Styling per type:

```css
.cell > rect {
    rx: 10px
    fill: var(--bg-color)            /* varies by symbol kind */
    stroke: var(--border-color)      /* varies by symbol kind */
    stroke-width: 1.6
}

/* Container types get square corners */
.cell:where(.class, .struct, .enum) > rect {
    rx: 0
}

/* Interfaces get dashed borders */
.cell.interface > rect {
    stroke-dasharray: 7, 5
}

/* Highlight uses gradient stroke */
.cell.highlight > rect {
    stroke: url(#highlightGradient)
    stroke-width: 3.2
}

/* Selected cells get drop shadow */
.cell.selected > rect {
    filter: drop-shadow(3px 3px 6px var(--border-color))
    stroke-width: 3.2
}
```

---

## 9. Edge Rendering

### 9.1 Edge Anatomy

Edges are directed arrows from a source symbol port to a target symbol
port. Graphviz computes the path geometry.

```
┌────────────┐                    ┌────────────┐
│ src/a.rs   │                    │ src/b.rs   │
├────────────┤                    ├────────────┤
│ ● foo()   ─┼───────────────────→│ ● bar()    │
│ ● baz()    │                    │ ● qux()    │
└────────────┘                    └────────────┘
```

### 9.2 Edge Styles

| Relation Kind | Line Style | Arrowhead |
|--------------|------------|-----------|
| Call | Solid, 3px | Filled triangle (arrowsize 1.5) |
| Impl | Dashed (8, 3), 3px | Hollow triangle (no fill) |
| Inherit | Solid, 3px | Filled triangle |

```css
/* Default edge */
.edge > path:not(.hover-path) {
    stroke: var(--edge-color)        /* #548f9e slate blue */
    stroke-width: 3
}

.edge > polygon {
    stroke: var(--edge-color)
    fill: var(--edge-color)
}

/* Implementation edges */
.edge.impl > path {
    stroke-dasharray: 8, 3
}
.edge.impl > polygon {
    stroke-width: 2
    fill: none                       /* hollow arrowhead */
}
```

### 9.3 Edge Interaction Target

Each edge path is duplicated with an invisible wide hit area:

```
hover_path = clone(edge_path)
hover_path.class = "hover-path"
hover_path.stroke = transparent
hover_path.stroke-width = 15         // 15px invisible hit area
hover_path.stroke-dasharray = none   // always solid
```

### 9.4 Edge Data Attributes

Edges carry metadata for interaction:

```
edge.dataset.from = "fileId:line_character"  // source symbol
edge.dataset.to   = "fileId:line_character"  // target symbol
```

### 9.5 Edge Highlighting Colors

| State | Color | CSS Variable |
|-------|-------|-------------|
| Default | Slate blue (#548f9e) | `--edge-color` |
| Hover | Cyan (#4fe1f4) | `--selected-color` |
| Incoming | Sage green (#698b69) | `--edge-incoming-color` |
| Outgoing | Bright blue (#008acd) | `--edge-outgoing-color` |
| Bidirectional | Black | `--edge-incoming-outgoing-color` |

---

## 10. Cluster (Directory) Rendering

### 10.1 Directory Hierarchy

Files are grouped into Graphviz subgraph clusters by directory path:

```
subgraph cluster_0 {
    label = "src/"
    
    subgraph cluster_1 {
        label = "models/"
        node_1 [...]
        node_2 [...]
    }
    
    subgraph cluster_2 {
        label = "handlers/"
        node_3 [...]
    }
}
```

### 10.2 Cluster Naming

Graphviz requires clusters to have names prefixed with `cluster_`. A
sequential counter generates unique cluster names.

### 10.3 Cluster Styling

```css
.cluster polygon {
    stroke-width: 1.6
}

.cluster .cluster-label {
    fill: var(--cluster-label-bg-color)   /* #e8eaed light gray */
    rx: 18px                               /* rounded label box */
}

.cluster text {
    pointer-events: none                   /* text is not clickable */
}

.cluster.selected polygon {
    stroke: var(--selected-color)          /* cyan highlight */
    stroke-width: 3.2
}
```

---

## 11. Layout

### 11.1 Layout Engine

Crabviz uses Graphviz DOT layout via `@viz-js/viz` (Graphviz compiled to
WebAssembly). The DOT algorithm is a hierarchical (Sugiyama-style) layout
optimized for directed graphs.

### 11.2 Layout Parameters

```
rankdir:  LR        // Left-to-right (source files left, targets right)
ranksep:  2.0       // Inter-rank spacing (controls horizontal spread)
fontsize: 16        // Base font size
fontname: Arial     // Font family
```

### 11.3 Layout Process

```
1. Graphviz parses the VizGraph structure
2. Rank assignment: nodes placed in layers by dependency depth
3. Ordering: nodes within each rank ordered to minimize edge crossings
4. Coordinate assignment: x,y positions computed
5. Edge routing: spline paths computed between ports
6. SVG output: positioned SVG elements with computed coordinates
```

### 11.4 Port-Based Edge Routing

In expanded mode, edges connect to specific symbol ports within file
nodes. Graphviz routes edges from the port position on the source node
to the port position on the target node, avoiding overlaps.

Port names follow the format `line_character` matching the symbol's
source location, giving each symbol a unique connection point.

---

## 12. SVG Post-Processing

After Graphviz produces raw SVG, several transformations are applied.

### 12.1 Title Removal

Graphviz adds `<title>` elements to nodes and edges. These are removed
since crabviz uses custom tooltips.

### 12.2 Anchor Processing

Graphviz renders `<a>` elements from HREF attributes. Post-processing:

```
for each <a> element:
    href = a.getAttribute("xlink:href")
    parent_g = a.closest("g")
    
    if href is numeric:
        // Symbol cell — href is SymbolKind number
        parent_g.classList.add("cell")
        parent_g.classList.add(symbolKindToClass(href))
    else:
        // File title — href is file path
        parent_g.classList.add("title")
        parent_g.dataset.path = href
    
    // Unwrap: move children out of <a>, remove <a>
    unwrap(a)
```

Symbol kind to CSS class mapping:

| SymbolKind | CSS Class |
|-----------|-----------|
| Module (2) | `.module` |
| Class (5) | `.class` |
| Method (6) | `.method` |
| Constructor (9) | `.constructor` |
| Interface (11) | `.interface` |
| Function (12) | `.function` |
| Field (8) | `.field` |
| Property (7) | `.property` |
| Enum (10) | `.enum` |
| Struct (23) | `.struct` |

### 12.3 Polygon-to-Rectangle Conversion

Graphviz renders all node shapes as `<polygon>` elements. Post-processing
converts these to `<rect>` for easier CSS styling:

```
polygon2rect(polygon):
    points = parse_points(polygon.getAttribute("points"))
    min_x = min(p.x for p in points)
    min_y = min(p.y for p in points)
    max_x = max(p.x for p in points)
    max_y = max(p.y for p in points)
    
    rect = create_element("rect")
    rect.x = min_x
    rect.y = min_y
    rect.width = max_x - min_x
    rect.height = max_y - min_y
    // Copy fill, stroke from polygon
    return rect
```

### 12.4 Edge Data Attribute Injection

```
for each edge group:
    id = edge.id    // format: "fileId:line_char-fileId:line_char"
    parts = id.split("-")
    edge.dataset.from = parts[0]
    edge.dataset.to = parts[1]
```

### 12.5 SVG Definitions Injection

Insert into `<defs>`:

```xml
<filter id="shadow">
    <feDropShadow dx="0" dy="0" stdDeviation="4" flood-opacity="0.5" />
</filter>

<linearGradient id="highlightGradient">
    <stop offset="0%" stop-color="var(--edge-incoming-color)" />
    <stop offset="100%" stop-color="var(--edge-outgoing-color)" />
</linearGradient>
```

### 12.6 Faded Layer

A `<g id="faded-group">` element is inserted into the SVG for the
fade-out interaction pattern. During selection, unrelated elements are
moved into this group.

---

## 13. Color System and Theming

### 13.1 Color Palette

All colors are defined as CSS custom properties:

**Background and Selection:**

| Variable | Value | Usage |
|----------|-------|-------|
| `--background-color` | `#f5fffa` | SVG background (mint white) |
| `--selected-color` | `#4fe1f4` | Selection highlight (cyan) |

**Edge Colors:**

| Variable | Value | Usage |
|----------|-------|-------|
| `--edge-color` | `#548f9e` | Default edge (slate blue) |
| `--edge-incoming-color` | `#698b69` | Incoming call (sage green) |
| `--edge-outgoing-color` | `#008acd` | Outgoing call (bright blue) |
| `--edge-incoming-outgoing-color` | `black` | Bidirectional |

**Node Colors:**

| Variable | Value | Usage |
|----------|-------|-------|
| `--node-bg-color` | `#f4f5f1` | File node background (off-white) |
| `--cluster-label-bg-color` | `#e8eaed` | Directory label (light gray) |

**Symbol Type Colors (background / border):**

| Symbol Kind | Background | Border |
|------------|-----------|--------|
| Function | `#bafbd0` (mint green) | `#4ac26b` |
| Method | `#fff8c5` (pale yellow) | `#d4a72c` |
| Constructor | `#ffdab9` (peach) | `#a66e3c` |
| Class | `#ddf4ff` (light blue) | `#54aeff` |
| Struct | `#ddf4ff` (light blue) | `#54aeff` |
| Enum | `#ddf4ff` (light blue) | `#54aeff` |
| Interface | `#fff8dc` (cornsilk) | `#a69348` |
| Module | `#ffebcd` (blanched almond) | `#a67e43` |

**Icon Colors:**

| Variable | Value | Usage |
|----------|-------|-------|
| `--type-icon-color` | `#8969da` (purple) | Type symbol icons (C, S, E, T) |
| `--property-icon-color` | `#5f9348` (green) | Property/field icons (f, p) |

### 13.2 Opacity for Fading

When elements are moved to the faded layer during selection:

```css
#faded-group > :where(.node, .cluster) {
    opacity: 0.2       /* 20% visible */
}

#faded-group > .edge {
    opacity: 0.05       /* 5% visible — nearly invisible */
}
```

---

## 14. Interactive Features

### 14.1 Pan and Zoom

Uses the `panzoom` library on the root SVG `<g>` element:

```
setup:
    target = svg.querySelector("#graph0")
    pz = createPanZoom(target, { smoothScroll: false, autocenter: true })
    save initial transform for reset

zoom_in:   pz.smoothZoom(current_scale * 1.5)
zoom_out:  pz.smoothZoom(current_scale / 1.5)
reset:     pz.moveTo(initial.x, initial.y); pz.zoomTo(initial.scale)
center_on: pz.centerOn(element)
```

### 14.2 Click Detection

Distinguish clicks from drags using a 6px movement threshold:

```
on mousedown:
    record (pageX, pageY)

on mouseup:
    dx = abs(pageX - start.pageX)
    dy = abs(pageY - start.pageY)
    if dx > 6 or dy > 6:
        return   // was a drag, not a click
    
    // Walk up DOM from event target to find interactive element
    for elem in ancestors(event.target):
        if elem.classList.contains("node"):   → onSelectNode(elem)
        if elem.classList.contains("cell"):   → onSelectCell(elem)
        if elem.classList.contains("edge"):   → onSelectEdge(elem)
        if elem.classList.contains("cluster-label"): → onSelectCluster(elem)
    
    // Click on background → clear selection
    onSelectElem(null)
```

### 14.3 Node Selection

When a file node is clicked:

```
onSelectNode(node):
    1. Add "selected" class to node rect
    2. Find all edges where from or to matches this file
    3. Add "incoming" class to edges pointing to this node
    4. Add "outgoing" class to edges pointing from this node
    5. Move all other nodes and edges to #faded-group
    6. Center view on selected node
```

### 14.4 Cell (Symbol) Selection

When a symbol cell is clicked:

**Normal mode:**

```
onSelectCell(cell):
    1. Add "selected" class to cell
    2. Collect cell ID and all children cell IDs
    3. Find edges where from or to matches any collected ID
    4. Classify edges as incoming/outgoing
    5. Move unrelated elements to #faded-group
    6. Center view on cell
```

**Focus mode** (when graph was generated from a specific function):

```
onSelectCellInFocusMode(cellId):
    1. BFS from cellId through incoming edge map
       → collect all ancestors in the call chain
    2. BFS from cellId through outgoing edge map
       → collect all descendants in the call chain
    3. Mark edges as incoming, outgoing, or both
    4. Mark bidirectional edges specially
    5. Highlight all nodes containing highlighted cells
    6. Fade everything else
```

### 14.5 Edge Selection

```
onSelectEdge(edge):
    1. Move ALL edges to #faded-group
    2. Move selected edge back to top layer
    3. Add "selected" class
    4. Fade all nodes
```

### 14.6 Cluster Selection

```
onSelectCluster(clusterLabel):
    1. Find all nodes within the cluster's bounding box
    2. Find edges between cluster members and external nodes
    3. Classify edges as incoming/outgoing relative to the cluster
    4. Fade everything outside the cluster
```

### 14.7 Selection Reset

Clicking on the SVG background clears all selection:

```
resetSelection():
    1. Remove all "selected", "incoming", "outgoing" classes
    2. Move all elements out of #faded-group back to their original groups
    3. Clear selectedElem reference
```

---

## 15. Export Formats

### 15.1 SVG Export

Standalone SVG with embedded CSS. No JavaScript or interactivity.

```xml
<svg xmlns="http://www.w3.org/2000/svg"
     width="{width}" height="{height}"
     viewBox="{viewBox}">
    <style>
        /* Full graph-theme.css + svg.css embedded */
    </style>
    <defs>
        <!-- shadow filter, gradient -->
    </defs>
    <!-- graph content (nodes, edges, clusters) -->
</svg>
```

### 15.2 HTML Export

Self-contained HTML file with full interactivity:

```html
<!DOCTYPE html>
<html>
<head>
    <style>/* Embedded CSS */</style>
</head>
<body>
    <svg class="callgraph" viewBox="...">
        <!-- Full SVG content -->
    </svg>
    <script type="module">
        // Embedded CallGraph class + panzoom
        // Sets up pan/zoom and click interaction
        const graph = new CallGraph(
            document.querySelector(".callgraph"),
            focus  // optional focus position
        );
        graph.setUpPanZoom();
    </script>
</body>
</html>
```

### 15.3 Webview Messages

Export is triggered via `window.postMessage` from webview to extension:

```
{ command: "save SVG", svg: svgString }
{ command: "save HTML", html: htmlString }
{ command: "go to definition", path: filePath, line: lineNumber }
```

---

## 16. Coordinate System and Viewport

### 16.1 Graphviz Coordinates

Graphviz uses a coordinate system where:
- Origin is bottom-left (Y increases upward)
- SVG output is transformed to top-left origin (Y increases downward)
- Units are points (1 point = 1/72 inch at 72 DPI)
- `ranksep` and `nodesep` are in inches (ranksep=2.0 means 144 points)

### 16.2 SVG ViewBox

The SVG viewBox is set by Graphviz to encompass all content with
automatic padding. The `panzoom` library operates on the root `<g>`
transform within this viewBox.

### 16.3 Responsive Sizing

The SVG fills its container (the webview panel or export viewport):

```css
.callgraph {
    width: 100%
    height: 100%
    user-select: none
}
```

---

## Appendix A: Key Source Files

| File | Purpose |
|------|---------|
| `core/src/types/graph.rs` | Graph, File, Symbol, Relation data structures |
| `core/src/types/lsp.rs` | LSP type definitions (DocumentSymbol, SymbolKind) |
| `core/src/generator/mod.rs` | Graph building from LSP data |
| `core/src/lang/mod.rs` | Language trait + default filtering |
| `webview-ui/src/graph/graphviz.ts` | Graph → Graphviz VizGraph conversion |
| `webview-ui/src/graph/render.ts` | SVG post-processing pipeline |
| `webview-ui/src/graph/CallGraph.ts` | Interactive graph (pan/zoom/selection) |
| `webview-ui/src/styles/graph-theme.css` | Color palette (CSS variables) |
| `webview-ui/src/styles/svg.css` | SVG element styling |
| `webview-ui/src/export/templates.ts` | SVG and HTML export templates |
| `editors/code/src/generator.ts` | VS Code LSP request orchestration |
| `editors/code/src/webview.ts` | Webview panel setup |

## Appendix B: Dependencies

**Rust Core:**
- `wasm-bindgen` — JavaScript/WASM FFI
- `serde` / `serde_json` — Serialization
- `serde-wasm-bindgen` — WASM-specific serialization

**Webview UI:**
- `@viz-js/viz` (3.9.0) — Graphviz WASM (DOT layout engine)
- `solid-js` (1.9.7) — Reactive UI framework
- `panzoom` (9.4.3) — Pan and zoom interaction
- `open-props` (1.7.10) — CSS utility properties

**VS Code Extension:**
- `vscode` API — Editor integration, LSP commands
- `ignore` — .gitignore-aware file filtering

## Appendix C: Comparison with Mermaid/Structurizr

| Aspect | Crabviz | Mermaid | Structurizr |
|--------|---------|---------|-------------|
| Input | Live code (LSP) | Text DSL | Text DSL |
| Model | Code symbols + calls | Arbitrary nodes/edges | C4 architecture |
| Layout engine | Graphviz DOT (LR) | dagre/elk (TB) | Sugiyama (TB) |
| Node content | HTML tables with ports | Single labels | Multi-line labels |
| Interactivity | Pan/zoom/select/highlight | None (static) | None (static) |
| Grouping | Directory clusters | Subgraphs | System boundaries |
| Edge semantics | Call / Impl / Inherit | Generic labeled | Typed with technology |
| Primary output | Interactive SVG | Static SVG | Static SVG/PNG |
