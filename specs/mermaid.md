# Mermaid Specification

A language-agnostic specification of how Mermaid transforms diagram text into
rendered output. Written for implementors porting the system to other languages
or runtimes.

---

## Table of Contents

1. [Overview](#1-overview)
2. [High-Level Pipeline](#2-high-level-pipeline)
3. [Text Preprocessing](#3-text-preprocessing)
4. [Diagram Type Detection](#4-diagram-type-detection)
5. [Diagram Definition Structure](#5-diagram-definition-structure)
6. [Parsing](#6-parsing)
7. [The Diagram Database (DB)](#7-the-diagram-database-db)
8. [Universal Data Model](#8-universal-data-model)
9. [Configuration System](#9-configuration-system)
10. [Theming and Styles](#10-theming-and-styles)
11. [Layout Algorithm Integration](#11-layout-algorithm-integration)
12. [The Dagre Layout Adapter](#12-the-dagre-layout-adapter)
13. [Cluster and Subgraph Handling](#13-cluster-and-subgraph-handling)
14. [Rendering Pipeline](#14-rendering-pipeline)
15. [Text Rendering](#15-text-rendering)
16. [Node Shapes](#16-node-shapes)
17. [Edge Rendering](#17-edge-rendering)
18. [Markers (Arrowheads)](#18-markers-arrowheads)
19. [Viewport and Coordinate System](#19-viewport-and-coordinate-system)
20. [Security Model](#20-security-model)
21. [Accessibility](#21-accessibility)
22. [Execution Serialization](#22-execution-serialization)
23. [Diagram Type Catalog](#23-diagram-type-catalog)
24. [Flowchart Diagram (Reference Implementation)](#24-flowchart-diagram-reference-implementation)

---

## 1. Overview

Mermaid is a plugin-based diagramming system that converts a text-based DSL into
visual diagrams. Each diagram type (flowchart, sequence, class, etc.) is an
independent plugin that registers a detector, parser, database, renderer, and
style provider.

The core system provides:
- A preprocessing pipeline that normalizes text and extracts configuration
- A type detection mechanism that routes text to the correct diagram plugin
- A configuration cascade with security controls
- A rendering framework with pluggable layout algorithms
- A universal node/edge data model for layout interchange

The fundamental flow is:

```
Text → Preprocess → Detect Type → Load Plugin → Parse → Build DB
     → getData() → Layout → Render SVG
```

---

## 2. High-Level Pipeline

### 2.1 Entry Points

There are three public entry points:

1. **run()** — Scans the DOM for elements with a marker class (default
   `.mermaid`), extracts their text content, and renders each one in place.

2. **render(id, text)** — Takes a diagram ID and text, returns rendered SVG
   markup and optional bind functions (for interactive elements like click
   handlers).

3. **parse(text)** — Validates syntax without rendering. Returns a parse result
   indicating success/failure and the detected diagram type.

All three pass through the same preprocessing and detection stages. `render()`
continues through the full pipeline to produce output.

### 2.2 Execution Order

```
1. Preprocess text
2. Detect diagram type
3. Lazy-load diagram definition (if not yet loaded)
4. Create Diagram object
   a. Clear database state
   b. Call diagram init (if defined)
   c. Run parser → populates database
5. Insert CSS styles into SVG
6. Call renderer.draw()
   a. db.getData() → {nodes, edges, config}
   b. Configure LayoutData
   c. Call layout algorithm render()
   d. Position elements, insert into SVG
7. Calculate viewport, set viewBox
8. Sanitize output (if security level requires it)
9. Return SVG string
```

---

## 3. Text Preprocessing

Preprocessing transforms raw input text into clean diagram code plus extracted
configuration. The steps run in fixed order.

### 3.1 Text Cleanup

1. Replace CRLF (`\r\n`) with LF (`\n`)
2. Replace HTML-encoded attribute quotes (`&quot;`) with `"`
3. Replace `<br>` and `<br/>` tags with `\n`

### 3.2 Frontmatter Extraction

YAML frontmatter is delimited by `---` lines at the start of the text:

```
---
title: My Diagram
config:
  theme: dark
---
flowchart LR
  A --> B
```

Extracted fields:
- **title** — Diagram title (used for accessibility)
- **displayMode** — Optional display mode hint
- **config** — Configuration overrides (merged into the config cascade)

The regex pattern matches `---` at start-of-string, captures everything until
the closing `---`, and strips both delimiters from the diagram text.

### 3.3 Directive Processing

Directives are inline configuration blocks with the syntax:

```
%%{init: {"theme": "dark", "flowchart": {"curve": "basis"}}}%%
```

Pattern: `%%{` keyword `:` JSON-value `}%%`

The keyword is always `init` for configuration directives. The JSON value is
parsed and merged into the configuration cascade. Multiple directives are
processed in order.

After extraction, directive text is removed from the diagram source.

### 3.4 Comment Removal

Lines containing `%%` (but NOT `%%{`) are comment markers. Everything from `%%`
to end-of-line is stripped. Full-line comments are removed entirely.

### 3.5 Preprocessing Output

The preprocessor returns:
- **code** — Clean diagram text with frontmatter, directives, and comments removed
- **title** — Extracted title (or undefined)
- **config** — Merged configuration from frontmatter and directives

---

## 4. Diagram Type Detection

### 4.1 Detector Registration

Each diagram type registers a detector function with a priority ordering. The
detector takes the cleaned text and optionally the current config, and returns
a boolean.

Detectors are stored in an ordered list. Detection iterates the list and returns
the ID of the first detector that matches.

### 4.2 Detection Algorithm

```
function detectType(text):
    stripped = remove frontmatter, directives, and comments from text
    for each (id, detector) in registered_detectors:
        if detector(stripped, config):
            return id
    throw UnknownDiagramError
```

### 4.3 Typical Detector Patterns

Most detectors use a simple regex test on the first keyword:

| Diagram Type | Matches |
|-------------|---------|
| flowchart-v2 | `/^\s*flowchart/` |
| flowchart | `/^\s*graph/` |
| sequence | `/^\s*sequenceDiagram/` |
| class | `/^\s*classDiagram/` |
| state | `/^\s*stateDiagram/` |
| er | `/^\s*erDiagram/` |
| gantt | `/^\s*gantt/` |
| pie | `/^\s*pie/` |
| gitGraph | `/^\s*gitGraph/` |
| mindmap | `/^\s*mindmap/` |
| timeline | `/^\s*timeline/` |
| quadrantChart | `/^\s*quadrantChart/` |
| sankey | `/^\s*sankey-beta/` |
| xychart | `/^\s*xychart-beta/` |
| block | `/^\s*block-beta/` |
| packet | `/^\s*packet-beta/` |
| kanban | `/^\s*kanban-beta/` |
| architecture | `/^\s*architecture-beta/` |

Registration order matters: if two detectors could both match, the first one
registered wins. For example, `flowchart-v2` (matching `flowchart`) is
registered before `flowchart` (matching `graph`).

### 4.4 Lazy Loading

Diagram definitions are loaded lazily — only the detector function is registered
eagerly. When a detector matches for the first time, the full diagram module
(parser, db, renderer, styles) is loaded via an async import. This keeps
initial load time small when many diagram types are registered but only one is
used.

---

## 5. Diagram Definition Structure

Every diagram type provides a definition object with these components:

```
DiagramDefinition:
    db:        DiagramDB       // State management
    parser:    ParserDefinition // Text → DB population
    renderer:  DiagramRenderer  // DB → SVG output
    styles:    StyleFunction    // Theme variables → CSS string
    init?:     InitFunction     // Optional initialization hook
```

### 5.1 Component Roles

- **db** — Stores parsed diagram state. Provides `clear()` to reset and
  `getData()` to export universal node/edge format.
- **parser** — Transforms cleaned text into db mutations. Either JISON-generated
  grammar or Langium-based.
- **renderer** — Has a `draw(text, id, version, diagObj)` method that orchestrates
  layout and SVG creation.
- **styles** — A function `(theme_variables) → css_string` that generates
  diagram-specific CSS.
- **init** — Optional hook called after db.clear() but before parsing. Used for
  config-dependent initialization.

### 5.2 Utility Injection

When a diagram is registered, shared utility functions are injected into it.
This provides diagram plugins access to core services without direct imports:
- `getConfig()` — Read current configuration
- `sanitizeText()` — Clean user text for safe rendering
- `setupGraphViewbox()` — Configure SVG viewport
- `commonDb` — Shared DB utilities (title, accDescription, etc.)
- Other rendering utilities

---

## 6. Parsing

### 6.1 Parser Types

**JISON Parsers (Legacy)**

JISON is a JS parser generator similar to Bison/Yacc. JISON parsers:
- Are generated from `.jison` grammar files at build time
- Use a shared `yy` object as the bridge between parser actions and the DB
- Grammar actions call `yy.addVertex()`, `yy.addLink()`, etc.
- Before parsing, the system sets `parser.parser.yy = db` to wire up the bridge

**Langium Parsers (Newer)**

Langium is a TypeScript-based language engineering framework. Langium parsers:
- Use `.langium` grammar definitions
- Produce a typed AST
- A separate transformer walks the AST and populates the DB
- More type-safe and maintainable than JISON

### 6.2 Parser Interface

Both parser types present the same interface to the system:

```
ParserDefinition:
    parse(text: string): void   // Parse text and populate the db
    parser?: { yy: any }        // JISON bridge (JISON parsers only)
```

### 6.3 Parse-to-DB Flow

```
1. parser.parser.yy = db          (JISON only — wire up bridge)
2. parser.parse(preprocessed_text)
3. Parser actions mutate db state:
   - db.addVertex(id, text, type, style)
   - db.addLink(from, to, linkType, text)
   - db.addSubGraph(id, title, members)
   - db.setDirection(dir)
   - etc.
```

---

## 7. The Diagram Database (DB)

### 7.1 DB Interface

All diagram databases implement (explicitly or implicitly):

```
DiagramDB:
    clear(): void                        // Reset all state
    getData(): { nodes, edges, config }  // Export universal format
    getConfig(): DiagramConfig           // Get diagram-specific config
    setDiagramTitle(title): void
    getDiagramTitle(): string
    setAccTitle(title): void
    getAccTitle(): string
    setAccDescription(desc): void
    getAccDescription(): string
```

### 7.2 DB Patterns

**Class-based (modern):**
The DB is a class instance. Methods are bound in the constructor for
compatibility with JISON's `yy` bridge pattern (JISON calls methods as
standalone functions, so they must be pre-bound).

**Module-level (legacy):**
State is held in module-level variables. Exported functions mutate that state
directly. `clear()` resets all variables.

### 7.3 State Lifecycle

```
1. Diagram constructed → db.clear() called
2. init() hook called (if defined) → may configure db based on config
3. parser.parse(text) → parser actions mutate db state
4. renderer.draw() → calls db.getData() to read accumulated state
5. After rendering, db state persists until next clear()
```

### 7.4 getData() — The Universal Bridge

`getData()` is the critical method that transforms diagram-specific internal
state into the universal `{nodes, edges}` format consumed by layout algorithms.
See [Section 8](#8-universal-data-model) for the data model.

---

## 8. Universal Data Model

All diagram types ultimately produce data in a universal format for layout and
rendering.

### 8.1 LayoutData

The top-level structure passed to layout algorithms:

```
LayoutData:
    nodes:       Node[]           // All nodes to lay out
    edges:       Edge[]           // All edges to route
    config:      object           // Merged configuration
    type:        string           // Diagram type identifier
    diagramId:   string           // Unique diagram instance ID
    direction:   string           // "TB" | "BT" | "LR" | "RL"
    markers:     string[]         // Arrow marker types needed
    nodeSpacing: number           // Minimum space between nodes
    rankSpacing: number           // Minimum space between ranks
```

### 8.2 Node

```
Node:
    id:          string           // Unique node identifier
    label:       string           // Display text (may contain markdown)
    type:        string           // "normal" or structural type
    shape:       string           // Shape identifier (see Section 16)
    parentId?:   string           // Parent subgraph/cluster ID
    isGroup:     boolean          // True if this is a subgraph container
    
    // Dimensions (set during rendering, before layout)
    width:       number
    height:      number
    
    // Position (set by layout algorithm)
    x?:          number
    y?:          number
    
    // Style
    cssStyles?:  string           // Inline CSS
    cssClasses?: string           // CSS class names
    padding?:    number           // Internal padding
    
    // Rich content
    icon?:       string           // Icon identifier
    img?:        string           // Image URL
    
    // Subgraph properties
    dir?:        string           // Direction override for subgraph
    labelStyle?: string           // Style for label text
    
    // Layout metadata
    domId:       string           // DOM element ID (diagramId + nodeId)
    labelBBox?:  { width, height } // Measured label dimensions
```

### 8.3 Edge

```
Edge:
    id:             string        // Unique edge identifier
    start:          string        // Source node ID
    end:            string        // Target node ID
    label?:         string        // Edge label text
    type:           string        // Edge style type
    
    // Arrow configuration
    arrowTypeStart: string        // "arrow_open" | "arrow_point" | "arrow_circle" | "arrow_cross" | "none"
    arrowTypeEnd:   string        // Same options
    
    // Label positioning
    startLabelRight?: string      // Right-side label at start
    startLabelLeft?:  string      // Left-side label at start  
    endLabelRight?:   string      // Right-side label at end
    endLabelLeft?:    string      // Left-side label at end
    
    // Style
    stroke:         string        // "normal" | "thick" | "dotted" | "invisible"
    cssStyles?:     string        // Inline CSS
    cssClasses?:    string        // CSS class names
    
    // Curve
    curve?:         CurveFunction // Interpolation curve type
    
    // Layout results
    points?:        Point[]       // Routed path points (set by layout)
    
    // Cluster references
    fromCluster?:   string        // Source cluster (if edge was rewired)
    toCluster?:     string        // Target cluster (if edge was rewired)
```

### 8.4 Point

```
Point:
    x: number
    y: number
```

---

## 9. Configuration System

### 9.1 Configuration Cascade

Configuration is resolved through a four-level cascade (later levels override
earlier ones, subject to security restrictions):

```
Level 1: defaultConfig     — Built-in defaults, read-only
Level 2: siteConfig        — Set by initialize(), persists across renders
Level 3: directives[]      — From %%{init}%% in diagram text, per-render
Level 4: currentConfig     — Computed merge of levels 1-3
```

### 9.2 Key Configuration Properties

```
Global:
    theme:           string    // "default" | "dark" | "forest" | "neutral" | "base"
    securityLevel:   string    // "sandbox" | "strict" | "loose" | "antiscript"
    maxTextSize:     number    // Maximum input text length
    maxEdges:        number    // Maximum number of edges
    fontFamily:      string    // Base font family
    fontSize:        number    // Base font size
    logLevel:        number    // Logging verbosity
    
Per-diagram (e.g., flowchart):
    curve:           string    // Edge curve interpolation
    nodeSpacing:     number    // Space between nodes
    rankSpacing:     number    // Space between ranks
    padding:         number    // Node internal padding
    defaultRenderer: string    // Layout algorithm to use
    htmlLabels:      boolean   // Use HTML or SVG text rendering
    diagramPadding:  number    // Padding around diagram
```

### 9.3 Configuration Resolution

```
function getConfig():
    currentConfig = deepClone(siteConfig)
    for each directive in directives:
        sanitize(directive)          // Remove secure keys
        merge(currentConfig, directive)
    return currentConfig
```

### 9.4 Secure Keys

Certain configuration keys cannot be overridden via directives (inline in
diagram text) because they could enable code execution or XSS:

- `securityLevel`
- `secure` (the list of secure keys itself)
- `callback`
- Any key containing `<script>` in its value

The `sanitize()` function recursively walks directive objects and removes any
key in the secure list.

### 9.5 Reset Behavior

`reset()` reverts `currentConfig` to `siteConfig` and clears all accumulated
directives. Called between renders to prevent config leakage.

---

## 10. Theming and Styles

### 10.1 Theme Architecture

Themes are defined as a set of named color/style variables. Each named theme
provides its own set of variable values. The "base" theme allows full user
customization of all variables.

Built-in themes:
- **default** — Light theme with blue/grey palette
- **dark** — Dark background with light text
- **forest** — Green-toned palette
- **neutral** — Greyscale, black and white friendly
- **base** — All variables user-configurable

### 10.2 Theme Variable Resolution

```
1. Start with named theme's default variables
2. Apply user overrides from config.themeVariables
3. Compute derived variables (e.g., lighter/darker shades)
4. Pass resolved variables to each diagram's style function
```

### 10.3 Style Compilation

Each diagram type provides a style function:

```
styles(themeVariables) → css_string
```

This generates CSS rules specific to that diagram type. Common elements (edges,
markers, labels) have shared styles. The CSS is:

1. Generated from theme variables
2. Scoped to the diagram's SVG element ID
3. Inserted as a `<style>` element inside the SVG
4. Compiled/processed for vendor prefixes (using a CSS preprocessor like stylis)

### 10.4 Common Style Elements

Shared across all diagram types:
- Edge path stroke colors and widths
- Edge animations (dashed/dotted stroke-dasharray animations)
- Marker (arrowhead) fill colors
- Label text styling
- Cluster/subgraph border and fill colors
- Neo look (rounded corners, shadow effects)

---

## 11. Layout Algorithm Integration

### 11.1 Pluggable Layout System

Layout algorithms are pluggable. The system maintains a registry of layout
loaders, keyed by name:

```
layoutLoaders:
    "dagre"          → loads dagre adapter
    "elk"            → loads ELK adapter
    "cose-bilkent"   → loads Cytoscape CoSE-Bilkent adapter
```

### 11.2 Layout Algorithm Interface

Each layout algorithm provides:

```
LayoutAlgorithm:
    render(layoutData, svg): Promise<void>
```

Where `layoutData` is the `LayoutData` structure from Section 8.1, and `svg` is
the target SVG element.

### 11.3 Layout Selection

The layout algorithm is selected per-render:
1. Diagram renderer specifies an algorithm name (default: `"dagre"`)
2. The render pipeline looks up the loader by name
3. The loader lazily imports the layout module
4. The layout's `render()` is called

### 11.4 Render Pipeline Integration

```
render(layoutData, svg):
    layoutAlgorithm = await loadLayout(layoutData.config.layout || "dagre")
    
    // Prefix all node DOM IDs with diagram ID for uniqueness
    for node in layoutData.nodes:
        node.domId = layoutData.diagramId + "-" + node.id
    
    // Set up SVG definitions (shadows, gradients)
    setupSVGDefs(svg)
    
    // Delegate to the layout algorithm
    await layoutAlgorithm.render(layoutData, svg)
```

---

## 12. The Dagre Layout Adapter

Dagre is the default layout algorithm. The adapter bridges between mermaid's
universal data model and dagre's graphlib-based interface.

### 12.1 Graph Construction

```
1. Create compound multigraph with configuration:
   - rankdir: from layoutData.direction ("TB" | "BT" | "LR" | "RL")
   - nodesep: from config.nodeSpacing
   - ranksep: from config.rankSpacing
   - marginx: 8
   - marginy: 8

2. For each node in layoutData.nodes:
   - graph.setNode(node.id, node)
   - If node has parentId: graph.setParent(node.id, node.parentId)

3. For each edge in layoutData.edges:
   - If self-loop (start === end):
       Create two dummy nodes and three edges to form a loop path
       (see Section 12.2)
   - Else:
       graph.setEdge(edge.start, edge.end, edge, edge.id)

4. Call adjustClustersAndEdges(graph) (see Section 13)
5. Call recursiveRender() (see Section 12.3)
```

### 12.2 Self-Loop Handling

Self-referencing edges (where start === end) cannot be laid out by dagre
directly. The adapter decomposes them:

```
For edge A → A:
    Create dummy node A---A---1 (small 10x10 labelRect)
    Create dummy node A---A---2 (small 10x10 labelRect)
    Set parents to same subgraph as A
    
    Create 3 edges:
    1. A → A---A---1     (carries no label, no end arrow)
    2. A---A---1 → A---A---2  (carries the label, no arrows)
    3. A---A---2 → A     (carries no label, has the end arrow)
    
    This forces dagre to route around the node, creating a visible loop path.
```

### 12.3 Recursive Rendering

The dagre adapter uses recursive rendering to handle compound graphs (subgraphs
within subgraphs). Each level of nesting is a separate dagre layout call.

```
recursiveRender(element, graph, diagramType, id, parentCluster, config):
    // Create SVG group structure
    root     = element.insert("g", class="root")
    clusters = root.insert("g", class="clusters")
    edgePaths = root.insert("g", class="edgePaths")
    edgeLabels = root.insert("g", class="edgeLabels")
    nodes    = root.insert("g", class="nodes")
    
    // Phase 1: Insert nodes into SVG to measure dimensions
    for each node v in graph:
        if node is a clusterNode (has nested graph):
            // Recursively render the cluster's sub-graph
            result = await recursiveRender(nodes, node.graph, ...)
            // Update node dimensions from rendered sub-graph
            node.width = measured_width
            node.height = measured_height
        elif node has children (cluster, not recursively rendered):
            // Store cluster reference for later
            clusterDb[node.id] = {id: findNonClusterChild(node.id), node}
        else:
            // Leaf node — insert SVG shape element
            insertNode(nodes, node, config)
    
    // Phase 2: Insert edge labels to measure their dimensions
    for each edge e in graph:
        insertEdgeLabel(edgeLabels, edge_data)
    
    // Phase 3: Run dagre layout
    dagreLayout(graph)
    
    // Phase 4: Position everything according to layout results
    for each node v in hierarchy order:
        if clusterNode:
            node.y += subGraphTitleMargin
            positionNode(node)  // Move SVG element to (x, y)
        elif has children (pure cluster):
            node.height += subGraphTitleMargin
            insertCluster(clusters, node)  // Draw cluster border
        else:
            node.y += subGraphTitleMargin / 2
            positionNode(node)
    
    // Phase 5: Route and draw edges
    for each edge e in graph:
        // Offset edge points by subgraph title margin
        for each point in edge.points:
            point.y += subGraphTitleMargin / 2
        // Draw edge path with arrows
        insertEdge(edgePaths, edge, clusterDb, ...)
        positionEdgeLabel(edge, paths)
    
    return { element, diff }
```

### 12.4 Hierarchy-Ordered Positioning

Nodes are positioned in hierarchy order (parents before children) to ensure
cluster containers are positioned before their contents. The sorting algorithm:

```
sortNodesByHierarchy(graph):
    result = graph.children()  // Root-level nodes
    for each node in result:
        children = graph.children(node)
        result.append(sortNodesByHierarchy(children))
    return result
```

---

## 13. Cluster and Subgraph Handling

Clusters (subgraphs) require special handling because dagre's layout algorithm
operates on flat graphs — edges must connect leaf nodes, not cluster containers.

### 13.1 Overview

The cluster adjustment process:
1. Identifies which nodes are clusters (have children)
2. Determines which clusters have external connections
3. Rewires edges from clusters to representative leaf nodes
4. Extracts internally-connected clusters into separate sub-graphs
5. Each sub-graph gets its own dagre layout call via recursive rendering

### 13.2 Data Structures

```
clusterDb: Map<string, {
    id:                  string    // Representative leaf node ID
    clusterData:         object    // Original cluster node data
    externalConnections: boolean   // Has edges crossing cluster boundary
    label?:              string
    node?:               object    // Positioned node (set after layout)
}>

descendants: Map<string, string[]>  // clusterId → all descendant node IDs
parents: Map<string, string>        // nodeId → parent cluster ID
```

### 13.3 adjustClustersAndEdges(graph)

**Phase 1: Identify clusters and find representative nodes**

```
for each node in graph:
    if node has children:
        descendants[node] = extractDescendants(node, graph)
        clusterDb[node] = {
            id: findNonClusterChild(node, graph, node),
            clusterData: graph.node(node)
        }
```

`extractDescendants(id, graph)` recursively collects all descendant node IDs:

```
extractDescendants(id, graph):
    children = graph.children(id)
    result = [...children]
    for child in children:
        parents[child] = id
        result.append(extractDescendants(child, graph))
    return result
```

`findNonClusterChild(id, graph, clusterId)` finds a leaf node to represent the
cluster in edge connections:

```
findNonClusterChild(id, graph, clusterId):
    children = graph.children(id)
    if no children:
        return id     // This is a leaf — use it
    
    reserve = null
    for each child:
        candidate = findNonClusterChild(child, graph, clusterId)
        commonEdges = findCommonEdges(graph, clusterId, candidate)
        if candidate exists:
            if commonEdges exist:
                reserve = candidate   // Has common edges — less preferred
            else:
                return candidate      // No common edges — preferred
    return reserve
```

**Phase 2: Detect external connections**

```
for each cluster in graph:
    for each edge in graph:
        d1 = isDescendant(edge.v, cluster)
        d2 = isDescendant(edge.w, cluster)
        if d1 XOR d2:                       // Edge crosses cluster boundary
            clusterDb[cluster].externalConnections = true
```

**Phase 3: Adjust parent references**

```
for each cluster in clusterDb:
    nonClusterChild = clusterDb[cluster].id
    parent = graph.parent(nonClusterChild)
    if parent !== cluster AND clusterDb[parent] exists
       AND NOT clusterDb[parent].externalConnections:
        clusterDb[cluster].id = parent
```

**Phase 4: Rewire edges**

```
for each edge (v → w) in graph:
    if v or w is in clusterDb:
        newV = getAnchorId(v)       // Resolve to representative leaf node
        newW = getAnchorId(w)
        graph.removeEdge(v, w)
        if newV !== v:
            mark parent cluster as having external connections
            edge.fromCluster = v
        if newW !== w:
            mark parent cluster as having external connections
            edge.toCluster = w
        graph.setEdge(newV, newW, edge)
```

`getAnchorId(id)`: if ID is in clusterDb and has external connections, return
the representative leaf node ID; otherwise return the ID unchanged.

**Phase 5: Extract sub-graphs**

```
extractor(graph, depth):
    if depth > 10: bail out (safety limit)
    if no node has children: done
    
    for each node:
        if node is cluster AND no external connections AND has children:
            // Create sub-graph for this cluster
            clusterGraph = new compound multigraph
            clusterGraph.setGraph({
                rankdir: perpendicular to parent (TB↔LR), or cluster's own dir
                nodesep: 50, ranksep: 50, marginx: 8, marginy: 8
            })
            
            // Copy all descendants into clusterGraph
            copy(node, graph, clusterGraph, node)
            
            // Replace cluster in parent graph with a single compound node
            graph.setNode(node, {
                clusterNode: true,
                id: node,
                clusterData: original_data,
                graph: clusterGraph    // Nested graph for recursive rendering
            })
    
    // Recurse into extracted sub-graphs
    for each node in graph:
        if node.clusterNode:
            extractor(node.graph, depth + 1)
```

The `copy()` function moves nodes and their edges from the parent graph into
a sub-graph:

```
copy(clusterId, graph, newGraph, rootId):
    nodes = graph.children(clusterId)
    if clusterId !== rootId:
        nodes.push(clusterId)
    
    for each node:
        if node has children:
            copy(node, graph, newGraph, rootId)  // Recurse
        else:
            newGraph.setNode(node, graph.node(node))
            // Set parent relationships
            if rootId !== graph.parent(node):
                newGraph.setParent(node, graph.parent(node))
            if clusterId !== rootId AND node !== clusterId:
                newGraph.setParent(node, clusterId)
            
            // Copy edges that stay within the cluster
            for each edge of node:
                if edgeInCluster(edge, rootId):
                    newGraph.setEdge(edge.v, edge.w, edge_data, edge.name)
            
            graph.removeNode(node)
```

### 13.4 Validation

Before layout, the graph is validated:

```
validate(graph):
    for each edge:
        if edge.v has children OR edge.w has children:
            return false    // Edges must connect leaf nodes only
    return true
```

---

## 14. Rendering Pipeline

### 14.1 SVG Structure

The rendered output is an SVG element with this structure:

```
<svg id="{diagramId}" class="mermaid" viewBox="...">
    <style>
        /* Scoped CSS from theme + diagram styles */
    </style>
    <defs>
        <!-- Markers (arrowheads) -->
        <!-- Filters (drop shadows) -->
        <!-- Gradients -->
    </defs>
    <g class="root">
        <g class="clusters">
            <!-- Subgraph borders/backgrounds -->
        </g>
        <g class="edgePaths">
            <!-- Edge path lines and arrows -->
        </g>
        <g class="edgeLabels">
            <!-- Edge label text -->
        </g>
        <g class="nodes">
            <!-- Node shapes and labels -->
        </g>
    </g>
</svg>
```

### 14.2 Rendering Order

The SVG group ordering determines visual stacking:
1. Clusters (background) — drawn first, appear behind everything
2. Edge paths — drawn on top of clusters
3. Edge labels — drawn on top of edge paths
4. Nodes — drawn last, appear on top of everything

This ensures nodes visually sit above edges, and edges sit above cluster
backgrounds.

### 14.3 SVG Definitions

Standard definitions inserted into every diagram:

**Drop Shadow Filter:**
```
<filter id="drop-shadow">
    <feDropShadow dx="0" dy="3" stdDeviation="3"
                  flood-color="rgba(0,0,0,0.25)" />
</filter>
```

**Gradients** — Used for specific node shapes (cylinders, etc.)

**Markers** — Arrowhead definitions (see Section 18)

### 14.4 Diagram Renderer draw()

Each diagram type's `draw()` method follows this pattern:

```
draw(text, id, version, diagObj):
    data = diagObj.db.getData()        // Get {nodes, edges, config}
    
    layoutData = {
        nodes: data.nodes,
        edges: data.edges,
        type: diagramType,
        diagramId: id,
        direction: data.config.direction || "TB",
        markers: determineMarkerTypes(data.edges),
        config: data.config,
        nodeSpacing: config.nodeSpacing,
        rankSpacing: config.rankSpacing,
    }
    
    await render(layoutData, svg)      // Call layout + render pipeline
    setupViewPortForSVG(svg, padding)  // Set final viewBox
```

---

## 15. Text Rendering

### 15.1 Dual-Mode Text

Mermaid supports two text rendering modes, controlled by the `htmlLabels`
configuration option:

**HTML Mode (default, `htmlLabels: true`):**
- Text is rendered as HTML inside an SVG `<foreignObject>` element
- Supports rich formatting: markdown (bold, italic), math (KaTeX), icons
- Text is sanitized via DOMPurify before insertion
- Allows natural word wrapping via CSS

**SVG Mode (`htmlLabels: false`):**
- Text is rendered as SVG `<text>` elements with `<tspan>` children
- Limited formatting: bold and italic via font-weight/font-style attributes
- Each line is a separate `<tspan>` with dy offset
- No math or icon support

### 15.2 Markdown Processing

Label text can contain markdown formatting. The processing pipeline:

```
1. Preprocess: normalize whitespace, handle line breaks
2. Parse with marked (markdown library):
   - Extract bold (**text** or __text__) 
   - Extract italic (*text* or _text_)
   - Detect line breaks (<br> or double-newline)
3. Convert to intermediate format:
   MarkdownLine: MarkdownWord[]
   MarkdownWord: { content, type: "normal"|"emphasis"|"strong" }
4. For HTML mode: convert to HTML string with <strong>/<em> tags
5. For SVG mode: create <tspan> elements with font-weight/font-style
```

### 15.3 Text Measurement

Text dimensions are measured before layout (dagre needs node widths/heights):

```
calculateTextDimensions(text, config):
    // Create temporary invisible SVG text element
    // Set font-family, font-size, font-weight from config
    // Measure bounding box
    // Return { width, height }
    
    // Results are memoized for performance
```

### 15.4 Text Wrapping

Long labels are wrapped to fit within a maximum width:

```
splitLineToFitWidth(line, maxWidth, config):
    words = segment(line)              // Use Unicode segmenter
    currentLine = ""
    lines = []
    for each word:
        testLine = currentLine + word
        if measureWidth(testLine) > maxWidth:
            lines.push(currentLine)
            currentLine = word
        else:
            currentLine = testLine
    lines.push(currentLine)
    return lines
```

Unicode-aware segmentation (via `Intl.Segmenter` or equivalent) is used for
proper CJK and complex script handling.

---

## 16. Node Shapes

### 16.1 Shape Registry

Mermaid supports approximately 70 node shapes. Each shape is a function that:
1. Takes a node's data (label, dimensions, style)
2. Creates SVG elements (path, rect, polygon, etc.)
3. Returns the shape element with intersection points for edge routing

### 16.2 Core Shape Categories

**Geometric:**
- `rect` — Rectangle (default)
- `roundedRect` — Rectangle with rounded corners
- `circle` — Circle
- `diamond` / `question` — Diamond/rhombus
- `hexagon` — Hexagon
- `triangle` / `triangleUp` / `triangleDown` — Triangles
- `trapezoid` / `trapezoidAlt` — Trapezoids
- `stadium` — Stadium/pill shape (fully rounded ends)
- `lean-right` / `lean-left` — Parallelograms

**Notation/UML:**
- `cylinder` — Database cylinder
- `doublecircle` — Double circle (state diagram)
- `forkJoin` — Fork/join bar (activity diagram)
- `classBox` — UML class box with sections

**Special:**
- `labelRect` — Invisible rectangle for edge labels
- `note` — Sticky note shape
- `subroutine` — Double-bordered rectangle
- `flag` — Flag shape
- `cloud` — Cloud shape

### 16.3 Shape Rendering Contract

```
shapeFunction(parent, node, config):
    // 1. Create SVG group
    group = parent.insert("g")
    
    // 2. Render label text (HTML or SVG mode)
    label = createText(group, node.label, ...)
    
    // 3. Measure label dimensions
    bbox = label.getBBox()
    
    // 4. Calculate shape dimensions (label + padding)
    width = bbox.width + node.padding * 2
    height = bbox.height + node.padding * 2
    
    // 5. Draw shape path/element
    shape = group.insert("rect/path/polygon", ...)
    
    // 6. Apply styles
    shape.attr("style", node.cssStyles)
    shape.attr("class", "node " + node.cssClasses)
    
    // 7. Update node dimensions for layout
    node.width = width
    node.height = height
    
    // 8. Store intersection function for edge routing
    node.intersect = function(point):
        return intersectRect(node, point)  // or shape-specific intersect
    
    return group
```

### 16.4 Intersection Functions

Each shape provides an intersection function used by edge routing to determine
where an edge line meets the shape boundary. Common intersection types:

- **Rectangle**: Line-rectangle intersection (see dagre spec Section 13.4)
- **Circle**: Line-circle intersection
- **Diamond**: Line-polygon intersection (4 sides)
- **Polygon**: General line-polygon intersection (iterates edges)

---

## 17. Edge Rendering

### 17.1 Edge Path Generation

After layout, edges have an array of routed `points`. The rendering pipeline:

```
1. Select interpolation curve from config (default: "basis")
2. Map curve name to curve function:
   - "basis" → curveBasis (smooth B-spline)
   - "linear" → curveLinear (straight segments)
   - "monotoneX" → curveMonotoneX
   - "monotoneY" → curveMonotoneY
   - "natural" → curveNatural
   - "step" → curveStep
   - "stepAfter" → curveStepAfter
   - "stepBefore" → curveStepBefore
   - "cardinal" → curveCardinal (with configurable tension)
   - "catmullRom" → curveCatmullRom

3. Generate SVG path using line generator with curve interpolation
4. Clip endpoints to node shape boundaries using intersection functions
```

### 17.2 Edge Styles

Edges have four stroke styles:

| Style | SVG Representation |
|-------|-------------------|
| `normal` | Solid line, standard width |
| `thick` | Solid line, increased width |
| `dotted` | `stroke-dasharray: 3` |
| `invisible` | `display: none` |

### 17.3 Edge Labels

Edge labels are positioned at the midpoint of the edge path:

```
positionEdgeLabel(edge, paths):
    midpoint = edge.points[floor(edge.points.length / 2)]
    label.transform = translate(midpoint.x, midpoint.y)
```

Additional labels (startLabelLeft, startLabelRight, endLabelLeft, endLabelRight)
are positioned near the respective endpoints using offset calculations.

### 17.4 Edge Label Position Calculation

```
calcLabelPosition(points):
    if 3+ points (odd count):
        midIdx = floor(length / 2)
        return points[midIdx]
    elif even count:
        midIdx = length / 2 - 1
        return midpoint(points[midIdx], points[midIdx + 1])
```

---

## 18. Markers (Arrowheads)

### 18.1 Marker Types

Markers are SVG `<marker>` elements defined in `<defs>` and referenced by edges:

| Marker | Shape | Description |
|--------|-------|-------------|
| `arrow_point` | Filled triangle | Standard arrowhead |
| `arrow_circle` | Filled circle | Circle marker |
| `arrow_cross` | X shape | Cross/cancel marker |
| `arrow_barb` | Open chevron | Barb marker |
| `arrow_open` | None | No marker (open end) |
| `aggregation` | Diamond (hollow) | UML aggregation |
| `composition` | Diamond (filled) | UML composition |
| `dependency` | Open arrowhead | UML dependency |
| `lollipop` | Circle on stem | UML interface |

### 18.2 Marker Definition

```
<marker id="{type}_{diagramId}"
        refX="center" refY="center"
        markerWidth="size" markerHeight="size"
        orient="auto-start-reverse">
    <path d="..." />
</marker>
```

The `orient="auto-start-reverse"` attribute automatically rotates the marker
to match the edge direction and mirrors it for start vs. end positions.

### 18.3 Marker Application

```
edge_path.attr("marker-end", "url(#arrow_point_{diagramId})")
edge_path.attr("marker-start", "url(#arrow_circle_{diagramId})")
```

---

## 19. Viewport and Coordinate System

### 19.1 Abstract Coordinates

All layout operates in abstract coordinate space. Node positions (x, y) and
edge points use arbitrary units that correspond to pixels at 1:1 zoom.

### 19.2 ViewBox Calculation

After rendering, the SVG viewBox is calculated to fit all content with padding:

```
setupViewPortForSVG(svg, padding):
    bbox = svg.getBBox()         // Measure rendered content bounds
    
    width = bbox.width + padding.left + padding.right
    height = bbox.height + padding.top + padding.bottom
    
    viewBox = [
        bbox.x - padding.left,
        bbox.y - padding.top,
        width,
        height
    ]
    
    svg.attr("viewBox", viewBox.join(" "))
    
    // For responsive sizing:
    svg.attr("width", "100%")
    svg.attr("height", height)
    // Or fixed:
    svg.attr("width", width)
    svg.attr("height", height)
```

### 19.3 Padding Configuration

Default padding is `diagramPadding` (typically 8) on all sides. Some diagram
types add additional padding for titles or legend elements.

### 19.4 Subgraph Title Margins

When subgraphs have titles, additional vertical margin is added:

```
subGraphTitleTotalMargin = subGraphTitleTopMargin + subGraphTitleBottomMargin
```

This margin shifts all content down to make room for the title text above the
cluster border. The margin is applied during the positioning phase of recursive
rendering (Section 12.3, Phase 4).

---

## 20. Security Model

### 20.1 Security Levels

| Level | Behavior |
|-------|----------|
| `sandbox` | Diagram rendered inside an iframe. No direct DOM access. Most restrictive. |
| `strict` | SVG output sanitized through DOMPurify. Event handlers stripped. Click/link disabled. |
| `antiscript` | Script tags removed but other interactive elements allowed. |
| `loose` | No sanitization. All interactive elements preserved. Click handlers work. |

### 20.2 Sanitization

In `strict` mode, the SVG output is processed through DOMPurify:
- Removes `<script>` elements
- Strips event handler attributes (`onclick`, `onload`, etc.)
- Removes `javascript:` URLs
- Preserves SVG structural elements and styling

### 20.3 Configuration Security

- Secure keys in configuration cannot be overridden via directives
- Values containing `<script>` tags are removed during sanitization
- The `callback` configuration key is always secure

### 20.4 Text Sanitization

All user-provided text (labels, titles) passes through `sanitizeText()`:
- Removes HTML tags (in strict/sandbox modes)
- Escapes special characters
- Prevents XSS through label injection

---

## 21. Accessibility

### 21.1 ARIA Attributes

Each rendered SVG includes accessibility metadata:

```
<svg role="graphics-document document" aria-roledescription="diagram">
    <title>{accTitle or diagramTitle}</title>
    <desc>{accDescription}</desc>
    ...
</svg>
```

### 21.2 Setting Accessibility

Diagram text can specify accessibility attributes:

```
accTitle: My Diagram Title
accDescr: A description of what this diagram shows
```

Or multi-line:
```
accDescr {
    A longer description that
    spans multiple lines
}
```

These are stored in the DB and applied during SVG generation.

---

## 22. Execution Serialization

### 22.1 Execution Queue

Render calls are serialized through an execution queue to prevent concurrent
DOM manipulation. This is important because:
- Text measurement requires temporary DOM elements
- Multiple diagrams on the same page could interfere
- SVG insertion and measurement are not thread-safe

```
executionQueue: Promise chain

render(id, text):
    return executionQueue.then(() => doRender(id, text))
```

Each render call chains onto the previous one, ensuring sequential execution
regardless of how many diagrams are requested concurrently.

### 22.2 Implication for Ports

In single-threaded environments (like a native GUI), this serialization may be
unnecessary. In multi-threaded environments, equivalent mutual exclusion should
be implemented around the rendering pipeline, particularly around any shared
mutable state (text measurement caches, configuration state, etc.).

---

## 23. Diagram Type Catalog

Each diagram type follows the same plugin architecture. Here is a summary of
all built-in types and their domain-specific concerns:

### 23.1 Flowchart / Graph

- **Keywords:** `flowchart`, `graph`
- **Direction:** TB, BT, LR, RL
- **Nodes:** Vertices with shapes, labels, styles, icons, images
- **Edges:** Links with labels, arrow types, stroke styles
- **Subgraphs:** Nested grouping with optional direction override
- **Special:** Click handlers, tooltips, CSS classes, YAML metadata on nodes

### 23.2 Sequence Diagram

- **Keyword:** `sequenceDiagram`
- **Participants:** Actors (stick figures) or participants (boxes)
- **Messages:** Solid/dashed, with/without arrowheads
- **Activations:** Nested activation bars on participants
- **Groups:** alt/else, opt, loop, par, critical, break, rect
- **Notes:** Over, left of, right of participants
- **Numbering:** Optional auto-numbering of messages
- **Layout:** Custom (not dagre) — vertical time axis, horizontal participant axis

### 23.3 Class Diagram

- **Keyword:** `classDiagram`
- **Nodes:** Classes with members (attributes + methods), visibility markers
- **Relationships:** Inheritance, composition, aggregation, association, dependency
- **Annotations:** `<<interface>>`, `<<abstract>>`, `<<service>>`, etc.
- **Namespaces:** Grouping of classes
- **Notes:** Attached to classes

### 23.4 State Diagram

- **Keyword:** `stateDiagram` or `stateDiagram-v2`
- **States:** Simple, composite (nested), fork/join, choice, notes
- **Transitions:** With labels and guards
- **Special nodes:** `[*]` for start/end states
- **Concurrency:** `--` separator for concurrent regions

### 23.5 Entity Relationship (ER)

- **Keyword:** `erDiagram`
- **Entities:** With typed attributes and key markers (PK, FK, UK)
- **Relationships:** With cardinality (one, many, zero-or-one, zero-or-more)
- **Labels:** Relationship descriptions

### 23.6 Gantt Chart

- **Keyword:** `gantt`
- **Sections:** Groups of tasks
- **Tasks:** With start dates, durations, dependencies
- **Milestones:** Zero-duration tasks
- **Layout:** Custom — horizontal timeline, no dagre

### 23.7 Pie Chart

- **Keyword:** `pie`
- **Data:** Named sections with numeric values
- **Options:** Title, showData flag
- **Layout:** Custom — circular, no dagre

### 23.8 Git Graph

- **Keyword:** `gitGraph`
- **Branches:** Named branches with commits
- **Operations:** commit, branch, checkout, merge, cherry-pick
- **Layout:** Custom — horizontal commit timeline with branch lanes

### 23.9 Mindmap

- **Keyword:** `mindmap`
- **Nodes:** Hierarchical via indentation
- **Shapes:** Various (rectangle, rounded, circle, bang, cloud, hexagon)
- **Icons:** FontAwesome icon support
- **Layout:** Uses tree layout (not dagre)

### 23.10 Timeline

- **Keyword:** `timeline`
- **Periods:** Time periods with events
- **Layout:** Custom horizontal timeline

### 23.11 Quadrant Chart

- **Keyword:** `quadrantChart`
- **Axes:** x-axis and y-axis with labels
- **Points:** Data points positioned in quadrants
- **Layout:** Custom — 2D scatter plot in quadrants

### 23.12 Sankey

- **Keyword:** `sankey-beta`
- **Nodes:** Sources and sinks
- **Flows:** Weighted connections between nodes
- **Layout:** Custom — Sankey flow diagram

### 23.13 XY Chart

- **Keyword:** `xychart-beta`
- **Series:** Line and bar data series
- **Axes:** Configurable x and y axes
- **Layout:** Custom — standard chart axes

### 23.14 Block Diagram

- **Keyword:** `block-beta`
- **Blocks:** Columns, spaces, and block elements
- **Edges:** Connections between blocks
- **Layout:** Grid-based layout

### 23.15 Packet Diagram

- **Keyword:** `packet-beta`
- **Fields:** Bit ranges with labels
- **Layout:** Custom — horizontal bit field visualization

### 23.16 Kanban Board

- **Keyword:** `kanban-beta`
- **Columns:** Kanban lanes
- **Cards:** Items within lanes
- **Layout:** Custom — column-based board

### 23.17 Architecture Diagram

- **Keyword:** `architecture-beta`
- **Services:** Named services with icons
- **Groups:** Service groups
- **Edges:** Connections between services
- **Layout:** Uses dedicated architecture layout

---

## 24. Flowchart Diagram (Reference Implementation)

The flowchart is the most commonly used diagram type and serves as a reference
implementation for the plugin architecture.

### 24.1 Internal State (FlowDB)

```
vertices:   Map<string, FlowVertex>     // Node definitions
edges:      FlowEdge[]                  // Edge definitions
classes:    Map<string, FlowClass>      // CSS class definitions
subGraphs:  FlowSubGraph[]              // Subgraph hierarchy
direction:  string                       // Default: "TB"
tooltips:   Map<string, string>         // Node tooltips
```

### 24.2 FlowVertex

```
FlowVertex:
    id:          string
    text:        string        // Display label (may contain markdown)
    type:        string        // Shape type identifier
    styles:      string[]      // Inline CSS properties
    classes:     string[]      // CSS class names
    dir?:        string        // Direction (for subgraphs)
    domId:       string        // DOM element ID
    props:       object        // Additional properties (icon, img, etc.)
    parentId?:   string        // Parent subgraph ID
    isGroup:     boolean       // True if this is a subgraph
    labelType:   string        // "text" or "markdown"
```

### 24.3 FlowEdge (Link)

```
FlowEdge:
    start:          string     // Source vertex ID
    end:            string     // Target vertex ID
    text:           string     // Label
    type:           string     // Arrow notation type
    stroke:         string     // "normal" | "thick" | "dotted" | "invisible"
    startLabel:     string     // Label at start
    endLabel:       string     // Label at end
    arrowTypeStart: string     // Marker type at start
    arrowTypeEnd:   string     // Marker type at end
```

### 24.4 FlowSubGraph

```
FlowSubGraph:
    id:          string
    title:       string
    nodes:       string[]      // Member vertex IDs
    dir?:        string        // Direction override
    classes:     string[]      // CSS classes
```

### 24.5 getData() Transform

The critical `getData()` method transforms internal flowchart state into the
universal `{nodes, edges}` format:

```
getData():
    nodes = []
    edges = []
    config = getConfig()
    
    // Transform vertices to universal nodes
    for (id, vertex) in vertices:
        node = {
            id: vertex.id,
            label: vertex.text,
            shape: mapShape(vertex.type),     // Map flowchart shape names
            domId: vertex.domId,
            parentId: vertex.parentId,
            isGroup: vertex.isGroup,
            dir: vertex.dir,
            padding: config.flowchart.padding,
            cssStyles: vertex.styles.join(";"),
            cssClasses: vertex.classes.join(" "),
            icon: vertex.props.icon,
            img: vertex.props.img,
            labelType: vertex.labelType,
        }
        nodes.push(node)
    
    // Transform links to universal edges
    for edge in edges:
        e = {
            id: generateEdgeId(),
            start: edge.start,
            end: edge.end,
            label: edge.text,
            type: "arrow",
            stroke: edge.stroke,
            arrowTypeStart: edge.arrowTypeStart,
            arrowTypeEnd: edge.arrowTypeEnd,
            startLabelLeft: edge.startLabel,
            endLabelLeft: edge.endLabel,
            curve: mapCurve(config.flowchart.curve),
        }
        edges.push(e)
    
    return {
        nodes,
        edges,
        config: { direction: direction },
        markers: collectMarkerTypes(edges),
    }
```

### 24.6 Shape Mapping

Flowchart syntax maps to shape identifiers:

| Syntax | Shape | Description |
|--------|-------|-------------|
| `[text]` | `rect` | Rectangle |
| `(text)` | `roundedRect` | Rounded rectangle |
| `([text])` | `stadium` | Stadium/pill |
| `[[text]]` | `subroutine` | Double-bordered rect |
| `[(text)]` | `cylinder` | Cylinder |
| `((text))` | `doublecircle` | Double circle |
| `{text}` | `diamond` | Diamond |
| `{{text}}` | `hexagon` | Hexagon |
| `[/text/]` | `lean-right` | Parallelogram (right) |
| `[\text\]` | `lean-left` | Parallelogram (left) |
| `[/text\]` | `trapezoid` | Trapezoid |
| `[\text/]` | `trapezoidAlt` | Inverted trapezoid |
| `>text]` | `flag` | Flag shape |
| `(((text)))` | `circle` | Circle |

### 24.7 Arrow Type Mapping

| Syntax | Arrow Type | Stroke |
|--------|-----------|--------|
| `-->` | `arrow_point` | normal |
| `---` | `arrow_open` | normal |
| `-.->` | `arrow_point` | dotted |
| `-.-.` | `arrow_open` | dotted |
| `==>` | `arrow_point` | thick |
| `===` | `arrow_open` | thick |
| `--o` | `arrow_circle` | normal |
| `--x` | `arrow_cross` | normal |
| `<-->` | Both ends `arrow_point` | normal |

### 24.8 Flowchart Renderer

```
draw(text, id, version, diagObj):
    db = diagObj.db
    data = db.getData()
    
    layoutData = {
        nodes: data.nodes,
        edges: data.edges,
        type: "flowchart-v2",
        diagramId: id,
        direction: data.config.direction,
        markers: data.markers,
        config: getConfig(),
        nodeSpacing: config.flowchart.nodeSpacing || 50,
        rankSpacing: config.flowchart.rankSpacing || 50,
    }
    
    await render(layoutData, svg)
    setupViewPortForSVG(svg, {
        top: config.flowchart.diagramPadding,
        bottom: config.flowchart.diagramPadding,
        left: config.flowchart.diagramPadding,
        right: config.flowchart.diagramPadding,
    })
```

---

## Appendix A: Entity Encoding

Some diagram texts contain characters that conflict with HTML/SVG parsing.
Mermaid uses entity encoding to protect these during processing:

```
Encode (before parsing):
    # → ﬂ (U+FB02, ﬂ ligature, used as escape)
    ; → ﬂ;
    
Decode (after SVG generation):
    ﬂ → #
    ﬂ; → ;
```

This prevents characters like `#` and `;` from being interpreted as HTML
entities during DOM manipulation.

## Appendix B: Curve Interpolation Mapping

| Config Name | Curve Type | Description |
|------------|------------|-------------|
| `basis` | B-spline | Smooth curve through control points (default) |
| `basisClosed` | Closed B-spline | Closed smooth curve |
| `basisOpen` | Open B-spline | Open smooth curve |
| `bundle` | Bundle | Hierarchical bundling |
| `cardinal` | Cardinal spline | Smooth curve with tension |
| `cardinalClosed` | Closed cardinal | Closed cardinal curve |
| `cardinalOpen` | Open cardinal | Open cardinal curve |
| `catmullRom` | Catmull-Rom | Centripetal CR spline |
| `catmullRomClosed` | Closed CR | Closed Catmull-Rom |
| `catmullRomOpen` | Open CR | Open Catmull-Rom |
| `linear` | Linear | Straight line segments |
| `linearClosed` | Closed linear | Closed straight line segments |
| `monotoneX` | Monotone X | Monotone in X (no Y overshoot) |
| `monotoneY` | Monotone Y | Monotone in Y (no X overshoot) |
| `natural` | Natural spline | Natural cubic spline |
| `step` | Step | Step function (mid) |
| `stepAfter` | Step after | Step function (after) |
| `stepBefore` | Step before | Step function (before) |

## Appendix C: Configuration Defaults

```
theme:             "default"
securityLevel:     "strict"
maxTextSize:       50000
maxEdges:          500
fontFamily:        "trebuchet ms, verdana, arial, sans-serif"
fontSize:          16
logLevel:          5           // "fatal" only
startOnLoad:       true
arrowMarkerAbsolute: false
deterministicIds:  false
deterministicIDSeed: null

flowchart:
    diagramPadding:    8
    htmlLabels:        true
    nodeSpacing:       50
    rankSpacing:       50
    curve:             "basis"
    padding:           15
    defaultRenderer:   "dagre-wrapper"
    wrappingWidth:     200
```
