# ELK Layout Algorithm Specification

This document is a language-agnostic specification of the Eclipse Layout Kernel (ELK) graph
layout system, derived from a complete reading of the elkjs source code, ELK Java source,
test suite, TypeScript type definitions, and official documentation. It is intended as a
porting reference.

ELK implements multiple graph layout algorithms behind a unified API. Its flagship is a
Sugiyama-style layered layout algorithm with first-class port support. The system is designed
for node-link diagrams with an inherent direction and explicit attachment points (ports) on
node borders.

Key academic references:

- Sugiyama, Tagawa, Toda: "Methods for Visual Understanding of Hierarchical System Structures" (1981)
- Gansner, Koutsofios, North, Vo: "A Technique for Drawing Directed Graphs" (1993)
- Brandes, Köpf: "Fast and Simple Horizontal Coordinate Assignment" (2002)
- Barth, Jünger, Mutzel: "Bilayer Cross Counting" (2002)
- Sander: "Layout of Compound Directed Graphs" (1996)
- Eades, Lin, Smyth: "A fast and effective heuristic for the feedback arc set problem" (1993)
- Forster: "A Fast and Simple Heuristic for Constrained Two-Level Crossing Reduction" (2004)

---

## Table of Contents

1. [Data Model (JSON Format)](#1-data-model-json-format)
2. [Overall Architecture](#2-overall-architecture)
3. [Layout Option Resolution](#3-layout-option-resolution)
4. [Recursive Graph Layout Engine](#4-recursive-graph-layout-engine)
5. [Available Layout Algorithms](#5-available-layout-algorithms)
6. [Layered Algorithm: Internal Graph Representation (LGraph)](#6-layered-algorithm-internal-graph-representation-lgraph)
7. [Layered Algorithm: Five-Phase Pipeline](#7-layered-algorithm-five-phase-pipeline)
8. [Layered Algorithm: Phase 1 — Cycle Breaking](#8-layered-algorithm-phase-1--cycle-breaking)
9. [Layered Algorithm: Phase 2 — Layer Assignment](#9-layered-algorithm-phase-2--layer-assignment)
10. [Layered Algorithm: Phase 3 — Crossing Minimization](#10-layered-algorithm-phase-3--crossing-minimization)
11. [Layered Algorithm: Phase 4 — Node Placement](#11-layered-algorithm-phase-4--node-placement)
12. [Layered Algorithm: Phase 5 — Edge Routing](#12-layered-algorithm-phase-5--edge-routing)
13. [Layered Algorithm: Dummy Node System](#13-layered-algorithm-dummy-node-system)
14. [Layered Algorithm: Intermediate Processor System](#14-layered-algorithm-intermediate-processor-system)
15. [Port System](#15-port-system)
16. [Hierarchical / Compound Graph Handling](#16-hierarchical--compound-graph-handling)
17. [Label Placement](#17-label-placement)
18. [Complete Layout Options Reference](#18-complete-layout-options-reference)
19. [Typical Pipeline Example](#19-typical-pipeline-example)
20. [Comparison: ELK vs Dagre](#20-comparison-elk-vs-dagre)
21. [Porting Priority Recommendations](#21-porting-priority-recommendations)

---

## 1. Data Model (JSON Format)

ELK uses a JSON graph format for input and output. The layout engine reads this graph,
computes positions for all elements, and writes results back to the same structure.

### 1.1 Core Types

```
ElkGraphElement
  id: string                      // unique identifier
  labels: ElkLabel[]              // optional labels
  layoutOptions: {string: string} // key-value layout options

ElkShape extends ElkGraphElement
  x: number                       // x-coordinate (output)
  y: number                       // y-coordinate (output)
  width: number                   // width (input/output)
  height: number                  // height (input/output)

ElkNode extends ElkShape
  id: string                      // REQUIRED
  children: ElkNode[]             // child nodes (hierarchy)
  ports: ElkPort[]                // explicit attachment points
  edges: ElkExtendedEdge[]        // edges contained by this node

ElkPort extends ElkShape
  id: string                      // REQUIRED
  // x, y relative to parent node
  // width, height specify port dimensions

ElkLabel extends ElkShape
  text: string                    // label text

ElkExtendedEdge extends ElkGraphElement
  id: string                      // REQUIRED
  sources: string[]               // source node/port IDs
  targets: string[]               // target node/port IDs
  sections: ElkEdgeSection[]      // edge routing segments (output)
  junctionPoints: ElkPoint[]      // hyperedge junction points

ElkEdgeSection extends ElkGraphElement
  id: string                      // REQUIRED
  startPoint: ElkPoint            // start coordinate {x, y}
  endPoint: ElkPoint              // end coordinate {x, y}
  bendPoints: ElkPoint[]          // intermediate routing points
  incomingShape: string           // source node/port ID
  outgoingShape: string           // target node/port ID
  incomingSections: string[]      // connected incoming section IDs
  outgoingSections: string[]      // connected outgoing section IDs

ElkPoint
  x: number
  y: number
```

### 1.2 Edge Ownership

Edges are stored on the **lowest common ancestor** node that contains both the source and
target. For a flat graph, all edges are on the root node. For hierarchical graphs, an edge
between two nodes in a compound node is stored on that compound node.

### 1.3 Edge Source/Target Resolution

`sources` and `targets` arrays contain string IDs. These can reference:
- A **node ID** — the edge connects to the node (port is implicit)
- A **port ID** — the edge connects to that specific port on a node

When an edge targets a port, the layout engine routes the edge to the port's position on
the parent node's border.

### 1.4 Coordinate System

- All coordinates are relative to the parent node
- The root node's coordinates are relative to (0, 0)
- `x` and `y` refer to the **top-left corner** of the element (unlike dagre which uses center)
- `width` and `height` define the bounding box
- Port positions are relative to their parent node's top-left corner

### 1.5 Input Requirements

The caller must provide:
- `id` for every node, port, edge, and edge section
- `width` and `height` for every node (the layout engine does not compute node sizes)
- `width` and `height` for ports if they have visual extent
- Graph structure (children, edges, ports, sources, targets)

The layout engine computes:
- `x` and `y` for all nodes, ports, and labels
- `width` and `height` for the root graph (bounding box)
- `sections` array for all edges (routing with bend points)

### 1.6 Example Input

```json
{
  "id": "root",
  "layoutOptions": { "elk.algorithm": "layered", "elk.direction": "RIGHT" },
  "children": [
    { "id": "n1", "width": 30, "height": 30 },
    { "id": "n2", "width": 30, "height": 30,
      "ports": [
        { "id": "n2_p1", "width": 8, "height": 8,
          "layoutOptions": { "port.side": "NORTH" }
        }
      ]
    }
  ],
  "edges": [
    { "id": "e1", "sources": ["n1"], "targets": ["n2_p1"] }
  ]
}
```

### 1.7 Example Output

After layout, the same object is returned with computed coordinates:

```json
{
  "id": "root",
  "x": 0, "y": 0, "width": 102, "height": 62,
  "children": [
    { "id": "n1", "x": 12, "y": 16, "width": 30, "height": 30 },
    { "id": "n2", "x": 62, "y": 16, "width": 30, "height": 30,
      "ports": [
        { "id": "n2_p1", "x": 11, "y": -8, "width": 8, "height": 8 }
      ]
    }
  ],
  "edges": [
    { "id": "e1", "sources": ["n1"], "targets": ["n2_p1"],
      "sections": [{
        "id": "e1_s0",
        "startPoint": { "x": 42, "y": 31 },
        "endPoint": { "x": 73, "y": 8 },
        "bendPoints": [{ "x": 52, "y": 31 }, { "x": 52, "y": 8 }]
      }]
    }
  ]
}
```

### 1.8 Legacy Edge Format

An older "primitive" edge format is also accepted:

```json
{
  "id": "e1",
  "source": "n1",
  "sourcePort": "n1_p0",
  "target": "n2",
  "targetPort": "n2_p1"
}
```

This is internally converted to the extended format during import.

---

## 2. Overall Architecture

### 2.1 System Layers

```
JSON Input
    │
    ▼
JsonImporter ── converts JSON to ElkGraph (ElkNode/ElkEdge/ElkPort/ElkLabel)
    │
    ▼
LayoutConfigurator ── applies global layout options to graph elements
    │
    ▼
RecursiveGraphLayoutEngine ── orchestrates layout over hierarchical graphs
    │
    ├── Algorithm 1 (Layered) ── converts ElkGraph to LGraph, runs 5-phase pipeline
    ├── Algorithm 2 (Force) ── force-directed simulation
    ├── Algorithm 3 (Stress) ── stress minimization
    ├── Algorithm 4 (MrTree) ── tree layout
    ├── Algorithm 5 (Radial) ── radial layout
    ├── Algorithm 6 (Rectpacking) ── rectangle packing
    └── Algorithm 7 (SPOrE) ── overlap removal / compaction
    │
    ▼
JsonImporter.transferLayout() ── writes computed positions back to JSON
    │
    ▼
JSON Output (same object, mutated in place)
```

### 2.2 Algorithm Registration

Each algorithm is a `LayoutMetaDataProvider` that registers itself with the central
`LayoutMetaDataService`. Registration provides:
- Algorithm ID (e.g., `org.eclipse.elk.layered`)
- Supported layout options
- Supported graph features
- Algorithm category

Layout options are matched by suffix: `algorithm` matches `org.eclipse.elk.algorithm`,
`elk.algorithm` also matches. The full qualified name is always safe.

### 2.3 Core Options (Always Available)

These options are registered by `CoreOptions` and apply to all algorithms:

| Option | Key | Description |
|--------|-----|-------------|
| Algorithm | `elk.algorithm` | Which layout algorithm to use |
| Direction | `elk.direction` | Primary flow direction |
| Padding | `elk.padding` | Padding inside the graph/compound node |
| Spacing | `elk.spacing.*` | Various spacing options |
| Port constraints | `elk.portConstraints` | How ports are constrained |
| Hierarchy handling | `elk.hierarchyHandling` | How compound graphs are processed |
| Separate components | `elk.separateConnectedComponents` | Layout disconnected components independently |
| Aspect ratio | `elk.aspectRatio` | Target width-to-height ratio |

---

## 3. Layout Option Resolution

### 3.1 Option Targets

Each layout option declares which element types it applies to:
- `NODES` — option applies to nodes
- `PARENTS` — option applies to parent/container nodes
- `EDGES` — option applies to edges
- `PORTS` — option applies to ports
- `LABELS` — option applies to labels

When global options are applied, the configurator sets each option only on elements
matching its declared targets.

### 3.2 Resolution Order (Precedence)

Layout options are resolved in this order (highest to lowest priority):

1. **Element-specific options**: `layoutOptions` on the individual graph element
2. **Global options passed to layout()**: Second argument's `layoutOptions`
3. **Constructor default options**: `defaultLayoutOptions` from ELK constructor

The `NO_OVERWRITE` filter ensures that element-specific options are never overridden by
global options. If a node declares `elk.direction: RIGHT`, a global option
`elk.direction: DOWN` will NOT overwrite it.

### 3.3 Option Suffix Matching

Options can be specified using any unique suffix of the full option ID:

| Full ID | Valid suffixes |
|---------|---------------|
| `org.eclipse.elk.algorithm` | `elk.algorithm`, `algorithm` |
| `org.eclipse.elk.layered.spacing.nodeNodeBetweenLayers` | `elk.layered.spacing.nodeNodeBetweenLayers`, `nodeNodeBetweenLayers` |

If a suffix is ambiguous (matches multiple options), it may be silently ignored. Always
prefix with `elk.` for safety.

### 3.4 Option Value Parsing

Layout option values are always strings in the JSON format. The option's declared type
determines how the string is parsed:

| Type | String format | Example |
|------|--------------|---------|
| Enum | Enum constant name | `"RIGHT"`, `"NETWORK_SIMPLEX"` |
| Boolean | `"true"` / `"false"` | `"true"` |
| Integer | Decimal integer | `"20"` |
| Float | Decimal number | `"0.3"` |
| ElkPadding | `[left=X, top=Y, right=Z, bottom=W]` | `"[left=2, top=3, right=3, bottom=2]"` |
| KVector | `(x, y)` | `"(23, 43)"` |
| KVectorChain | `( {x1,y1}, {x2,y2}, ... )` | `"( {1,2}, {3,4} )"` |
| EnumSet | Space-separated enum values | `"INSIDE V_CENTER H_CENTER"` |

---

## 4. Recursive Graph Layout Engine

The `RecursiveGraphLayoutEngine` handles hierarchical graphs by laying out each level of
the hierarchy.

### 4.1 Algorithm

```
function layoutRecursive(node):
    // 1. Determine the algorithm for this node
    algorithm = resolveAlgorithm(node.layoutOptions["elk.algorithm"])

    // 2. Recursively lay out all children that are compound nodes
    for each child in node.children:
        if child.children is not empty:
            layoutRecursive(child)

    // 3. Lay out this level (the node's direct children and edges)
    algorithm.layout(node)
```

### 4.2 Hierarchy Handling Modes

The behavior depends on `elk.hierarchyHandling`:

**SEPARATE_CHILDREN (default):**
- Bottom-up processing: innermost compound nodes are laid out first
- Each level gets an independent layout run
- After inner layout, the compound node's size is fixed
- Parent layout treats the compound node as an atomic box

**INCLUDE_CHILDREN:**
- The algorithm processes parent and all children in a single run
- Cross-hierarchy edges can be properly routed
- Continues until encountering a descendant with `SEPARATE_CHILDREN`
- Only the layered algorithm fully supports this mode

### 4.3 Connected Component Handling

When `elk.separateConnectedComponents` is true (default):
1. Split the graph into connected components
2. Lay out each component independently
3. Pack components together using a rectangle packing algorithm
4. `elk.aspectRatio` controls the target packing ratio

---

## 5. Available Layout Algorithms

### 5.1 Layered (`org.eclipse.elk.layered`)

The flagship algorithm. Sugiyama-style layer-based layout for directed graphs.
Described in detail in sections 6-19.

Default for all graphs. Best for DAGs, dataflow diagrams, UML, state machines.

### 5.2 Force (`org.eclipse.elk.force`)

Force-directed layout using the Eades spring model or Fruchterman-Reingold model.

Nodes repel each other like charged particles. Edges act as springs pulling connected
nodes together. Iterative simulation until equilibrium.

Key options:
- `elk.force.model`: `EADES` or `FRUCHTERMAN_REINGOLD`
- `elk.force.iterations`: Number of simulation iterations
- `elk.force.repulsion`: Repulsion force constant
- `elk.force.temperature`: Initial temperature (for FR model)

Best for: undirected graphs, social networks, clustering visualization.

### 5.3 Stress (`org.eclipse.elk.stress`)

Stress minimization layout. Minimizes the difference between graph-theoretic distance
(shortest path length) and geometric distance between all pairs of nodes.

Uses iterative majorization (SMACOF algorithm).

Key options:
- `elk.stress.desiredEdgeLength`: Target edge length
- `elk.stress.epsilon`: Convergence threshold

Best for: undirected graphs where edge length should reflect graph distance.

### 5.4 MrTree (`org.eclipse.elk.mrtree`)

Tree layout algorithm. Computes a tree drawing for graphs that are trees or forests.

Based on Walker's algorithm with Buchheim/Jünger/Leipert improvements for linear time.

Key options:
- `elk.direction`: Tree growth direction
- `elk.spacing.nodeNode`: Sibling spacing

Best for: hierarchies, file systems, org charts.

### 5.5 Radial (`org.eclipse.elk.radial`)

Places nodes on concentric circles (rings) around a central root node.

Key options:
- `elk.radial.centerOnRoot`: Center layout on root
- `elk.radial.orderId`: Node ordering strategy
- `elk.radial.radius`: Minimum radius

Best for: centered hierarchies, dependency rings.

### 5.6 Rectangle Packing (`org.eclipse.elk.rectpacking`)

Packs rectangular nodes into a compact arrangement without overlaps.
No edges are considered.

Key options:
- `elk.aspectRatio`: Target aspect ratio

Best for: dashboards, icon grids, disconnected component packing.

### 5.7 SPOrE (`org.eclipse.elk.sporeOverlap`, `org.eclipse.elk.sporeCompaction`)

Scanning, Packing, Overlap Removal, and Expansion. Two variants:
- **sporeOverlap**: Remove node overlaps while preserving relative positions
- **sporeCompaction**: Compact node positions to reduce whitespace

Best for: post-processing other layouts, overlap removal.

### 5.8 Built-in Simple Algorithms (Always Available)

These are always registered regardless of `algorithms` array:

- **Box (`org.eclipse.elk.box`)**: Packs nodes left-to-right, wrapping rows
- **Fixed (`org.eclipse.elk.fixed`)**: Keeps nodes at their input positions. Useful for
  applying edge routing to a pre-laid-out graph
- **Random (`org.eclipse.elk.random`)**: Random node placement

---

## 6. Layered Algorithm: Internal Graph Representation (LGraph)

Before the layered algorithm runs, the input ElkGraph is converted into an internal
representation optimized for the algorithm. After layout, results are transferred back.

### 6.1 Core Types

| Type | Description |
|------|-------------|
| **LGraph** | The internal graph. Contains a list of `Layer` objects and `layerlessNodes` (not yet assigned). Has `size` (width, height), `padding`, `offset`. May have a `parentNode` for nested graphs. |
| **Layer** | A column of nodes. Created by the layering phase. Contains an ordered list of `LNode` objects. Has an index, size, and position. |
| **LNode** | A node. Has `type` (NodeType), `ports` (list), `labels` (list), `margin`, `padding`, `layer` reference. Ports filterable by `PortType` (INPUT/OUTPUT) and `PortSide` (N/S/E/W). Optional `nestedGraph` for compound nodes. |
| **LPort** | A port on a node border. Has `incomingEdges`, `outgoingEdges`, `side` (PortSide), `type` (PortType), position, size. Has `anchor` point for edge attachment within the port's bounds. |
| **LEdge** | An edge between two LPort instances. Has `bendPoints` (list of coordinate pairs), `junctionPoints`, `labels`, `source` (LPort), `target` (LPort). |
| **LLabel** | A label with position, size, text. |

### 6.2 LNode.NodeType Enum

Classifies every node in the internal graph:

| Value | Description |
|-------|-------------|
| `NORMAL` | An actual node from the original input graph |
| `LONG_EDGE` | Dummy splitting a long edge spanning multiple layers |
| `EXTERNAL_PORT` | Dummy representing a hierarchical port on the compound node boundary |
| `NORTH_SOUTH_PORT` | Dummy handling ports on north or south side of a node |
| `LABEL` | Dummy reserving space for a center edge label |
| `BREAKING_POINT` | Dummy for graph wrapping (breaking long graphs into rows) |
| `PLACEHOLDER` | Space reservation when unzipping layers |
| `NONSHIFTING_PLACEHOLDER` | Fixed-position placeholder during unzipping |

### 6.3 GraphProperties Flags

These boolean flags are set on the LGraph during import. They drive which intermediate
processors are activated:

| Flag | Meaning |
|------|---------|
| `COMMENTS` | Graph contains comment box nodes |
| `EXTERNAL_PORTS` | Graph has ports on the outer compound node boundary |
| `HYPEREDGES` | Graph has edges with multiple sources or targets |
| `HYPERNODES` | Graph has nodes that are hypernode containers |
| `NON_FREE_PORTS` | Some ports have constraints (not FREE) |
| `NORTH_SOUTH_PORTS` | Some ports are on the north or south side |
| `SELF_LOOPS` | Some edges have the same source and target node |
| `CENTER_LABELS` | Some edges have center-positioned labels |
| `END_LABELS` | Some edges have head or tail labels |
| `PARTITIONS` | Nodes are partitioned into groups |

### 6.4 ElkGraph to LGraph Conversion

The import process:
1. Create an LNode for each ElkNode child
2. Create an LPort for each ElkPort on each node
3. Create an LEdge for each ElkEdge, connecting the corresponding LPorts
4. Set GraphProperties flags based on detected features
5. Store the original ElkNode/ElkEdge/ElkPort references for later export

The export process:
1. Write computed x, y coordinates from LNode back to ElkNode
2. Write computed port positions from LPort back to ElkPort
3. Convert LEdge bend points into ElkEdgeSection objects
4. Compute graph bounding box (width, height)

---

## 7. Layered Algorithm: Five-Phase Pipeline

The algorithm is structured as five sequential main phases with intermediate processor
slots before, between, and after each phase:

```
BEFORE_P1 → P1 → BEFORE_P2 → P2 → BEFORE_P3 → P3 → BEFORE_P4 → P4 → BEFORE_P5 → P5 → AFTER_P5
```

| Phase | Enum | Purpose |
|-------|------|---------|
| P1 | `P1_CYCLE_BREAKING` | Make the graph acyclic by reversing edges |
| P2 | `P2_LAYERING` | Assign each node to a layer (column) |
| P3 | `P3_NODE_ORDERING` | Order nodes within each layer to minimize edge crossings |
| P4 | `P4_NODE_PLACEMENT` | Assign y-coordinates to nodes within their layers |
| P5 | `P5_EDGE_ROUTING` | Route edges with bend points and assign x-coordinates |

Each slot can hold zero or more intermediate processors. The exact set depends on graph
properties and configuration. A simple graph typically has ~17 total processors; complex
graphs with ports, labels, and compound structure can have 30+.

### 7.1 Coordinate Conventions

The algorithm works in a **left-to-right** (LTR) coordinate system internally:
- Layers are columns arranged left to right
- Nodes within a layer are ordered top to bottom
- x-axis = inter-layer direction (horizontal)
- y-axis = intra-layer direction (vertical)

For other directions (DOWN, LEFT, UP), the `DIRECTION_PREPROCESSOR` transforms the graph
to LTR before processing, and `DIRECTION_POSTPROCESSOR` transforms back after.

| Input Direction | Internal Mapping |
|----------------|-----------------|
| `RIGHT` (default) | No transformation needed |
| `LEFT` | Mirror horizontally |
| `DOWN` | Swap x↔y axes |
| `UP` | Swap x↔y axes + mirror |

---

## 8. Layered Algorithm: Phase 1 — Cycle Breaking

Makes the graph acyclic by identifying and reversing back-edges. Reversed edges are
tagged with a property and restored to their original direction after layout completes
(by `REVERSED_EDGE_RESTORER` in AFTER_P5).

### 8.1 Strategies

| Strategy | Algorithm |
|----------|-----------|
| `GREEDY` (default) | Greedy heuristic by Eades/Lin/Smyth (1993). Computes in-degree and out-degree for each node. Iteratively extracts sources (out-degree > in-degree) to the front and sinks to the back. Edges going against the computed order are reversed. Minimizes total reversed edges. |
| `DEPTH_FIRST` | DFS traversal. Reverses back-edges (edges pointing to an ancestor in the DFS tree). Uses port/edge iteration order for determinism. |
| `INTERACTIVE` | Uses existing node positions from input to determine edge direction. Edges pointing "upstream" (against the layout direction) are reversed. |
| `MODEL_ORDER` | Uses the order nodes appear in the input model. Edges from later-ordered nodes to earlier-ordered nodes are reversed. |
| `GREEDY_MODEL_ORDER` | Greedy heuristic with model order as tie-breaker. |

### 8.2 Greedy Cycle Breaker Detail

```
function greedyCycleBreak(nodes, edges):
    // Partition nodes into sources (SL) and sinks (SR)
    SL = []  // left sequence (sources first)
    SR = []  // right sequence (sinks first)
    remaining = copy(nodes)

    while remaining is not empty:
        // Remove sinks (nodes with no outgoing edges in remaining)
        while exists node in remaining with out_degree(node) == 0:
            SR.prepend(node)
            remaining.remove(node)
            update degrees of neighbors

        // Remove sources (nodes with no incoming edges in remaining)
        while exists node in remaining with in_degree(node) == 0:
            SL.append(node)
            remaining.remove(node)
            update degrees of neighbors

        // Pick node with max (out_degree - in_degree) from remaining
        if remaining is not empty:
            best = argmax(node in remaining, out_degree(node) - in_degree(node))
            SL.append(best)
            remaining.remove(best)
            update degrees of neighbors

    // Final order: SL + SR
    order = SL + SR

    // Reverse edges that go against this order
    for each edge (u, v):
        if order.indexOf(u) > order.indexOf(v):
            reverse(edge)
            edge.reversed = true
```

### 8.3 Reversed Edge Restoration

At the end of layout (AFTER_P5), `REVERSED_EDGE_RESTORER` processes all edges marked as
reversed:
1. Swap source and target ports
2. Reverse the list of bend points
3. Reverse junction points
4. Swap labels marked as head/tail

---

## 9. Layered Algorithm: Phase 2 — Layer Assignment

Assigns each node to a layer (column). The goal is to minimize total edge length while
respecting constraints. After layering, all edges ideally span exactly one layer (achieved
by inserting dummy nodes for long edges in a later step).

### 9.1 Strategies

| Strategy | Algorithm |
|----------|-----------|
| `NETWORK_SIMPLEX` (default) | Optimal layering minimizing total edge length via the network simplex algorithm (Gansner et al. 1993). Processes connected components separately. |
| `LONGEST_PATH` | Greedy: assigns sink nodes to layer 0, then assigns each remaining node to (1 + max layer of successors). Simple, O(V+E), but not optimal. |
| `LONGEST_PATH_SOURCE` | Same as longest path but measured from sources instead of sinks. |
| `COFFMAN_GRAHAM` | Restricts maximum number of original nodes per layer. Configurable `layerBound`. Solves a precedence-constrained scheduling problem. |
| `INTERACTIVE` | Uses previous node positions from input to determine layers. |
| `STRETCH_WIDTH` | (Experimental) Reduces max nodes per layer. |
| `MIN_WIDTH` | (Experimental) Minimizes layer width considering dummy nodes. |
| `BF_MODEL_ORDER` | (Experimental) Breadth-first model order. |
| `DF_MODEL_ORDER` | (Experimental) Depth-first model order. |

### 9.2 Network Simplex Layering

The network simplex method finds the optimal layer assignment that minimizes `sum(length(e) * weight(e))` for all edges, where `length(e) = layer(target) - layer(source)` and `weight(e)` defaults to 1.

```
function networkSimplexLayering(graph):
    // 1. Initial feasible ranking using longest-path from sinks
    feasibleTree(graph)

    // 2. Construct a spanning tree of the graph
    tree = initSpanningTree(graph)

    // 3. Iteratively improve by pivoting
    while exists edge e not in tree with negative cut value:
        // Find the entering edge (non-tree edge with negative cut value)
        enterEdge = selectEnteringEdge(tree, graph)
        if enterEdge is null:
            break  // optimal

        // Find the leaving edge (tree edge that becomes redundant)
        leaveEdge = selectLeavingEdge(tree, enterEdge)

        // Pivot: swap entering and leaving edges in the tree
        exchange(tree, enterEdge, leaveEdge)

        // Recompute cut values for affected edges
        updateCutValues(tree)

    // 4. Normalize: shift so minimum layer = 0
    normalize(graph)
```

**Cut value** of a tree edge: the number of edges going from the source-side component
to the target-side component minus edges going the other way, weighted. A negative cut
value means the tree edge is not optimal and should be replaced.

### 9.3 Longest Path Layering

```
function longestPathLayering(graph):
    layered = set()

    function visit(node):
        if node in layered:
            return node.layer

        maxSuccessorLayer = -1
        for each outgoing edge (node, target):
            targetLayer = visit(target)
            maxSuccessorLayer = max(maxSuccessorLayer, targetLayer)

        node.layer = maxSuccessorLayer + 1
        layered.add(node)
        return node.layer

    // Start from all nodes (sinks will get layer 0)
    for each node in graph:
        visit(node)

    // Normalize so minimum layer = 0
    normalize(graph)
```

### 9.4 Layer Constraints

The `elk.layered.layering.layerConstraint` option can force a node to a specific layer:
- `NONE` — no constraint
- `FIRST` — node must be in the first layer
- `FIRST_SEPARATE` — node in a layer before the first layer
- `LAST` — node must be in the last layer
- `LAST_SEPARATE` — node in a layer after the last layer

Processed by `LAYER_CONSTRAINT_PREPROCESSOR` and `LAYER_CONSTRAINT_POSTPROCESSOR`.

### 9.5 Post-Layering

**Node Promotion** (`elk.layered.layering.nodePromotion.strategy`): After initial
layering, nodes may be "promoted" to earlier layers to reduce the number of dummy
nodes needed for long edges.

---

## 10. Layered Algorithm: Phase 3 — Crossing Minimization

Determines the vertical ordering of nodes within each layer to minimize edge crossings.

### 10.1 Strategies

| Strategy | Algorithm |
|----------|-----------|
| `LAYER_SWEEP` (default) | Layer-by-layer sweep using barycenter heuristic |
| `MEDIAN_LAYER_SWEEP` | Same sweep but using median of neighbor positions |
| `INTERACTIVE` | Uses previous positions from input |
| `NONE` | No crossing minimization |

### 10.2 Layer Sweep Barycenter Algorithm

```
function layerSweepCrossingMin(layers, thoroughness):
    bestCrossings = infinity
    bestOrdering = null

    repeat thoroughness times:  // default thoroughness = 7
        // Forward sweep (left to right)
        for i = 1 to layers.length - 1:
            reorderLayer(layers[i], layers[i-1], direction=LEFT_TO_RIGHT)

        // Backward sweep (right to left)
        for i = layers.length - 2 downto 0:
            reorderLayer(layers[i], layers[i+1], direction=RIGHT_TO_LEFT)

        // Count crossings
        crossings = countAllCrossings(layers)
        if crossings < bestCrossings:
            bestCrossings = crossings
            bestOrdering = copyOrdering(layers)

    applyOrdering(layers, bestOrdering)

function reorderLayer(layer, fixedLayer, direction):
    for each node in layer:
        // Compute barycenter: average position of connected nodes in fixedLayer
        connectedPositions = []
        for each edge connecting node to fixedLayer:
            neighbor = edge.otherEnd(node)
            connectedPositions.append(neighbor.positionInLayer)

        if connectedPositions is empty:
            node.barycenter = null  // unconstrained
        else:
            node.barycenter = average(connectedPositions)

    // Sort layer by barycenter (nodes without connections keep relative position)
    sort(layer, by=barycenter, stableSort=true)
```

### 10.3 Crossing Counting

Edge crossings between two adjacent layers can be counted efficiently using the
**accumulator tree** (Barth, Jünger, Mutzel 2002) in O(|E| log |V|) time:

```
function countCrossings(leftLayer, rightLayer, edges):
    // Build a sorted order of edges by position of left endpoint
    // For each edge, count how many previously processed edges
    // have a right endpoint below the current edge's right endpoint
    // This count equals the number of crossings involving this edge
    // Total: sum of all such counts

    // Implementation uses a binary indexed tree (Fenwick tree)
    // or merge-sort based inversion counting
```

### 10.4 Port-Aware Crossing Minimization

When ports have constraints, crossing minimization is more complex:

- **FIXED_ORDER ports**: The order of ports on each side is fixed. Node ordering
  must respect the constraint that edges to/from port i must not cross edges
  to/from port j when i < j (for the fixed side).
- **FIXED_SIDE ports**: Ports on different sides create different crossing
  considerations. North/south ports are handled by dedicated dummy nodes.

### 10.5 Greedy Switch Post-Processing

After the main crossing minimization, a greedy local search further reduces crossings:

```
function greedySwitch(layers, type):
    improved = true
    while improved:
        improved = false
        for each layer:
            for each adjacent pair (node_i, node_i+1) in layer:
                crossingsBefore = countLocalCrossings(node_i, node_i+1)
                swap(node_i, node_i+1)
                crossingsAfter = countLocalCrossings(node_i, node_i+1)
                if crossingsAfter >= crossingsBefore:
                    swap(node_i, node_i+1)  // undo
                else:
                    improved = true
```

Types:
- `ONE_SIDED`: Only considers crossings with one adjacent layer per swap
- `TWO_SIDED` (default): Considers crossings with both adjacent layers
- `OFF`: Disabled

Activation threshold: greedy switch is only applied if the graph has fewer than
`greedySwitch.activationThreshold` (default 40) nodes per layer. Above this,
the cost of counting crossings per swap is too high.

### 10.6 In-Layer Constraints

After crossing minimization, the `IN_LAYER_CONSTRAINT_PROCESSOR` enforces positional
constraints within each layer:
- Nodes marked `TOP` are moved to the top of their layer
- Nodes marked `BOTTOM` are moved to the bottom

---

## 11. Layered Algorithm: Phase 4 — Node Placement

Computes the y-coordinates of nodes within their layers (and refines x positioning).

### 11.1 Strategies

| Strategy | Algorithm |
|----------|-----------|
| `BRANDES_KOEPF` (default) | Block alignment with four-directional balancing |
| `LINEAR_SEGMENTS` | Pendulum method aligning long-edge chains |
| `NETWORK_SIMPLEX` | Auxiliary graph with network simplex for balanced placement |
| `SIMPLE` | Centers all nodes vertically; very fast |
| `INTERACTIVE` | Preserves y-coordinates from input |

### 11.2 Brandes-Köpf Algorithm Detail

This is the default and most sophisticated placement strategy. It produces compact
layouts with many straight edges.

```
function brandesKoepfPlacement(layers):
    // 1. Mark type-1 conflicts (crossings between inner segments)
    //    Inner segments = edges between two long-edge dummy nodes
    conflicts = markType1Conflicts(layers)

    // 2. Compute four extreme alignments
    placements = []
    for vertDir in [UP, DOWN]:
        for horizDir in [LEFT, RIGHT]:
            // a. Vertical alignment: group nodes into blocks
            blocks = verticalAlignment(layers, vertDir, horizDir, conflicts)

            // b. Horizontal compaction: assign x-coordinates to blocks
            placement = horizontalCompaction(blocks, horizDir)

            placements.append(placement)

    // 3. Balance: for each node, take the median of its 4 placement values
    for each node:
        values = [placements[0][node], placements[1][node],
                  placements[2][node], placements[3][node]]
        sort(values)
        node.y = (values[1] + values[2]) / 2  // median of middle two
```

**Vertical Alignment** builds chains of vertically aligned nodes:

```
function verticalAlignment(layers, vertDir, horizDir, conflicts):
    root = {}   // root[node] = root of node's block
    align = {}  // align[node] = next node in alignment chain

    // Initialize: each node is its own root and alignment
    for each node:
        root[node] = node
        align[node] = node

    // Process layers in vertDir order, nodes in horizDir order
    for each layer (in vertDir order):
        for each node (in horizDir order):
            // Find median incoming edge (from previous layer in vertDir)
            medianEdge = findMedianEdge(node, vertDir)
            if medianEdge is not null:
                neighbor = medianEdge.otherEnd
                if not isConflict(medianEdge, conflicts):
                    if align[neighbor] == neighbor:  // not yet aligned
                        align[neighbor] = node
                        root[node] = root[neighbor]
                        align[node] = root[node]
```

**Horizontal Compaction** assigns coordinates to blocks:

```
function horizontalCompaction(blocks, horizDir):
    // Process blocks in topological order
    // Each block's position = max(predecessor position + spacing)
    // Compact by pushing blocks as close together as possible
    // while respecting minimum spacing constraints
```

### 11.3 Linear Segments Algorithm

Groups long-edge dummy chains into "linear segments" that should be drawn as
straight lines. Uses the pendulum method (Sander 1996):

```
function linearSegments(layers):
    // 1. Identify linear segments (chains of LONG_EDGE dummies)
    segments = identifySegments(layers)

    // 2. Initial placement: place each segment at the barycenter
    //    of its connected neighbors
    for each segment:
        segment.y = barycenter(neighbors(segment))

    // 3. Iterate: pendulum relaxation
    //    Each segment "swings" toward the average of its neighbors
    repeat until converged:
        for each segment:
            target = weightedAverage(neighbors(segment))
            segment.y += deflectionDampening * (target - segment.y)
```

### 11.4 Size Computation (Pre-P4)

Before node placement, the `LABEL_AND_NODE_SIZE_PROCESSOR` computes final node sizes by:
1. Computing label sizes and positions
2. Adding port label space
3. Applying minimum size constraints
4. Computing node margins (extra space around nodes for labels, ports, etc.)

The `INNERMOST_NODE_MARGIN_CALCULATOR` computes the margin around the innermost level of
compound nodes.

---

## 12. Layered Algorithm: Phase 5 — Edge Routing

Determines the final edge routes (bend points) and assigns definitive x-coordinates to
layers, establishing inter-layer spacing.

### 12.1 Edge Routing Styles

| Style | Description |
|-------|-------------|
| `ORTHOGONAL` (default) | Edges use only horizontal and vertical segments. Creates a clean, block-diagram appearance. Most complex to route but most readable. |
| `POLYLINE` | Straight-line segments between bend points. Simpler routing with fewer bends but lines may be diagonal. |
| `SPLINES` | Smooth cubic Bezier curves. Bend points become control points. Aesthetically pleasing but harder to follow for complex graphs. |
| `UNDEFINED` | No specific style enforced. |

### 12.2 Orthogonal Edge Routing Algorithm

```
function orthogonalEdgeRouting(layers):
    for each pair of adjacent layers (L_i, L_i+1):
        // 1. Compute inter-layer gap
        gap = max(
            minSpacingNodeNodeBetweenLayers,
            spaceNeededForEdgesBetween(L_i, L_i+1)
        )

        // 2. Position L_i+1 relative to L_i
        L_i+1.x = L_i.x + L_i.width + gap

        // 3. Route edges between L_i and L_i+1
        for each edge from L_i to L_i+1:
            sourcePort = edge.sourcePort
            targetPort = edge.targetPort

            // Create orthogonal route:
            // - Exit source port horizontally
            // - Vertical segment in the inter-layer gap
            // - Enter target port horizontally
            edge.bendPoints = computeOrthogonalRoute(
                sourcePort.absolutePosition(),
                targetPort.absolutePosition(),
                gap
            )

    // 4. Handle self-loops, north/south port edges
    routeSelfLoops()
    routeNorthSouthEdges()
```

Key spacing options for edge routing:
- `elk.spacing.edgeEdge`: Minimum distance between parallel edges in the routing channel
- `elk.spacing.edgeNode`: Minimum distance between edges and node borders
- `elk.layered.spacing.edgeNodeBetweenLayers`: Edge-node spacing in the inter-layer gap
- `elk.layered.spacing.edgeEdgeBetweenLayers`: Edge-edge spacing in the inter-layer gap

### 12.3 Spline Edge Routing

Two modes controlled by `elk.layered.edgeRouting.splines.mode`:

**SLOPPY (default):** Fewer control points, produces curvier routes. May overlap nodes
in tight layouts. Faster computation.

**CONSERVATIVE:** More control points, properly routes around nodes. More orthogonal
feel. Respects node boundaries strictly.

After initial routing, the `FINAL_SPLINE_BENDPOINTS_CALCULATOR` converts intermediate
control points into final cubic Bezier control points.

### 12.4 Layer Size Computation (Pre-P5)

Before edge routing, `LAYER_SIZE_AND_GRAPH_HEIGHT_CALCULATOR`:
1. Computes width and height of each layer based on contained nodes
2. Determines overall graph height
3. Sets y-offsets for layers based on node positions

---

## 13. Layered Algorithm: Dummy Node System

The algorithm introduces several types of dummy nodes to normalize the graph. All dummy
nodes are removed or resolved before producing final output.

### 13.1 Long Edge Dummies (`LONG_EDGE`)

**Purpose:** Enforce proper layering — all edges must span exactly one layer.

**Insertion** (by `LONG_EDGE_SPLITTER` in BEFORE_P3):

```
function splitLongEdges(layers):
    for each edge (source, target) where layer(target) - layer(source) > 1:
        currentSource = source
        for layerIndex = layer(source) + 1 to layer(target) - 1:
            // Create dummy node in this layer
            dummy = createDummyNode(type=LONG_EDGE)
            layers[layerIndex].add(dummy)

            // Dummy has two ports: WEST (input) and EAST (output)
            inPort = createPort(side=WEST, type=INPUT, constraint=FIXED_POS)
            outPort = createPort(side=EAST, type=OUTPUT, constraint=FIXED_POS)
            dummy.addPort(inPort)
            dummy.addPort(outPort)

            // Redirect edge
            redirect(currentEdge, target=inPort)

            // Create new edge for next segment
            currentEdge = createEdge(source=outPort, target=...)
            currentSource = dummy

        // Final segment connects to original target
        redirect(currentEdge, target=originalTarget)

        // Store original endpoints for later reconstruction
        dummy.properties[LONG_EDGE_SOURCE] = originalSource
        dummy.properties[LONG_EDGE_TARGET] = originalTarget
```

**Removal** (by `LONG_EDGE_JOINER` in AFTER_P5):
1. Trace the chain of dummy nodes from source to target
2. Collect all bend points from intermediate edges
3. Create a single edge from original source to original target
4. Remove all dummy nodes from their layers

### 13.2 North/South Port Dummies (`NORTH_SOUTH_PORT`)

**Purpose:** Handle edges connected to ports on the north or south side of a node.
In a left-to-right layout, east/west ports are naturally handled. North/south ports
need special treatment because their edges go perpendicular to the flow direction.

**Insertion** (by `NORTH_SOUTH_PORT_PREPROCESSOR` in BEFORE_P3):

```
function createNorthSouthDummies(layers):
    for each node with north/south ports:
        for each north port with edges:
            dummy = createDummyNode(type=NORTH_SOUTH_PORT)
            // Place dummy in the SAME layer as the node
            // Dummy is constrained to appear ABOVE the node (for north ports)
            dummy.inLayerConstraint = TOP
            layers[node.layer].add(dummy)

            // Reroute edges through dummy
            for each edge on the port:
                reroute(edge, through=dummy)

        for each south port with edges:
            dummy = createDummyNode(type=NORTH_SOUTH_PORT)
            dummy.inLayerConstraint = BOTTOM
            layers[node.layer].add(dummy)
            // reroute edges...
```

### 13.3 External Port Dummies (`EXTERNAL_PORT`)

**Purpose:** Represent ports on the boundary of a compound node that connect the inner
graph to the outer graph.

For `SEPARATE_CHILDREN` hierarchy handling:
- East/west external ports become dummy nodes in the first/last layer
- North/south external ports become dummy nodes with special handling

For `INCLUDE_CHILDREN` hierarchy handling:
- Cross-hierarchy edges pass through external port dummies
- The `HIERARCHICAL_PORT_CONSTRAINT_PROCESSOR` manages port ordering constraints

### 13.4 Label Dummies (`LABEL`)

**Purpose:** Reserve space for center edge labels.

Created by `LABEL_DUMMY_INSERTER`. For each edge with a center label, a dummy node is
inserted in the layer where the label should appear. The dummy's size equals the label's
size. It participates in crossing minimization and node placement to ensure labels don't
overlap. Removed by `LABEL_DUMMY_REMOVER`.

### 13.5 Breaking Point Dummies (`BREAKING_POINT`)

**Purpose:** Support graph wrapping — breaking a single long row of layers into multiple
rows to fit a target aspect ratio.

Created by `BREAKING_POINT_INSERTER` when `elk.layered.wrapping.strategy == MULTI_EDGE`.
Splitting points are computed based on the target aspect ratio. Removed by
`BREAKING_POINT_REMOVER` after edge routing.

---

## 14. Layered Algorithm: Intermediate Processor System

### 14.1 Architecture

Each main phase declares which intermediate processors it needs via
`getLayoutProcessorConfiguration(graph)`. The `GraphConfigurator` collects requirements
from:
1. The chosen phase strategy implementations
2. The graph's properties (GraphProperties flags)
3. Global algorithm configuration options

The `AlgorithmAssembler` then builds the final linear pipeline by sorting processors
within each slot.

### 14.2 Slots

There are 11 processing slots:

```
BEFORE_PHASE_1
PHASE_1 (cycle breaking)
BEFORE_PHASE_2
PHASE_2 (layering)
BEFORE_PHASE_3
PHASE_3 (crossing minimization)
BEFORE_PHASE_4
PHASE_4 (node placement)
BEFORE_PHASE_5
PHASE_5 (edge routing)
AFTER_PHASE_5
```

### 14.3 Baseline Processors (Always Active)

These four processors run on every graph regardless of configuration:

| Processor | Slot | Purpose |
|-----------|------|---------|
| `INNERMOST_NODE_MARGIN_CALCULATOR` | BEFORE_P4 | Compute margins for innermost compound nodes |
| `LABEL_AND_NODE_SIZE_PROCESSOR` | BEFORE_P4 | Compute final node sizes including labels and ports |
| `LAYER_SIZE_AND_GRAPH_HEIGHT_CALCULATOR` | BEFORE_P5 | Compute layer dimensions and graph height |
| `END_LABEL_SORTER` | AFTER_P5 | Sort end labels for consistent ordering |

### 14.4 Complete Intermediate Processor List

All 58 processors, their implementing classes, and when they activate:

| # | Processor | Purpose | Slot | Condition |
|---|-----------|---------|------|-----------|
| 1 | `DIRECTION_PREPROCESSOR` | Transform graph to LTR coordinates | BEFORE_P1 | Direction ≠ RIGHT |
| 2 | `COMMENT_PREPROCESSOR` | Remove comment boxes, attach to neighbors | BEFORE_P1 | COMMENTS flag |
| 3 | `EDGE_AND_LAYER_CONSTRAINT_EDGE_REVERSER` | Reverse edges conflicting with layer constraints | BEFORE_P2 | Always with network simplex |
| 4 | `INTERACTIVE_EXTERNAL_PORT_POSITIONER` | Position external ports from input | BEFORE_P2 | Interactive mode |
| 5 | `PARTITION_PREPROCESSOR` | Set up partition constraints | BEFORE_P1 | PARTITIONS flag |
| 6 | `LABEL_DUMMY_INSERTER` | Create dummy nodes for center labels | BEFORE_P3 | CENTER_LABELS flag |
| 7 | `SELF_LOOP_PREPROCESSOR` | Remove self-loops before main processing | BEFORE_P1 | SELF_LOOPS flag |
| 8 | `LAYER_CONSTRAINT_PREPROCESSOR` | Process FIRST/LAST layer constraints | BEFORE_P2 | Always with network simplex |
| 9 | `PARTITION_MIDPROCESSOR` | Enforce partition ordering between layers | BEFORE_P2 | PARTITIONS flag |
| 10 | `HIGH_DEGREE_NODE_LAYER_PROCESSOR` | Spread high-degree node connections | BEFORE_P3 | High degree enabled |
| 11 | `NODE_PROMOTION` | Promote nodes to reduce dummies | BEFORE_P3 | Promotion ≠ NONE |
| 12 | `LAYER_CONSTRAINT_POSTPROCESSOR` | Finalize layer constraint positions | BEFORE_P3 | Always with network simplex |
| 13 | `PARTITION_POSTPROCESSOR` | Finalize partition constraints | BEFORE_P3 | PARTITIONS flag |
| 14 | `HIERARCHICAL_PORT_CONSTRAINT_PROCESSOR` | Handle external port ordering | BEFORE_P3 | EXTERNAL_PORTS flag |
| 15 | `SEMI_INTERACTIVE_CROSSMIN_PROCESSOR` | Lock certain node positions | BEFORE_P3 | Semi-interactive enabled |
| 16 | `BREAKING_POINT_INSERTER` | Insert wrapping break points | BEFORE_P3 | Wrapping = MULTI_EDGE |
| 17 | `LONG_EDGE_SPLITTER` | Split long edges with dummies | BEFORE_P3 | Always |
| 18 | `PORT_SIDE_PROCESSOR` | Assign port sides based on edge directions | BEFORE_P1 or BEFORE_P3 | Always (slot depends on feedback edges) |
| 19 | `INVERTED_PORT_PROCESSOR` | Handle ports facing "wrong" direction | BEFORE_P3 | NON_FREE_PORTS or feedback edges |
| 20 | `PORT_LIST_SORTER` | Sort port lists by position/constraint | BEFORE_P3 | Always with crossing min |
| 21 | `SORT_BY_INPUT_ORDER_OF_MODEL` | Preserve input model order | BEFORE_P3 | Model order ≠ NONE |
| 22 | `NORTH_SOUTH_PORT_PREPROCESSOR` | Create N/S port dummies | BEFORE_P3 | NORTH_SOUTH_PORTS flag |
| 23 | `BREAKING_POINT_PROCESSOR` | Process wrapping break points | BEFORE_P4 | Wrapping = MULTI_EDGE |
| 24 | `ONE_SIDED_GREEDY_SWITCH` | One-directional greedy swap | BEFORE_P4 | Greedy switch type = ONE_SIDED |
| 25 | `TWO_SIDED_GREEDY_SWITCH` | Two-directional greedy swap | BEFORE_P4 | Greedy switch type = TWO_SIDED |
| 26 | `SELF_LOOP_PORT_RESTORER` | Restore self-loop port positions | BEFORE_P4 | SELF_LOOPS flag |
| 27 | `ALTERNATING_LAYER_UNZIPPER` | Unzip layers for better aspect ratio | BEFORE_P4 | Unzipping = ALTERNATING |
| 28 | `SINGLE_EDGE_GRAPH_WRAPPER` | Wrap single-edge chains | BEFORE_P4 | Wrapping = SINGLE_EDGE |
| 29 | `IN_LAYER_CONSTRAINT_PROCESSOR` | Enforce TOP/BOTTOM constraints | BEFORE_P4 | Always with crossing min |
| 30 | `END_NODE_PORT_LABEL_MANAGEMENT_PROCESSOR` | Manage end-node port labels | BEFORE_P4 | Label manager exists |
| 31 | `LABEL_AND_NODE_SIZE_PROCESSOR` | Compute node sizes with labels | BEFORE_P4 | **Always** |
| 32 | `INNERMOST_NODE_MARGIN_CALCULATOR` | Compute innermost margins | BEFORE_P4 | **Always** |
| 33 | `SELF_LOOP_ROUTER` | Route self-loop edges | BEFORE_P4 | SELF_LOOPS flag |
| 34 | `COMMENT_NODE_MARGIN_CALCULATOR` | Adjust margins for comments | BEFORE_P4 | COMMENTS flag |
| 35 | `END_LABEL_PREPROCESSOR` | Process head/tail edge labels | BEFORE_P4 | END_LABELS flag |
| 36 | `LABEL_DUMMY_SWITCHER` | Adjust label dummy positions | BEFORE_P4 | CENTER_LABELS flag |
| 37 | `CENTER_LABEL_MANAGEMENT_PROCESSOR` | Manage center label sizes | BEFORE_P4 | Label manager + CENTER_LABELS |
| 38 | `LABEL_SIDE_SELECTOR` | Choose label sides (above/below edge) | BEFORE_P4 | CENTER_LABELS or END_LABELS |
| 39 | `HYPEREDGE_DUMMY_MERGER` | Merge dummies for hyperedges | BEFORE_P4 | HYPEREDGES flag |
| 40 | `HIERARCHICAL_PORT_DUMMY_SIZE_PROCESSOR` | Size external port dummies | BEFORE_P5 | EXTERNAL_PORTS flag |
| 41 | `LAYER_SIZE_AND_GRAPH_HEIGHT_CALCULATOR` | Compute layer/graph dimensions | BEFORE_P5 | **Always** |
| 42 | `HIERARCHICAL_PORT_POSITION_PROCESSOR` | Position hierarchical ports | AFTER_P5 | EXTERNAL_PORTS flag |
| 43 | `CONSTRAINTS_POSTPROCESSOR` | Write constraint data back | AFTER_P5 | Interactive or generate IDs |
| 44 | `COMMENT_POSTPROCESSOR` | Restore comment boxes | AFTER_P5 | COMMENTS flag |
| 45 | `HYPERNODE_PROCESSOR` | Handle hypernode containers | AFTER_P5 | HYPERNODES flag |
| 46 | `HIERARCHICAL_PORT_ORTHOGONAL_EDGE_ROUTER` | Route edges to external ports | AFTER_P5 | EXTERNAL_PORTS + orthogonal |
| 47 | `LONG_EDGE_JOINER` | Recombine split long edges | AFTER_P5 | Always |
| 48 | `SELF_LOOP_POSTPROCESSOR` | Restore self-loops | AFTER_P5 | SELF_LOOPS flag |
| 49 | `BREAKING_POINT_REMOVER` | Remove wrapping break points | AFTER_P5 | Wrapping = MULTI_EDGE |
| 50 | `NORTH_SOUTH_PORT_POSTPROCESSOR` | Remove N/S port dummies | AFTER_P5 | NORTH_SOUTH_PORTS flag |
| 51 | `HORIZONTAL_COMPACTOR` | Compact horizontal spacing | AFTER_P5 | Compaction enabled |
| 52 | `LABEL_DUMMY_REMOVER` | Remove label dummies | AFTER_P5 | CENTER_LABELS flag |
| 53 | `FINAL_SPLINE_BENDPOINTS_CALCULATOR` | Compute final spline control points | AFTER_P5 | Spline routing |
| 54 | `END_LABEL_SORTER` | Sort end labels | AFTER_P5 | **Always** |
| 55 | `REVERSED_EDGE_RESTORER` | Restore reversed edges | AFTER_P5 | Always (from cycle breaking) |
| 56 | `END_LABEL_POSTPROCESSOR` | Position end labels | AFTER_P5 | END_LABELS flag |
| 57 | `HIERARCHICAL_NODE_RESIZER` | Resize compound nodes after layout | AFTER_P5 | INCLUDE_CHILDREN mode |
| 58 | `DIRECTION_POSTPROCESSOR` | Transform back from LTR | AFTER_P5 | Direction ≠ RIGHT |

---

## 15. Port System

ELK's first-class port system is the biggest differentiator from simpler Sugiyama
implementations. Ports are explicit edge attachment points on node borders.

### 15.1 Port Constraints (`PortConstraints` enum)

From least to most restrictive:

| Value | Description |
|-------|-------------|
| `UNDEFINED` | No constraints. Equivalent to FREE. |
| `FREE` | Algorithm has complete freedom to place ports anywhere on any side. |
| `FIXED_SIDE` | The side (N/S/E/W) of each port is fixed, but order and position within the side can change. |
| `FIXED_ORDER` | Side and order of ports are fixed. Spacing/positions along the side may be adjusted. Uses `port.index` for clockwise ordering from top-left. |
| `FIXED_RATIO` | Position as a ratio of the node size. When node is resized, ports maintain proportional position. |
| `FIXED_POS` | Exact position is fixed. Most restrictive. |

### 15.2 Port Side (`PortSide` enum)

| Value | Description | Layout role |
|-------|-------------|-------------|
| `EAST` | Right side of node | Output side in LTR layout |
| `WEST` | Left side of node | Input side in LTR layout |
| `NORTH` | Top side of node | Perpendicular, needs special handling |
| `SOUTH` | Bottom side of node | Perpendicular, needs special handling |
| `UNDEFINED` | Not yet determined | Assigned during PORT_SIDE_PROCESSOR |

In a left-to-right layout, edges naturally flow from EAST ports to WEST ports. North and
south ports require dummy nodes because their edges are perpendicular to the main flow.

### 15.3 Port Alignment (`PortAlignment` enum)

Controls how ports are distributed along a side:

| Value | Description |
|-------|-------------|
| `JUSTIFIED` (default) | Ports spread evenly across the side |
| `BEGIN` | Ports aligned to the beginning of the side (top or left) |
| `CENTER` | Ports centered on the side |
| `END` | Ports aligned to the end of the side (bottom or right) |

Can be set globally (`elk.portAlignment.default`) or per-side:
- `elk.portAlignment.north`
- `elk.portAlignment.south`
- `elk.portAlignment.east`
- `elk.portAlignment.west`

### 15.4 Port Side Assignment

The `PORT_SIDE_PROCESSOR` assigns sides to ports with `UNDEFINED` side based on edge
directions:
- Ports with only outgoing edges → EAST (output side)
- Ports with only incoming edges → WEST (input side)
- Ports with both → EAST (prefer output)
- Ports with no edges → EAST (default)

### 15.5 Port Sorting

The `PORT_LIST_SORTER` sorts ports within each side according to constraints:
- **FREE**: Sort by connected edge positions for crossing reduction
- **FIXED_ORDER**: Sort by `port.index`
- **FIXED_POS**: Sort by position coordinate

### 15.6 How Ports Affect Each Phase

| Phase | Port influence |
|-------|--------------|
| P1 Cycle Breaking | Edge direction determined by source/target port sides |
| P2 Layering | Port constraints may restrict layer assignment |
| P3 Crossing Min | Port order constraints restrict node reordering; NORTH/SOUTH ports create dummies |
| P4 Node Placement | Port positions affect vertical node placement; port spacing affects node height |
| P5 Edge Routing | Edge routes must connect to specific port positions on node borders |

### 15.7 Port Border Offset

`elk.port.borderOffset`: Positive values move the port outside the node border, negative
values move it inside. Zero means the port sits directly on the border.

---

## 16. Hierarchical / Compound Graph Handling

### 16.1 Hierarchy Handling Modes

| Value | Description |
|-------|-------------|
| `INHERIT` (default) | Inherits from parent. At root, defaults to SEPARATE_CHILDREN. |
| `SEPARATE_CHILDREN` | Each compound node gets an independent layout run. Bottom-up processing. |
| `INCLUDE_CHILDREN` | Parent and children are laid out together in a single run. |

### 16.2 Bottom-Up Layout (SEPARATE_CHILDREN)

```
function layoutSeparateChildren(graph):
    // 1. Recursively layout all innermost compound nodes first
    for each compound node (deepest first):
        layoutAlgorithm.layout(compoundNode.innerGraph)
        // After inner layout, the compound node's size is determined
        compoundNode.width = innerGraph.width + padding
        compoundNode.height = innerGraph.height + padding

    // 2. Layout this level treating compound nodes as opaque boxes
    layoutAlgorithm.layout(graph)

    // 3. Apply offsets to inner graphs
    for each compound node:
        translateInnerGraph(compoundNode, compoundNode.x, compoundNode.y)
```

**Limitations:**
- Cross-hierarchy edges cannot be properly routed
- Ordering decisions at each level are independent, so cross-hierarchy edge crossings
  cannot be optimized
- Hierarchical port positions are fixed by the inner layout

### 16.3 Single-Run Layout (INCLUDE_CHILDREN)

All hierarchy levels processed together:
1. Flatten the hierarchy into a single graph with external port dummies
2. Run the full layered pipeline once
3. `HIERARCHICAL_NODE_RESIZER` adjusts compound node sizes after layout
4. Cross-hierarchy edges are properly routed

Only the layered algorithm fully supports this mode.

### 16.4 Cross-Hierarchy Edges

When `elk.layered.mergeHierarchyEdges` is true (default), edges that cross hierarchy
levels are "merged" — broken into segments with external port dummies at each hierarchy
boundary, then recombined after layout.

---

## 17. Label Placement

### 17.1 Node Labels

Controlled by `elk.nodeLabels.placement` (EnumSet):

Vertical position: `V_TOP`, `V_CENTER`, `V_BOTTOM`
Horizontal position: `H_LEFT`, `H_CENTER`, `H_RIGHT`
Inside/outside: `INSIDE`, `OUTSIDE`

Examples:
- `"INSIDE V_CENTER H_CENTER"` — centered inside the node
- `"OUTSIDE V_TOP H_CENTER"` — centered above the node

When placed OUTSIDE, labels affect the node's margin (extra space around the node).

### 17.2 Edge Labels

`elk.edgeLabels.placement`:
- `CENTER` — label placed at the middle of the edge (uses label dummies)
- `HEAD` — label placed near the target end
- `TAIL` — label placed near the source end

`elk.layered.edgeLabels.sideSelection`:
- `ALWAYS_UP` — label above the edge
- `ALWAYS_DOWN` — label below the edge
- `SMART_UP` — above for downward edges, below for upward
- `SMART_DOWN` (default) — below for downward, above for upward
- `DIRECTION_UP` / `DIRECTION_DOWN` — based on flow direction

### 17.3 Port Labels

`elk.portLabels.placement`:
- `OUTSIDE` — label placed outside the node, next to the port
- `INSIDE` — label placed inside the node, next to the port
- `NEXT_TO_PORT_IF_POSSIBLE` — outside if space allows, inside otherwise
- `ALWAYS_SAME_SIDE` — consistently same side as port

---

## 18. Complete Layout Options Reference

### 18.1 Core Options

| Option | Key | Default | Description |
|--------|-----|---------|-------------|
| Algorithm | `elk.algorithm` | `layered` | Layout algorithm ID |
| Direction | `elk.direction` | `UNDEFINED` (= RIGHT for layered) | Flow direction: UP, DOWN, LEFT, RIGHT |
| Padding | `elk.padding` | `12` all sides | Padding inside container: `[left=X, top=Y, right=Z, bottom=W]` |
| Aspect Ratio | `elk.aspectRatio` | `1.6` | Target width/height for component packing |
| Separate Components | `elk.separateConnectedComponents` | `true` | Layout disconnected subgraphs independently |
| Hierarchy Handling | `elk.hierarchyHandling` | `INHERIT` | INHERIT, SEPARATE_CHILDREN, INCLUDE_CHILDREN |
| Port Constraints | `elk.portConstraints` | `UNDEFINED` | FREE, FIXED_SIDE, FIXED_ORDER, FIXED_RATIO, FIXED_POS |
| Interactive Layout | `elk.interactiveLayout` | `false` | Enable interactive mode |
| Random Seed | `elk.randomSeed` | `1` | Seed for randomized algorithms |
| Debug Mode | `elk.debugMode` | `false` | Enable debug output |

### 18.2 Spacing Options

| Option | Key | Default |
|--------|-----|---------|
| Node ↔ Node | `elk.spacing.nodeNode` | 20 |
| Node ↔ Node (between layers) | `elk.layered.spacing.nodeNodeBetweenLayers` | 20 |
| Edge ↔ Edge | `elk.spacing.edgeEdge` | 10 |
| Edge ↔ Edge (between layers) | `elk.layered.spacing.edgeEdgeBetweenLayers` | 10 |
| Edge ↔ Node | `elk.spacing.edgeNode` | 10 |
| Edge ↔ Node (between layers) | `elk.layered.spacing.edgeNodeBetweenLayers` | 10 |
| Edge ↔ Label | `elk.spacing.edgeLabel` | 2 |
| Port ↔ Port | `elk.spacing.portPort` | 10 |
| Label ↔ Label | `elk.spacing.labelLabel` | 0 |
| Label ↔ Node | `elk.spacing.labelNode` | 5 |
| Label ↔ Port (horizontal) | `elk.spacing.labelPortHorizontal` | 1 |
| Label ↔ Port (vertical) | `elk.spacing.labelPortVertical` | 1 |
| Comment ↔ Comment | `elk.spacing.commentComment` | 10 |
| Comment ↔ Node | `elk.spacing.commentNode` | 10 |
| Component ↔ Component | `elk.spacing.componentComponent` | 20 |
| Node Self Loop | `elk.spacing.nodeSelfLoop` | 10 |

### 18.3 Strategy Selection

| Option | Key | Default |
|--------|-----|---------|
| Cycle Breaking | `elk.layered.cycleBreaking.strategy` | `GREEDY` |
| Layering | `elk.layered.layering.strategy` | `NETWORK_SIMPLEX` |
| Crossing Minimization | `elk.layered.crossingMinimization.strategy` | `LAYER_SWEEP` |
| Node Placement | `elk.layered.nodePlacement.strategy` | `BRANDES_KOEPF` |
| Edge Routing | `elk.edgeRouting` | `ORTHOGONAL` |

### 18.4 Crossing Minimization Sub-Options

| Option | Key | Default |
|--------|-----|---------|
| Greedy Switch Type | `elk.layered.crossingMinimization.greedySwitch.type` | `TWO_SIDED` |
| Greedy Threshold | `elk.layered.crossingMinimization.greedySwitch.activationThreshold` | 40 |
| Thoroughness | `elk.layered.thoroughness` | 7 |
| Hierarchical Sweepiness | `elk.layered.crossingMinimization.hierarchicalSweepiness` | 0.1 |
| Semi-Interactive | `elk.layered.crossingMinimization.semiInteractive` | false |
| Force Model Order | `elk.layered.crossingMinimization.forceNodeModelOrder` | false |

### 18.5 Node Placement Sub-Options

| Option | Key | Default |
|--------|-----|---------|
| BK Edge Straightening | `elk.layered.nodePlacement.bk.edgeStraightening` | `IMPROVE_STRAIGHTNESS` |
| BK Fixed Alignment | `elk.layered.nodePlacement.bk.fixedAlignment` | `NONE` |
| Linear Segments Dampening | `elk.layered.nodePlacement.linearSegments.deflectionDampening` | 0.3 |
| NS Node Flexibility | `elk.layered.nodePlacement.networkSimplex.nodeFlexibility.default` | `NONE` |
| Favor Straight Edges | `elk.layered.nodePlacement.favorStraightEdges` | true |

### 18.6 Edge Routing Sub-Options

| Option | Key | Default |
|--------|-----|---------|
| Self-Loop Distribution | `elk.layered.edgeRouting.selfLoopDistribution` | `NORTH` |
| Self-Loop Ordering | `elk.layered.edgeRouting.selfLoopOrdering` | `STACKED` |
| Polyline Sloped Zone Width | `elk.layered.edgeRouting.polyline.slopedEdgeZoneWidth` | 2.0 |
| Spline Routing Mode | `elk.layered.edgeRouting.splines.mode` | `SLOPPY` |
| Sloppy Layer Spacing Factor | `elk.layered.edgeRouting.splines.sloppy.layerSpacingFactor` | 0.2 |

### 18.7 Layering Sub-Options

| Option | Key | Default |
|--------|-----|---------|
| Layer Constraint | `elk.layered.layering.layerConstraint` | `NONE` |
| Coffman-Graham Bound | `elk.layered.layering.coffmanGraham.layerBound` | MAX_INT |
| Node Promotion Strategy | `elk.layered.layering.nodePromotion.strategy` | `NONE` |
| Node Promotion Max Iter | `elk.layered.layering.nodePromotion.maxIterations` | 0 |
| Layer Unzipping | `elk.layered.layerUnzipping.strategy` | `NONE` |

### 18.8 Label Options

| Option | Key | Default |
|--------|-----|---------|
| Node Label Placement | `elk.nodeLabels.placement` | `fixed()` |
| Node Label Padding | `elk.nodeLabels.padding` | `5` all sides |
| Edge Label Placement | `elk.edgeLabels.placement` | `CENTER` |
| Edge Label Side | `elk.layered.edgeLabels.sideSelection` | `SMART_DOWN` |
| Center Label Strategy | `elk.layered.edgeLabels.centerLabelPlacementStrategy` | `MEDIAN_LAYER` |
| Port Label Placement | `elk.portLabels.placement` | `OUTSIDE` |
| Inline Edge Labels | `elk.edgeLabels.inline` | false |

### 18.9 Port Options

| Option | Key | Default |
|--------|-----|---------|
| Port Constraints | `elk.portConstraints` | `UNDEFINED` |
| Port Side | `elk.port.side` | `UNDEFINED` |
| Port Index | `elk.port.index` | 0 |
| Port Border Offset | `elk.port.borderOffset` | 0 |
| Port Alignment Default | `elk.portAlignment.default` | `JUSTIFIED` |
| Port Alignment North | `elk.portAlignment.north` | (inherit) |
| Port Alignment South | `elk.portAlignment.south` | (inherit) |
| Port Alignment East | `elk.portAlignment.east` | (inherit) |
| Port Alignment West | `elk.portAlignment.west` | (inherit) |

### 18.10 Other Layered Options

| Option | Key | Default | Description |
|--------|-----|---------|-------------|
| Merge Edges | `elk.layered.mergeEdges` | false | Combine parallel edges |
| Merge Hierarchy Edges | `elk.layered.mergeHierarchyEdges` | true | Combine cross-hierarchy edges |
| Feedback Edges | `elk.layered.feedbackEdges` | false | Handle cycle-creating edges specially |
| Unnecessary Bendpoints | `elk.layered.unnecessaryBendpoints` | false | Add extra bend points for uniform look |
| Wrapping Strategy | `elk.layered.wrapping.strategy` | `OFF` | OFF, SINGLE_EDGE, MULTI_EDGE |
| Compaction Strategy | `elk.layered.compaction.postCompaction.strategy` | `NONE` | Post-layout compaction |
| High Degree Treatment | `elk.layered.highDegreeNodes.treatment` | false | Special handling |
| High Degree Threshold | `elk.layered.highDegreeNodes.threshold` | 16 | Degree threshold |
| Consider Model Order | `elk.layered.considerModelOrder.strategy` | `NONE` | Preserve input ordering |
| Direction Congruency | `elk.layered.directionCongruency` | `READING_DIRECTION` | Ensure direction matches reading |

---

## 19. Typical Pipeline Example

For a simple left-to-right graph with orthogonal routing, the pipeline looks like:

```
Slot 00: DIRECTION_PREPROCESSOR              (BEFORE_P1) — Transform to LTR
Slot 01: PORT_SIDE_PROCESSOR                 (BEFORE_P1) — Assign port sides
Slot 02: GreedyCycleBreaker                  (P1)        — Break cycles
Slot 03: EDGE_AND_LAYER_CONSTRAINT_REVERSER  (BEFORE_P2) — Reverse constraint edges
Slot 04: LAYER_CONSTRAINT_PREPROCESSOR       (BEFORE_P2) — Process layer constraints
Slot 05: NetworkSimplexLayerer               (P2)        — Assign layers
Slot 06: LAYER_CONSTRAINT_POSTPROCESSOR      (BEFORE_P3) — Finalize constraints
Slot 07: LONG_EDGE_SPLITTER                  (BEFORE_P3) — Create long-edge dummies
Slot 08: PORT_LIST_SORTER                    (BEFORE_P3) — Sort port lists
Slot 09: LayerSweepCrossingMinimizer         (P3)        — Minimize crossings
Slot 10: IN_LAYER_CONSTRAINT_PROCESSOR       (BEFORE_P4) — Enforce TOP/BOTTOM
Slot 11: INNERMOST_NODE_MARGIN_CALCULATOR    (BEFORE_P4) — Compute margins
Slot 12: LABEL_AND_NODE_SIZE_PROCESSOR       (BEFORE_P4) — Compute node sizes
Slot 13: BrandesKoepfNodePlacer              (P4)        — Place nodes
Slot 14: LAYER_SIZE_AND_GRAPH_HEIGHT_CALC    (BEFORE_P5) — Compute dimensions
Slot 15: OrthogonalEdgeRouter                (P5)        — Route edges
Slot 16: LONG_EDGE_JOINER                    (AFTER_P5)  — Recombine long edges
Slot 17: REVERSED_EDGE_RESTORER              (AFTER_P5)  — Restore reversed edges
Slot 18: END_LABEL_SORTER                    (AFTER_P5)  — Sort end labels
Slot 19: DIRECTION_POSTPROCESSOR             (AFTER_P5)  — Transform back from LTR
```

With north/south ports, self-loops, external ports, and center labels, the pipeline can
grow to 30+ slots.

---

## 20. Comparison: ELK vs Dagre

### 20.1 Algorithm Sophistication

| Feature | Dagre | ELK Layered |
|---------|-------|-------------|
| Main phases | 3 (rank, order, position) | 5 + 58 intermediate processors |
| Cycle breaking | 1 strategy (DFS) | 9 strategies |
| Layering | 2 strategies (longest path, network simplex) | 9 strategies |
| Crossing minimization | 1 (barycenter) | 4 strategies + greedy switch |
| Node placement | 1 (simplified Brandes-Köpf) | 5 strategies |
| Edge routing | 1 (basic) | 4 styles with sub-options |
| Layout options | ~20 | 170+ |
| Dummy node types | 1 (long edge) | 8 types |

### 20.2 Port Support

| Feature | Dagre | ELK |
|---------|-------|-----|
| Port concept | None (edges connect to node centers) | First-class LPort with 6 constraint levels |
| Port sides | N/A | NORTH, SOUTH, EAST, WEST with per-side alignment |
| Port ordering | N/A | Fixed order, free, by index |
| Port influence | N/A | Affects all 5 phases |

### 20.3 Compound Graph Support

| Feature | Dagre | ELK |
|---------|-------|-----|
| Nested nodes | Limited (nesting graph approach) | Full compound with cross-hierarchy edges |
| Hierarchy modes | N/A | SEPARATE_CHILDREN, INCLUDE_CHILDREN |
| Cross-hierarchy edges | Not supported | Full support with merging |

### 20.4 Coordinate System

| Feature | Dagre | ELK |
|---------|-------|-----|
| Node position | Center of node | Top-left corner |
| Edge routing | List of points | Sections with start/end/bend points |
| Label position | Separate | Inline on elements |

### 20.5 Features Unique to ELK

- Self-loop routing with configurable distribution and ordering
- Comment box layout
- Hyperedge and hypernode support
- Graph wrapping (multi-row layout)
- Layout partitioning
- Model order preservation
- Interactive layout modes
- Port alignment per side
- Post-layout compaction
- High-degree node special handling

---

## 21. Porting Priority Recommendations

### Phase 1: Core Pipeline (MVP)

These are the absolute minimum for a working layered layout:

1. **LGraph internal representation** — LGraph, LNode, LEdge, LPort, LLabel, Layer
2. **JSON import/export** — ElkGraph ↔ LGraph conversion
3. **DIRECTION_PREPROCESSOR / POSTPROCESSOR** — Graph transformation for non-RIGHT directions
4. **PORT_SIDE_PROCESSOR** — Assign port sides
5. **P1: GreedyCycleBreaker** + REVERSED_EDGE_RESTORER
6. **P2: LongestPathLayerer** (simplest) or NetworkSimplexLayerer (optimal)
7. **LONG_EDGE_SPLITTER** + LONG_EDGE_JOINER
8. **PORT_LIST_SORTER**
9. **P3: LayerSweepCrossingMinimizer** (barycenter heuristic)
10. **IN_LAYER_CONSTRAINT_PROCESSOR**
11. **INNERMOST_NODE_MARGIN_CALCULATOR** + LABEL_AND_NODE_SIZE_PROCESSOR
12. **P4: SimpleNodePlacer** (easiest) or BrandesKoepfNodePlacer (best quality)
13. **LAYER_SIZE_AND_GRAPH_HEIGHT_CALCULATOR**
14. **P5: PolylineEdgeRouter** (simplest) or OrthogonalEdgeRouter (best quality)

### Phase 2: Port Support

15. North/South port dummies (NORTH_SOUTH_PORT_PREPROCESSOR / POSTPROCESSOR)
16. INVERTED_PORT_PROCESSOR
17. Port constraints (FIXED_SIDE, FIXED_ORDER, FIXED_POS)
18. Port alignment options

### Phase 3: Compound Graphs

19. External port dummies + hierarchical port processors
20. HIERARCHICAL_NODE_RESIZER
21. INCLUDE_CHILDREN / SEPARATE_CHILDREN handling
22. Cross-hierarchy edge merging

### Phase 4: Labels

23. Node label placement (INSIDE/OUTSIDE with alignment)
24. Edge label dummies (center labels)
25. End labels (head/tail)

### Phase 5: Polish

26. Self-loop handling
27. Comment node handling
28. Network simplex layering (if not done in Phase 1)
29. Brandes-Köpf node placement (if not done in Phase 1)
30. Orthogonal edge routing (if not done in Phase 1)
31. Greedy switch post-processing
32. Graph wrapping
33. Post-layout compaction
34. Connected component packing
35. Additional cycle-breaking / layering / crossing-min strategies
