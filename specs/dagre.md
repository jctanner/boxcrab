# Dagre Layout Algorithm Specification

This document is a language-agnostic specification of the dagre directed graph layout algorithm, derived from a complete reading of the dagre source code. It is intended as a porting reference.

Dagre implements a layered (Sugiyama-style) graph layout. The core technique comes from:

- Gansner, Koutsofios, North, Vo: "A Technique for Drawing Directed Graphs" (1993)
- Brandes, Köpf: "Fast and Simple Horizontal Coordinate Assignment" (2002)
- Barth, Jünger, Mutzel: "Bilayer Cross Counting" (2002)
- Sander: "Layout of Compound Directed Graphs" (1996)
- Eades, Lin, Smyth: "A fast and effective heuristic for the feedback arc set problem" (1993)
- Forster: "A Fast and Simple Heuristic for Constrained Two-Level Crossing Reduction" (2004)

---

## Table of Contents

1. [Data Model](#1-data-model)
2. [High-Level Pipeline](#2-high-level-pipeline)
3. [Phase 0: Input Graph Construction](#3-phase-0-input-graph-construction)
4. [Phase 1: Make Space for Edge Labels](#4-phase-1-make-space-for-edge-labels)
5. [Phase 2: Cycle Removal (Acyclic)](#5-phase-2-cycle-removal)
6. [Phase 3: Nesting Graph (Compound Graph Support)](#6-phase-3-nesting-graph)
7. [Phase 4: Rank Assignment](#7-phase-4-rank-assignment)
8. [Phase 5: Edge Label Proxy Injection](#8-phase-5-edge-label-proxy-injection)
9. [Phase 6: Rank Normalization & Cleanup](#9-phase-6-rank-normalization--cleanup)
10. [Phase 7: Edge Normalization (Long Edge Splitting)](#10-phase-7-edge-normalization)
11. [Phase 8: Parent Dummy Chains](#11-phase-8-parent-dummy-chains)
12. [Phase 9: Border Segments](#12-phase-9-border-segments)
13. [Phase 10: Node Ordering (Crossing Minimization)](#13-phase-10-node-ordering)
14. [Phase 11: Coordinate Assignment (Positioning)](#14-phase-11-coordinate-assignment)
15. [Phase 12: Post-Processing & Undo](#15-phase-12-post-processing--undo)
16. [Graph Library Requirements](#16-graph-library-requirements)
17. [Configuration Reference](#17-configuration-reference)
18. [Utility Algorithms](#18-utility-algorithms)

---

## 1. Data Model

### 1.1 Graph

The algorithm operates on a **directed multigraph** with optional **compound** (hierarchical) structure. A compound graph allows nodes to be children of other nodes, forming subgraphs/clusters.

Properties of the graph:
- **Directed**: edges have a source (`v`) and target (`w`)
- **Multigraph**: multiple edges between the same pair of nodes are allowed, distinguished by `name`
- **Compound**: nodes can have parent-child relationships, forming a hierarchy

### 1.2 Edge

An edge is identified by the triple `(v, w, name)` where:
- `v` — source node ID (string)
- `w` — target node ID (string)
- `name` — optional edge name for multigraph disambiguation (string)

### 1.3 Node Label (properties assigned to each node)

| Property | Type | Default | Description |
|---|---|---|---|
| `width` | float | 0 | Width of the node in pixels |
| `height` | float | 0 | Height of the node in pixels |
| `x` | float | — | Assigned x-coordinate (center) |
| `y` | float | — | Assigned y-coordinate (center) |
| `rank` | int | — | Assigned layer/rank |
| `order` | int | — | Position within its rank layer |
| `dummy` | string | — | If set, this is a dummy node. Values: `"edge"`, `"border"`, `"edge-label"`, `"edge-proxy"`, `"selfedge"`, `"root"` |
| `borderType` | string | — | `"borderLeft"` or `"borderRight"` for border dummy nodes |
| `borderTop` | string | — | Node ID of the top border node (for compound nodes) |
| `borderBottom` | string | — | Node ID of the bottom border node (for compound nodes) |
| `borderLeft` | string[] | — | Array indexed by rank, holding left border node IDs |
| `borderRight` | string[] | — | Array indexed by rank, holding right border node IDs |
| `minRank` | int | — | Minimum rank spanned (for compound nodes) |
| `maxRank` | int | — | Maximum rank spanned (for compound nodes) |
| `edgeLabel` | EdgeLabel | — | Back-reference to the original edge label (for dummy nodes on long edges) |
| `edgeObj` | Edge | — | Back-reference to the original edge (for dummy nodes on long edges) |
| `labelpos` | string | — | `"l"`, `"c"`, or `"r"` (for edge-label dummy nodes) |
| `padding` | float | — | Node padding |
| `selfEdges` | array | — | Temporarily stored self-edges during layout |

### 1.4 Edge Label (properties assigned to each edge)

| Property | Type | Default | Description |
|---|---|---|---|
| `minlen` | int | 1 | Minimum number of ranks the edge must span |
| `weight` | float | 1 | Edge weight (higher = shorter, more vertical) |
| `width` | float | 0 | Width of the edge label |
| `height` | float | 0 | Height of the edge label |
| `labelpos` | string | `"r"` | Label position: `"l"` (left), `"c"` (center), `"r"` (right) |
| `labeloffset` | float | 10 | Offset of label from edge |
| `labelRank` | int | — | Rank where the label should be placed |
| `x` | float | — | Assigned x-coordinate of the edge label |
| `y` | float | — | Assigned y-coordinate of the edge label |
| `points` | Point[] | — | Route points for the edge |
| `reversed` | bool | — | True if the edge was reversed to break a cycle |
| `forwardName` | string | — | Original edge name before reversal |
| `nestingEdge` | bool | — | True if this is a nesting graph structural edge |
| `cutvalue` | float | — | Used during network simplex |

### 1.5 Graph Label (global layout configuration)

| Property | Type | Default | Description |
|---|---|---|---|
| `rankdir` | string | `"TB"` | Rank direction: `"TB"`, `"BT"`, `"LR"`, `"RL"` |
| `align` | string | — | Alignment for rank assignment: `"UL"`, `"UR"`, `"DL"`, `"DR"` |
| `rankalign` | string | `"center"` | Vertical alignment within a rank: `"top"`, `"center"`, `"bottom"` |
| `nodesep` | float | 50 | Horizontal separation between nodes in the same rank |
| `edgesep` | float | 20 | Horizontal separation between edges in the same rank |
| `ranksep` | float | 50 | Vertical separation between ranks |
| `marginx` | float | 0 | Horizontal margin around the graph |
| `marginy` | float | 0 | Vertical margin around the graph |
| `acyclicer` | string | — | Cycle removal algorithm: `"greedy"` or default (DFS) |
| `ranker` | string | `"network-simplex"` | Ranking algorithm: `"network-simplex"`, `"tight-tree"`, `"longest-path"` |
| `nestingRoot` | string | — | Internal: ID of the nesting root node |
| `nodeRankFactor` | int | — | Internal: spacing multiplier for nesting graph |
| `dummyChains` | string[] | — | Internal: first dummy in each chain of long-edge dummies |
| `maxRank` | int | — | Internal: maximum rank value in the graph |

### 1.6 Point

A simple `{x: float, y: float}` coordinate pair.

---

## 2. High-Level Pipeline

The layout algorithm is a sequential pipeline of transformations on the graph. Each phase mutates the graph in place. Many phases are reversible (have an "undo" step that runs later).

```
 Input Graph
     |
     v
 [Build Layout Graph]         -- Copy input, apply defaults, canonicalize
     |
     v
 [Make Space for Edge Labels] -- Double minlen, halve ranksep
     |
     v
 [Remove Self-Edges]          -- Stash self-edges on their nodes
     |
     v
 [Cycle Removal]              -- Reverse back-edges to make DAG  (reversible)
     |
     v
 [Nesting Graph]              -- Add compound-graph structure edges  (reversible)
     |
     v
 [Rank Assignment]            -- Assign each node a rank (layer)
     |
     v
 [Edge Label Proxies]         -- Inject proxy nodes to preserve label rank  (reversible)
     |
     v
 [Remove Empty Ranks]         -- Collapse gaps in rank numbering
     |
     v
 [Nesting Graph Cleanup]      -- Remove nesting edges and root
     |
     v
 [Normalize Ranks]            -- Shift ranks so minimum is 0
     |
     v
 [Assign Rank Min/Max]        -- Set minRank/maxRank on compound nodes
     |
     v
 [Remove Edge Label Proxies]  -- Transfer labelRank to edges, remove proxy nodes
     |
     v
 [Edge Normalization]         -- Split long edges into chains of unit-length edges  (reversible)
     |
     v
 [Parent Dummy Chains]        -- Assign dummy nodes to correct parent subgraphs
     |
     v
 [Add Border Segments]        -- Add left/right border dummy nodes per rank per subgraph
     |
     v
 [Node Ordering]              -- Minimize edge crossings (assign "order" to each node)
     |
     v
 [Insert Self-Edges]          -- Re-introduce self-edge dummy nodes into ordering
     |
     v
 [Adjust Coordinate System]   -- Swap width/height for LR/RL layouts
     |
     v
 [Position Assignment]        -- Assign x, y coordinates to all nodes
     |
     v
 [Position Self-Edges]        -- Compute self-edge control points, restore edges
     |
     v
 [Remove Border Nodes]        -- Compute compound node sizes, remove border dummies
     |
     v
 [Edge Denormalization]       -- Reconstruct original edges from dummy chains  (undo)
     |
     v
 [Fix Edge Label Coords]      -- Adjust label positions based on labelpos
     |
     v
 [Undo Coordinate System]     -- Reverse Y for BT/RL, swap X/Y for LR/RL
     |
     v
 [Translate Graph]            -- Shift all coordinates so graph starts at (marginx, marginy)
     |
     v
 [Assign Node Intersects]     -- Clip edge endpoints to node boundaries
     |
     v
 [Reverse Points]             -- Reverse point arrays for reversed edges
     |
     v
 [Undo Cycle Removal]         -- Restore original edge directions
     |
     v
 [Update Input Graph]         -- Copy x, y, width, height, points back to input
     |
     v
 Output Graph
```

---

## 3. Phase 0: Input Graph Construction

### 3.1 Build Layout Graph

Create a fresh internal graph (multigraph + compound) by copying from the input graph.

**Graph-level attributes**: Copy from input, canonicalize all keys to lowercase, merge with defaults:
- `ranksep=50, edgesep=20, nodesep=50, rankdir="TB", rankalign="center"`
- Numeric attributes (`nodesep, edgesep, ranksep, marginx, marginy`) are cast to numbers
- String attributes (`acyclicer, ranker, rankdir, align, rankalign`) are copied as-is

**Node attributes**: For each node, copy numeric attributes (`width, height, rank`), merge with defaults `{width: 0, height: 0}`. Preserve parent-child relationships.

**Edge attributes**: For each edge, copy numeric attributes (`minlen, weight, width, height, labeloffset`) and string attributes (`labelpos`), merge with defaults `{minlen: 1, weight: 1, width: 0, height: 0, labeloffset: 10, labelpos: "r"}`.

### 3.2 Canonicalization

All attribute keys from the input are lowercased. This allows case-insensitive input (e.g., `rankDir` becomes `rankdir`).

---

## 4. Phase 1: Make Space for Edge Labels

This technique comes from the Gansner paper. To accommodate edge labels, each rank is conceptually split in half:

1. **Halve `ranksep`**: `graph.ranksep = graph.ranksep / 2`
2. **Double `minlen`** on every edge: `edge.minlen = edge.minlen * 2`
3. **Pad edge label dimensions**: If the label is not centered (`labelpos != "c"`):
   - For `TB`/`BT` layouts: `edge.width += edge.labeloffset`
   - For `LR`/`RL` layouts: `edge.height += edge.labeloffset`

The effect: labels get their own dedicated rank between node ranks.

---

## 5. Phase 2: Cycle Removal

The input graph may contain cycles. All subsequent algorithms require a DAG. This phase identifies **back-edges** and reverses them.

### 5.1 Self-Edge Removal (before cycle removal)

Self-edges (`v == w`) are removed from the graph and stashed on the source node's label in a `selfEdges` array. They are re-introduced later after ordering.

### 5.2 Feedback Arc Set

Two strategies for finding edges to reverse:

#### 5.2.1 DFS-Based (default)

1. Perform DFS over all nodes
2. Maintain a `stack` set (nodes currently on the DFS path) and a `visited` set
3. When an out-edge leads to a node already on the stack, it's a back-edge: add it to the feedback arc set
4. After DFS completes, return all collected back-edges

#### 5.2.2 Greedy FAS (when `acyclicer = "greedy"`)

From Eades, Lin, Smyth. This heuristic finds a small feedback arc set using weighted bucket sorting.

**Algorithm**:

1. Build a simplified graph aggregating multi-edge weights into single edges
2. For each node, compute `in` (total incoming weight) and `out` (total outgoing weight)
3. Create `maxIn + maxOut + 3` buckets. The zero index is at `maxIn + 1`
4. Assign each node to a bucket:
   - If `out == 0`: bucket 0 (sinks)
   - If `in == 0`: last bucket (sources)
   - Otherwise: bucket `out - in + zeroIdx`
5. Loop until graph is empty:
   a. Remove all sinks (bucket 0), updating neighbor bucket assignments
   b. Remove all sources (last bucket), updating neighbor bucket assignments
   c. If nodes remain, remove the node from the highest non-empty bucket (maximizing `out - in`). Record its incoming edges as part of the FAS
6. Expand the FAS edges back to the original multi-edges

### 5.3 Edge Reversal

For each edge in the feedback arc set:
1. Remove the edge `(v, w, name)` from the graph
2. Store `forwardName = name` and `reversed = true` on the edge label
3. Re-insert as `(w, v, uniqueId("rev"))`

### 5.4 Undo (later in pipeline)

For each edge with `reversed == true`:
1. Remove the reversed edge
2. Delete `reversed` and `forwardName` from the label
3. Re-insert as `(w, v, forwardName)` (restoring original direction)

---

## 6. Phase 3: Nesting Graph (Compound Graph Support)

For compound graphs (those with subgraph/cluster hierarchy), this phase ensures that:
1. All cluster members are placed between their cluster's top and bottom boundaries
2. The graph is connected (required for ranking)

Based on Sander, "Layout of Compound Directed Graphs."

### 6.1 Tree Depths

Compute the depth of each node in the compound hierarchy via DFS from the root's children. Root-level children have depth 1, their children depth 2, etc.

### 6.2 Node Separation Factor

```
height = max(all_depths) - 1
nodeSep = 2 * height + 1
```

All existing edge `minlen` values are multiplied by `nodeSep` to ensure regular nodes don't land on the same rank as subgraph border nodes.

### 6.3 Structural Edges

The sum of all edge weights plus 1 is used as `weight` for nesting edges (ensuring they dominate ranking).

For each subgraph, via recursive DFS:

**Leaf nodes** (no children): Add edge `(root, leaf)` with `{weight: 0, minlen: nodeSep}`.

**Subgraph nodes** (has children):
1. Create two border dummy nodes: `top` and `bottom`
2. Set them as children of the subgraph
3. Store `borderTop` and `borderBottom` on the subgraph's label
4. For each child of the subgraph:
   - Recursively process the child
   - Add edge `(top, childTop)` with `{weight: thisWeight, minlen: minlen, nestingEdge: true}`
   - Add edge `(childBottom, bottom)` with `{weight: thisWeight, minlen: minlen, nestingEdge: true}`
   - `thisWeight` = `weight` if child is a subgraph, `2 * weight` otherwise
   - `minlen` = 1 if child is a subgraph, else `height - depth[v] + 1`
5. If the subgraph has no parent: add edge `(root, top)` with `{weight: 0, minlen: height + depth[v]}`

Store `nodeRankFactor = nodeSep` on the graph label for later use in empty rank removal.

### 6.4 Cleanup (later in pipeline)

1. Remove the nesting root node
2. Remove all edges with `nestingEdge == true`

---

## 7. Phase 4: Rank Assignment

Assign an integer `rank` to each node such that for every edge `(v, w)`: `rank(w) - rank(v) >= minlen(v, w)`.

The algorithm works on a **non-compound** view of the graph (only leaf nodes, no subgraph containers).

Three strategies are available:

### 7.1 Longest Path (fast, poor quality)

A simple DFS from source nodes (nodes with no incoming edges):

```
function dfs(v):
    if visited[v]: return rank[v]
    visited[v] = true
    rank[v] = min over all out-edges (v,w):  dfs(w) - minlen(v,w)
    if no out-edges: rank[v] = 0
    return rank[v]

for each source node: dfs(source)
```

This pushes all nodes to the lowest possible rank, making the graph wide at the bottom. Fast, but produces long edges.

### 7.2 Tight-Tree (moderate quality)

1. Run longest path to get initial ranks
2. Build a feasible tight tree (see 7.4)
3. The resulting ranks are the output

### 7.3 Network Simplex (best quality, default)

From Gansner et al. This iteratively improves the ranking.

**Overview**:
1. Compute initial ranks via longest path
2. Build a feasible tight tree
3. Compute low/lim values (for ancestor testing)
4. Compute cut values for all tree edges
5. Iteratively swap tree edges to improve the ranking

#### 7.3.1 Feasible Tight Tree

A spanning tree where every tree edge is **tight** (has zero slack). Slack = `rank(w) - rank(v) - minlen(v,w)`.

**Algorithm**:
1. Start with an arbitrary node as the tree
2. Grow the tree by DFS, adding any tight edges (slack == 0) reachable from current tree nodes
3. If the tree doesn't span all nodes, find the non-tree edge with minimum slack that crosses the tree boundary
4. Shift all tree node ranks by `delta` to make that edge tight:
   - If the tree side is `v`: `delta = +slack`
   - If the tree side is `w`: `delta = -slack`
5. Repeat from step 2 until the tree spans all nodes

Returns an undirected tree graph.

#### 7.3.2 Low/Lim Values

Assign to each tree node via DFS:
- `low`: the smallest DFS number in the subtree
- `lim`: the DFS number of this node itself
- `parent`: the parent node in the tree

These enable O(1) ancestor testing: node A is a descendant of node B iff `B.low <= A.lim <= B.lim`.

#### 7.3.3 Cut Values

For each tree edge, the **cut value** represents the difference in total edge weight on each side if the tree were split at that edge. A negative cut value means the edge should be replaced.

**Computation** (bottom-up, post-order):

For tree edge `(child, parent)`:
1. Determine if `child` is the tail of the corresponding graph edge: check if `graph.edge(child, parent)` exists. If not, child is the head and we flip the logic.
2. Start with `cutvalue = weight(graph_edge)`
3. For each graph edge `(child, other)` where `other != parent`:
   - `pointsToHead` = `(isOutEdge == childIsTail)`
   - If `pointsToHead`: `cutvalue += weight(edge)`; else `cutvalue -= weight(edge)`
   - If `(child, other)` is also a tree edge:
     - If `pointsToHead`: `cutvalue -= cutvalue(child, other)`; else `cutvalue += cutvalue(child, other)`

#### 7.3.4 Leave Edge

Find any tree edge with `cutvalue < 0`. If none exists, the ranking is optimal — stop.

#### 7.3.5 Enter Edge

Given the leaving edge `(v, w)`, find the entering edge:

1. Orient so v is tail, w is head in the graph. If `graph.edge(v, w)` doesn't exist, swap.
2. Determine the `tailLabel` = the node that is NOT the tree root side
3. `flip` = true if the root is on the tail side (i.e., `vLabel.lim > wLabel.lim`)
4. Find all graph edges `(a, b)` where exactly one of `{a, b}` is a descendant of `tailLabel`:
   - `flip == isDescendant(a, tailLabel)` AND `flip != isDescendant(b, tailLabel)`
5. Among these candidates, return the one with minimum slack

#### 7.3.6 Exchange Edges

1. Remove the leaving edge from the tree
2. Add the entering edge to the tree
3. Recompute low/lim values
4. Recompute all cut values
5. Update ranks: DFS from root, for each non-root node:
   - If `graph.edge(v, parent)` exists: `rank(v) = rank(parent) - minlen`
   - If `graph.edge(parent, v)` exists: `rank(v) = rank(parent) + minlen`

Repeat the leave/enter/exchange cycle until no negative cut value exists.

### 7.4 Graph Simplification for Ranking

Before network simplex, the multigraph is simplified to a simple graph:
- Multiple edges between the same pair are collapsed
- Weights are summed: `newWeight = sum(all edge weights)`
- Minlen is the maximum: `newMinlen = max(all edge minlens)`

---

## 8. Phase 5: Edge Label Proxy Injection

For edges with non-zero `width` AND `height` (i.e., the edge has a label):
1. Compute `rank = (rank(w) - rank(v)) / 2 + rank(v)` (midpoint rank)
2. Create a dummy node of type `"edge-proxy"` at that rank
3. Store the edge reference on the proxy node

This ensures the label's rank is preserved when empty ranks are removed.

---

## 9. Phase 6: Rank Normalization & Cleanup

### 9.1 Remove Empty Ranks

After nesting graph cleanup, some ranks may be empty. The `nodeRankFactor` (from nesting graph) determines which empty ranks are allowed to be removed:

- An empty rank at position `i` is only removed if `i % nodeRankFactor != 0`
- Remaining nodes have their ranks shifted to close gaps

### 9.2 Nesting Graph Cleanup

Remove the nesting root node and all nesting edges.

### 9.3 Normalize Ranks

Shift all ranks so the minimum rank is 0: `rank(v) -= min(all ranks)`.

### 9.4 Assign Rank Min/Max

For each compound node (has children), set:
- `minRank` = rank of its `borderTop` node
- `maxRank` = rank of its `borderBottom` node
- `graph.maxRank` = max of all `maxRank` values

### 9.5 Remove Edge Label Proxies

For each node with `dummy == "edge-proxy"`:
1. Transfer `labelRank = proxy.rank` to the corresponding edge
2. Remove the proxy node

---

## 10. Phase 7: Edge Normalization (Long Edge Splitting)

**Preconditions**: The graph is a DAG. Each node has a `rank`.

**Goal**: All edges must span exactly 1 rank. Edges spanning multiple ranks are split.

### 10.1 Splitting

For each edge `(v, w)` where `rank(w) - rank(v) > 1`:
1. Remove the edge
2. Create a chain of dummy nodes, one per intermediate rank
3. Each dummy node has `{width: 0, height: 0, dummy: "edge", edgeLabel: originalLabel, edgeObj: originalEdge, rank: r}`
4. If the dummy's rank matches `labelRank`, it becomes an `"edge-label"` dummy with the edge's width, height, and labelpos
5. Connect the chain with edges carrying the original weight
6. Store the first dummy of each chain in `graph.dummyChains[]`

### 10.2 Undo (later in pipeline)

For each chain (starting from `graph.dummyChains`):
1. Restore the original edge with its original label
2. Walk the chain following successors
3. For each dummy node in the chain:
   - Add `{x: node.x, y: node.y}` to the edge's `points` array
   - If the dummy is `"edge-label"`: also set `edge.x, edge.y, edge.width, edge.height`
   - Remove the dummy node

---

## 11. Phase 8: Parent Dummy Chains

Dummy nodes from edge normalization need to be assigned to the correct subgraph in the compound hierarchy. This ensures they are placed within the correct cluster boundaries.

### 11.1 Postorder Numbers

Compute `{low, lim}` for each node in the compound hierarchy tree (same concept as network simplex low/lim). These enable O(1) ancestor testing.

### 11.2 Path Finding (LCA)

For each dummy chain's original edge `(v, w)`:
1. Find the **Lowest Common Ancestor (LCA)** of `v` and `w` in the compound hierarchy
2. Build the path from `v` up to LCA, then from LCA down to `w`

**LCA algorithm**:
1. Walk up from `v`, collecting ancestors, until reaching a node whose `[low, lim]` range contains both `v` and `w`
2. That node is the LCA
3. Walk up from `w` to LCA, collecting ancestors
4. Concatenate: `path = [v_ancestors..., lca, w_ancestors_reversed...]`

### 11.3 Assignment

Walk each dummy chain from first dummy to `w`:
1. In the ascending portion (before LCA): assign the dummy to the first path node whose `maxRank >= dummy.rank`
2. In the descending portion (after LCA): assign the dummy to the first path node whose `minRank <= dummy.rank`

This is done by calling `graph.setParent(dummy, pathNode)`.

---

## 12. Phase 9: Border Segments

For each compound node (those with `minRank` and `maxRank`):

1. Initialize `borderLeft = []` and `borderRight = []` arrays on the node
2. For each rank from `minRank` to `maxRank`:
   - Create a left border dummy node: `{width: 0, height: 0, rank: rank, dummy: "border", borderType: "borderLeft"}`
   - Create a right border dummy node: `{width: 0, height: 0, rank: rank, dummy: "border", borderType: "borderRight"}`
   - Set both as children of the compound node
   - Store their IDs in the respective arrays at index `rank`
   - If there's a border node at `rank-1`, add an edge from it to the current one (weight 1)

These border chains ensure compound node boundaries are properly maintained during ordering and positioning.

---

## 13. Phase 10: Node Ordering (Crossing Minimization)

**Goal**: Assign an `order` value to each node within its rank to minimize edge crossings.

### 13.1 Overview

The ordering algorithm uses an iterative heuristic:

1. Compute initial ordering via DFS
2. Alternate between top-down and bottom-up sweeps
3. In each sweep, sort nodes within each rank using barycenter heuristic
4. Count crossings after each sweep
5. Keep the best ordering found
6. Stop after 4 iterations without improvement

### 13.2 Initial Ordering

DFS from nodes sorted by rank (lowest first):
1. Visit each node exactly once
2. When visiting, add to `layers[node.rank]`
3. Then DFS to all successors

This produces a reasonable starting order.

### 13.3 Layer Graph Construction

For each rank, build a "layer graph" that includes:
- All nodes at that rank (preserving hierarchy)
- Edges connecting those nodes to adjacent ranks
- A virtual root node that is parent of all parentless nodes at that rank

Two sets of layer graphs are built:
- **Down layer graphs**: for ranks 1..maxRank, using `inEdges` (predecessors influence position)
- **Up layer graphs**: for ranks (maxRank-1)..0, using `outEdges` (successors influence position)

**Optimized node lookup**: An index mapping from rank to nodes is pre-built to avoid quadratic scans.

Edge weights from the input graph are aggregated (multi-edges collapsed by summing weights) since the layer graph is not a multigraph.

For compound nodes that span multiple ranks, the layer graph node stores `{borderLeft: borderLeftId, borderRight: borderRightId}` for that specific rank.

### 13.4 Sweep

For each layer graph in the sweep:
1. Apply any explicit order constraints
2. Sort the subgraph rooted at the layer's virtual root
3. Assign order values from the sort result
4. Add subgraph constraints to the constraint graph

The sweep alternates direction (`i % 2` chooses down vs up) and bias (`i % 4 >= 2` chooses left vs right bias).

### 13.5 Barycenter Calculation

For each movable node `v`:
1. Look at all incoming edges (in the layer graph)
2. Compute weighted average of the `order` values of connected nodes in the adjacent layer:

```
barycenter(v) = sum(weight(e) * order(u)) / sum(weight(e))
                for all in-edges (u, v)
```

Nodes with no incoming edges have no barycenter (they are "free" and placed in gaps).

### 13.6 Subgraph Sorting

Recursive algorithm for sorting a subgraph:

1. Get all children of the subgraph (excluding border nodes if they exist)
2. Compute barycenters for all children
3. For children that are themselves subgraphs: recursively sort them, merge their barycenters
4. Resolve constraint conflicts (see 13.7)
5. Expand subgraph results inline
6. Sort by barycenter (see 13.8)
7. If border nodes exist: prepend left border, append right border; compute combined barycenter

### 13.7 Conflict Resolution

From Forster, "A Fast and Simple Heuristic for Constrained Two-Level Crossing Reduction."

Given barycenter entries and a constraint graph (where edge A→B means A must be left of B):

1. Build a map of entries with indegree counting
2. Process in topological order (sources first)
3. For each entry, examine its in-constraints:
   - If the constraining entry has no barycenter, or its barycenter >= this entry's barycenter: **merge** them (they can be in the same group without violating the constraint)
   - Otherwise: leave them separate
4. Merging two entries:
   - Concatenate their node lists (`source.vs + target.vs`)
   - Compute weighted average barycenter
   - Sum weights
   - Take minimum index

### 13.8 Final Sort

Partition entries into:
- **Sortable**: entries that have a barycenter value
- **Unsortable**: entries with no barycenter (free nodes)

1. Sort sortable entries by barycenter (with tie-breaking by index, bias-aware)
2. Interleave unsortable entries into gaps based on their original index

### 13.9 Cross Count

Uses the algorithm from Barth et al., "Bilayer Cross Counting" — an efficient O(|E| log |V|) method using an accumulator tree (binary indexed tree).

For each pair of adjacent layers:
1. Map south-layer nodes to positions 0..n
2. For each north-layer node (left to right), collect its south-layer connections sorted by position
3. Build an accumulator tree (array of size 2*nextPowerOf2 - 1)
4. For each south entry:
   - Walk up the tree from its position
   - At each level, if on a left branch, add the right sibling's weight to the crossing count
   - Add the entry's weight to each tree node on the path
5. Total crossings = sum of `weight * weightSum` for all entries

### 13.10 Subgraph Constraints

After sorting a layer, add constraints to the constraint graph to enforce relative ordering of subgraphs. For each node in the layer:
- Walk up the compound hierarchy
- If a sibling of the current subgraph was seen previously, add a constraint edge from the previous sibling to the current one

---

## 14. Phase 11: Coordinate Assignment (Positioning)

### 14.1 Y-Coordinate Assignment

Simple layer-by-layer assignment:

1. Build the layer matrix from ranks and orders
2. For each layer:
   - Find the maximum node height in the layer
   - Assign y-coordinates based on `rankalign`:
     - `"top"`: `y = prevY + height/2`
     - `"bottom"`: `y = prevY + maxHeight - height/2`
     - `"center"` (default): `y = prevY + maxHeight/2`
   - Advance: `prevY += maxHeight + ranksep`

### 14.2 X-Coordinate Assignment (Brandes-Köpf)

Based on "Fast and Simple Horizontal Coordinate Assignment" by Brandes and Köpf.

The algorithm computes **four** independent alignments and takes their median.

#### 14.2.1 Conflict Detection

**Type-1 conflicts**: A non-inner segment crosses an inner segment. An **inner segment** is an edge where both endpoints are dummy nodes (part of a long-edge chain).

Scan layer by layer (top to bottom):
1. Track the last inner-segment predecessor position (`k0`)
2. For each node, if it's on an inner segment, scan all predecessors of nodes between `scanPos` and current position
3. If a predecessor's order is outside `[k0, k1]` and it's not a dummy-to-dummy edge: mark as type-1 conflict

**Type-2 conflicts**: Similar but for border node segments crossing inner segments.

Conflicts are stored as a set of `(v, w)` pairs (canonicalized so `v < w` lexicographically).

#### 14.2.2 Four Alignments

The algorithm runs four times with different orientations:
1. **UL** (up-left): scan top-to-bottom, left-to-right, use predecessors
2. **UR** (up-right): scan top-to-bottom, right-to-left, use predecessors
3. **DL** (down-left): scan bottom-to-top, left-to-right, use successors
4. **DR** (down-right): scan bottom-to-top, right-to-left, use successors

For right-biased alignments, the layer arrays are reversed, and resulting x-values are negated.

#### 14.2.3 Vertical Alignment

For each of the 4 orientations:

1. Initialize: `root[v] = v`, `align[v] = v` for all nodes
2. Cache node positions from the layering matrix
3. For each layer, for each node `v`:
   - Get neighbors (predecessors or successors depending on orientation)
   - Sort neighbors by position
   - Find the **median** neighbor(s): indices `floor((len-1)/2)` to `ceil((len-1)/2)`
   - For each median neighbor `w`:
     - If `align[v] == v` (v not yet aligned) AND `pos(w) > prevIdx` (no crossing) AND no conflict between v and w:
       - `align[w] = v`
       - `root[v] = align[v] = root[w]`
       - `prevIdx = pos(w)`

This creates chains: `root → ... → align[v] → v → ... → align[last]` forming vertical blocks.

#### 14.2.4 Horizontal Compaction

Unlike the original BK algorithm, dagre uses a block graph approach:

1. **Build block graph**: For each layer, for each pair of adjacent nodes `(u, v)`:
   - If they have different roots: add edge `(root[u], root[v])` with weight = separation between them
   - Separation = `u.width/2 + (isDummy? edgesep : nodesep)/2 + (isDummy? edgesep : nodesep)/2 + v.width/2` (with label position adjustments)

2. **Pass 1 — Forward sweep**: Topological order (process predecessors first)
   - `x[v] = max over all predecessors u: x[u] + edge_weight(u,v)`

3. **Pass 2 — Backward sweep**: Reverse topological order (process successors first)
   - `x[v] = max(x[v], min over all successors w: x[w] - edge_weight(v,w))`
   - But don't shift border nodes of the "wrong" type past their natural position

4. **Propagate**: For every node, `x[v] = x[root[v]]`

Both passes use an iterative DFS-based topological traversal (not a simple sort).

#### 14.2.5 Alignment and Balancing

1. **Find smallest width alignment**: Among the 4 alignments, find the one with the smallest total width (max_x - min_x accounting for node widths)

2. **Align coordinates**: Shift each alignment so that:
   - Left-biased alignments: `min(xs)` matches `min(smallestAlignment)`
   - Right-biased alignments: `max(xs)` matches `max(smallestAlignment)`

3. **Balance**: For each node, take the **median** of its 4 alignment x-values (specifically the average of the 2nd and 3rd values when sorted). If a specific alignment is requested via `graph.align`, use that alignment's value directly.

### 14.3 Coordinate System Adjustment

**Before positioning** (`adjust`):
- For `LR`/`RL` layouts: swap `width` and `height` on all nodes and edges

**After positioning** (`undo`):
- For `BT`/`RL` layouts: negate all y-coordinates
- For `LR`/`RL` layouts: swap x↔y on all nodes, edge points, and edge labels; swap width↔height

This allows the algorithm to always work in "top-to-bottom" orientation internally.

---

## 15. Phase 12: Post-Processing & Undo

### 15.1 Self-Edge Positioning

For each dummy node with `dummy == "selfedge"`:
1. Get the real node it references
2. Compute 5 control points forming a loop on the right side:
   ```
   dx = dummyNode.x - (realNode.x + realNode.width/2)
   dy = realNode.height / 2
   points = [
     (x + 2dx/3, y - dy),
     (x + 5dx/6, y - dy),
     (x + dx,     y),
     (x + 5dx/6, y + dy),
     (x + 2dx/3, y + dy)
   ]
   ```
   where `x = realNode.x + realNode.width/2`, `y = realNode.y`
3. Restore the original edge with these points
4. Remove the dummy node

### 15.2 Border Node Removal

For each compound node:
1. Compute the compound node's dimensions from its border nodes:
   - `width = |borderRight.x - borderLeft.x|` (using the last rank's border nodes)
   - `height = |borderBottom.y - borderTop.y|`
   - `x = borderLeft.x + width/2`
   - `y = borderTop.y + height/2`
2. Remove all border dummy nodes

### 15.3 Edge Label Coordinate Fixup

For edges with labels (`edge.x` exists):
- If `labelpos == "l"` or `"r"`: reduce `edge.width` by `labeloffset`
- If `labelpos == "l"`: `edge.x -= edge.width/2 + labeloffset`
- If `labelpos == "r"`: `edge.x += edge.width/2 + labeloffset`

### 15.4 Graph Translation

Compute the bounding box of all nodes and edge labels:
1. Find `minX, minY, maxX, maxY` considering node centers ± half-widths/heights
2. Adjust: `minX -= marginx`, `minY -= marginy`
3. Shift all node positions: `x -= minX, y -= minY`
4. Shift all edge points: `x -= minX, y -= minY`
5. Shift all edge labels: `x -= minX` (only if edge has x), `y -= minY` (only if edge has y)
6. Set `graph.width = maxX - minX + marginx`, `graph.height = maxY - minY + marginy`

### 15.5 Node Intersect Assignment

Clip edge endpoints to the boundaries of their source/target nodes:

For each edge `(v, w)`:
1. If the edge has no points, use `nodeW` as p1 and `nodeV` as p2
2. Otherwise, `p1 = points[0]` (first point), `p2 = points[last]` (last point)
3. Prepend `intersectRect(nodeV, p1)` to the points array
4. Append `intersectRect(nodeW, p2)` to the points array

**Rectangle-line intersection** (`intersectRect`):

Given a rectangle (center x,y; width w; height h) and a point (px, py):
```
dx = px - x
dy = py - y
if |dy| * (w/2) > |dx| * (h/2):
    // intersects top or bottom
    h = h/2 * sign(dy)
    sx = h * dx / dy
    sy = h
else:
    // intersects left or right
    w = w/2 * sign(dx)
    sx = w
    sy = w * dy / dx
return (x + sx, y + sy)
```

### 15.6 Reverse Points for Reversed Edges

For each edge with `reversed == true`: reverse the `points` array so that points flow from the original source to the original target.

### 15.7 Update Input Graph

Copy results from the layout graph back to the input graph:
- **Nodes**: `x, y, order, rank`. For compound nodes: also `width, height`
- **Edges**: `points`. If the edge has a label: also `x, y`
- **Graph**: `width, height`

---

## 16. Graph Library Requirements

The algorithm requires a graph library supporting:

### Node Operations
- `setNode(v, label)` — add/update node
- `removeNode(v)` — remove node and incident edges
- `node(v)` → label — get node label (mutable reference)
- `hasNode(v)` → bool
- `nodes()` → string[] — all node IDs
- `nodeCount()` → int
- `sources()` → string[] — nodes with no incoming edges

### Edge Operations
- `setEdge(v, w, label, name?)` — add/update edge
- `setEdge(edge, label)` — add/update edge from edge object
- `removeEdge(edge)` or `removeEdge(v, w)`
- `edge(edge)` or `edge(v, w)` → label — get edge label (mutable reference)
- `hasEdge(v, w)` → bool
- `edges()` → Edge[] — all edges
- `inEdges(v, w?)` → Edge[] — edges into v (optionally from w)
- `outEdges(v, w?)` → Edge[] — edges from v (optionally to w)
- `nodeEdges(v)` → Edge[] — all edges incident on v

### Adjacency
- `predecessors(v)` → string[]
- `successors(v)` → string[]
- `neighbors(v)` → string[] — predecessors + successors (undirected)

### Compound Graph Operations
- `setParent(v, parent)` — set node's parent
- `parent(v)` → string | undefined
- `children(v?)` → string[] — children of v, or root-level nodes if v omitted

### Graph Properties
- `setGraph(label)` — set graph-level label
- `graph()` → label — get graph-level label (mutable reference)
- `isMultigraph()` → bool
- `isCompound()` → bool
- `isDirected()` → bool

### Graph Algorithms (external)
- `preorder(graph, roots)` → string[] — pre-order DFS traversal
- `postorder(graph, roots)` → string[] — post-order DFS traversal

### Constructor Options
- `directed` — whether the graph is directed (default true)
- `multigraph` — whether multiple edges between same nodes are allowed
- `compound` — whether parent-child relationships are supported
- `setDefaultNodeLabel(fn)` — factory for default node labels

---

## 17. Configuration Reference

### User-Facing Options

| Option | Type | Default | Effect |
|---|---|---|---|
| `rankdir` | `"TB"\|"BT"\|"LR"\|"RL"` | `"TB"` | Direction of rank layout |
| `align` | `"UL"\|"UR"\|"DL"\|"DR"` | — | Use a specific BK alignment instead of median |
| `rankalign` | `"top"\|"center"\|"bottom"` | `"center"` | Vertical alignment of nodes within a rank |
| `nodesep` | float | 50 | Min horizontal gap between nodes |
| `edgesep` | float | 20 | Min horizontal gap between edges |
| `ranksep` | float | 50 | Min vertical gap between ranks |
| `marginx` | float | 0 | Left/right margin |
| `marginy` | float | 0 | Top/bottom margin |
| `acyclicer` | `"greedy"` | DFS | Cycle removal strategy |
| `ranker` | `"network-simplex"\|"tight-tree"\|"longest-path"` | `"network-simplex"` | Ranking algorithm |
| `debugTiming` | bool | false | Log timing for each phase |

### Per-Node Options

| Option | Type | Default | Effect |
|---|---|---|---|
| `width` | float | 0 | Node width |
| `height` | float | 0 | Node height |

### Per-Edge Options

| Option | Type | Default | Effect |
|---|---|---|---|
| `minlen` | int | 1 | Minimum edge span in ranks |
| `weight` | float | 1 | Edge weight (higher = shorter, more vertical) |
| `width` | float | 0 | Edge label width |
| `height` | float | 0 | Edge label height |
| `labelpos` | `"l"\|"c"\|"r"` | `"r"` | Edge label position |
| `labeloffset` | float | 10 | Label offset from edge |

### Extended Options (LayoutOptions)

| Option | Type | Default | Effect |
|---|---|---|---|
| `customOrder` | function | — | Custom node ordering function |
| `disableOptimalOrderHeuristic` | bool | false | Skip iterative crossing minimization |
| `constraints` | array | `[]` | Order constraints: `[{left, right}]` pairs forcing left < right |

---

## 18. Utility Algorithms

### 18.1 Unique ID Generation

Global counter-based: `prefix + (++counter)`. Must be globally unique within a layout run.

### 18.2 Dummy Node Creation

```
addDummyNode(graph, type, attrs, namePrefix):
    v = namePrefix
    while graph.hasNode(v):
        v = uniqueId(namePrefix)
    attrs.dummy = type
    graph.setNode(v, attrs)
    return v
```

### 18.3 Build Layer Matrix

Given a graph with `rank` and `order` on each node, produce a 2D array `layers[rank][order] = nodeId`.

### 18.4 Doubly Linked List

Used by the greedy FAS algorithm. Supports:
- `enqueue(entry)` — add to front (after sentinel)
- `dequeue()` → entry — remove from back (before sentinel); returns undefined if empty
- Entries track `_prev` and `_next` pointers

### 18.5 Apply With Chunking

For large arrays, `Math.min(...)` / `Math.max(...)` can exceed the call stack limit. The utility splits arrays larger than 65535 elements into chunks, applies the function to each chunk, then applies it to the chunk results.

### 18.6 Predecessor/Successor Weight Maps

Build maps of `{node: {neighbor: totalWeight}}` for each node, aggregating multi-edge weights. Used during ordering.
