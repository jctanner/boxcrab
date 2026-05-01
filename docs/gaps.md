# Gap Analysis: Specs vs. Current Implementation

Comparing specs (`dagre.md`, `mermaid.md`, `mermaid-cli.md`, `d3.md`, `elkjs.md`,
`katex.md`, `dompurify.md`) against the current codebase.

Items marked **DONE** were implemented. Remaining items ordered by priority.

---

## DONE

### ~~1. Node Intersect Clipping~~

Implemented: `intersect_rect`, `intersect_diamond`, `intersect_circle`, `intersect_node`
in `src/layout/mod.rs`. Edge endpoints now clip to actual node boundaries including
shape-aware clipping for diamonds and circles.

### ~~2. Network Simplex Ranking~~

Implemented: Full network simplex algorithm in `src/layout/sugiyama.rs` —
`longest_path_ranking`, `feasible_tight_tree`, `compute_low_lim`, `compute_cut_values`,
`leave_edge`, `enter_edge`, `network_simplex_rank`. Falls back to balanced heuristic
on failure.

---

### ~~3. Brandes-Köpf Coordinate Assignment~~

Implemented: Full 4-alignment BK algorithm in `src/layout/sugiyama.rs` —
`bk_vertical_alignment`, `bk_horizontal_compaction`, `bk_balance_alignments`,
`bk_find_type1_conflicts`, `bk_build_final_positions`, `bk_coordinate_assignment`.
Falls back to simple centering on failure.

---

## Layout Algorithm Gaps (from dagre.md)

### 4. Integrated Compound Graph Layout

**Dagre Phases 3, 8, 9 + Mermaid Section 13** — Nesting graph, parent dummy chains,
border segments, edge rewiring.

**Current**: Subgraphs laid out independently bottom-up, nested as opaque compound nodes.

**Dagre spec**: Structural edges between nesting root and subgraph border nodes. Dummy
nodes assigned to correct parent via LCA. Left/right border dummies per rank per subgraph.

**Mermaid spec** adds: Complex edge rewiring for clusters — edges targeting a cluster get
redirected to internal leaf nodes. Dummy node insertion at cluster boundaries. Recursive
dagre runs for nested clusters.

**Files**: `src/layout/mod.rs`, `src/layout/sugiyama.rs`

---

### 5. Improved Crossing Minimization

**Dagre Phase 10** — Full barycentric ordering with constraint support.

**Current**: Basic barycentric heuristic, 12 iterations, group-aware sorting.

**Missing**:
- Bilayer cross counting (Barth et al.) — O(|E| log |V|) accumulator tree
- Initial DFS ordering from rank-sorted nodes
- Subgraph constraint graph (Forster)
- "Keep best" tracking by cross count, stop after 4 iterations without improvement

**Files**: `src/layout/sugiyama.rs` (`minimize_crossings_grouped`)

---

### 6. Edge Label Space

**Dagre Phase 1** — Dedicate rank space for edge labels.

**Current**: Labels float at midpoints, can overlap nodes/edges.

**Spec**: Halve ranksep, double minlen, mark edge-label dummies at `labelRank`.

**Files**: `src/layout/sugiyama.rs`, `src/layout/mod.rs`

---

### 7. Self-Edge Support

**Dagre Phases 2.1, 12.1** — Self-edges (v == w).

**Current**: Not handled. Creates zero-length edges.

**Spec**: Stash before cycle removal, reintroduce after ordering, render as 5-point
control loop on the right side of the node.

---

## Rendering Gaps (from mermaid.md, d3.md)

### 8. Extended Node Shapes (~65 missing)

**Mermaid Section 16** — ~70 shapes with intersection functions.

**Current**: 5 shapes (rect, rounded, diamond, circle, flag).

**High-priority missing shapes** (commonly used in flowcharts):
- Stadium/pill `([text])` — rounded rectangle with semicircle ends
- Subroutine `[[text]]` — double-bordered rectangle
- Cylinder `[(text)]` — database shape
- Parallelogram `[/text/]` and `[\text\]`
- Trapezoid `[/text\]` and `[\text/]`
- Hexagon `{{text}}`
- Double circle `(((text)))`

**Lower-priority** (used in specialized diagrams):
- Lean shapes, tagged rectangles, window panes, curved trapezoids
- Icon shapes, image shapes, markdown-rendered shapes

Each shape needs both a **draw function** (for rendering) and an **intersection function**
(for edge clipping). Parser grammar updates needed for new shape syntax.

**Files**: `src/parser/grammar.pest`, `src/parser/mod.rs`, `src/renderer/shapes.rs`,
`src/renderer/export.rs`, `src/layout/mod.rs` (intersection functions)

---

### 9. Edge Curve Types (~6 missing)

**D3 Section 4 / Mermaid Section 17** — 8+ curve interpolation types.

**Current**: Cubic bezier and Catmull-Rom spline.

**Missing**:
- `curveBasis` — B-spline (smooth, passes near control points)
- `curveCardinal` — Cardinal spline with tension parameter
- `curveMonotoneX/Y` — Monotone interpolation (no overshoot)
- `curveNatural` — Natural cubic spline
- `curveStep` / `curveStepBefore` / `curveStepAfter` — Piecewise constant
- `curveBumpX` / `curveBumpY` — For tree links

Mermaid's default curve type depends on diagram type. For flowcharts it's typically
`curveBasis`.

**Files**: `src/renderer/shapes.rs`, `src/renderer/export.rs`

---

### 10. Arrowhead/Marker Types (~10 missing)

**Mermaid Section 18** — Marker SVG definitions.

**Current**: Single filled triangle arrowhead.

**Missing**:
- `arrow_circle` — filled/open circle
- `arrow_cross` — X mark
- `aggregation` — open diamond (UML)
- `composition` — filled diamond (UML)
- `dependency` — open arrow (UML)
- `lollipop` — circle on a stick
- Bidirectional arrows (`o--o`, `x--x`, `<-->`)

Each needs both egui (interactive) and tiny-skia (export) implementations.

**Files**: `src/renderer/shapes.rs`, `src/renderer/export.rs`, `src/parser/grammar.pest`

---

## Parser & Syntax Gaps (from mermaid.md)

### 11. Preprocessing Pipeline

**Mermaid Section 3** — Input normalization before parsing.

**Current**: Raw text passed directly to pest parser.

**Missing**:
- YAML frontmatter extraction (`---\ntitle: ...\n---`)
- Inline directive extraction (`%%{init: {...}}%%`)
- Comment stripping before parse (currently handled in grammar)
- HTML entity decoding in labels

**Files**: `src/parser/mod.rs`

---

### 12. Extended Edge Syntax

**Mermaid Section 24** — Full flowchart edge grammar.

**Current**: `-->`, `---`, `-.->`, `-.-`, `==>`, `===`

**Missing**:
- Multi-character length: `---->` (longer edges), `....>` (longer dotted)
- Bidirectional: `<-->`, `o--o`, `x--x`
- Text on edge: `-- text -->`, `== text ==>` (alternative to `|text|` syntax)
- `~~~` invisible link (no-render edge, layout-only)
- `linkStyle` directive for per-edge styling

**Files**: `src/parser/grammar.pest`, `src/parser/mod.rs`, `src/parser/ast.rs`

---

### 13. Class Shorthand Syntax

**Mermaid Section 24** — `:::` class application.

**Current**: Only `class` statement supported.

**Missing**: `A:::className` inline class application on node references.

**Files**: `src/parser/grammar.pest`, `src/parser/mod.rs`

---

## Configuration & Theming Gaps (from mermaid.md)

### 14. Configurable Layout Parameters

**Dagre Section 17 / Mermaid Section 9** — Per-graph configuration.

**Current**: Hard-coded constants (`LAYER_SPACING=60`, `NODE_SPACING=150`, `EDGE_SPACING=30`).

**Spec**: Configurable `nodesep`, `edgesep`, `ranksep`, `marginx`, `marginy`.
Mermaid adds 4-level config cascade (defaults < siteConfig < directives < currentConfig).

**Files**: `src/layout/mod.rs`, `src/parser/ast.rs`

---

### 15. Theme System

**Mermaid Section 10** — 5 built-in themes.

**Current**: White background, gray strokes, basic style/classDef support.

**Missing**: default, dark, forest, neutral, base themes with proper color palettes.
Each theme defines ~50 CSS variables for node fills, strokes, text colors, edge colors, etc.

**Files**: New `src/theme.rs` module

---

## Low Priority / Future Gaps

### 16. Additional Diagram Types

**Mermaid Section 5** — 19 diagram types total.

**Current**: Flowchart/graph only.

**Future candidates** (rough priority by user demand):
1. Sequence diagrams
2. Class diagrams
3. State diagrams
4. Entity-Relationship diagrams
5. Gantt charts
6. Pie charts
7. Mindmaps (requires d3-hierarchy tree layout)
8. Git graphs

Each diagram type needs its own parser, database, renderer, and styles module.

---

### 17. KaTeX Math Rendering

**katex.md** — LaTeX math notation in labels.

**Current**: Not supported.

**Scope**: Full compiler pipeline (lexer → macro expander → parser → build tree),
~5000 lines of logic + font metrics tables. Only needed if `$...$` math labels
are required.

---

### 18. ELK Layout Engine

**elkjs.md** — Advanced alternative to dagre.

**Current**: Not needed. Dagre-based layout is correct for flowcharts.

**When needed**: Class diagrams or architecture diagrams with explicit ports,
hierarchical layouts with INCLUDE_CHILDREN mode.

---

### 19. Text Sanitization

**dompurify.md** — HTML/SVG sanitization.

**Current**: No sanitization (labels rendered as-is).

**Minimum needed**: Escape `<`, `>`, `&` in user-provided text labels. Strip HTML tags
in strict mode. No need for full DOMPurify — native Rust doesn't have DOM clobbering risks.

---

### 20. Markdown Input Processing

**mermaid-cli.md Section 6** — Extract mermaid blocks from markdown files.

**Current**: Only accepts `.mmd` files with a single diagram.

**Spec**: Detect markdown input, scan for `` ```mermaid `` fenced code blocks, extract and
render each diagram independently. Output numbered files (`out-1.png`, `out-2.png`).

**Files**: `src/main.rs`, `src/parser/mod.rs`

---

### 21. Stdin/Stdout Support

**mermaid-cli.md Section 16** — Pipe-friendly CLI.

**Current**: File path argument only.

**Spec**: Read diagram from stdin (`-i -`), write PNG to stdout (`-o -`). Enables
`cat diagram.mmd | mmd-viewer --export -` workflows.

**Files**: `src/main.rs`

---

### 22. SVG Export

**mermaid-cli.md Sections 2, 8** — SVG output format.

**Current**: Only PNG export (via tiny-skia) and interactive viewer (egui).

**Spec**: SVG is the primary output format in mermaid-cli. Would require a new SVG
renderer alongside the existing tiny-skia and egui renderers.

**Files**: New `src/renderer/svg.rs`

---

### 23. Accessibility

**Mermaid Section 21** — ARIA attributes on SVG output.

**Current**: Not applicable (egui viewer, not SVG output).

**When needed**: If SVG export is added, include `role="img"`, `<title>`, `<desc>` from
`accTitle` / `accDescr` directives.

---

### 24. CSS File Injection

**mermaid-cli.md Section 11** — External CSS styling.

**Current**: Only inline `style` and `classDef` statements.

**Spec**: Load external `.css` files and apply as overrides on top of theme defaults.
Cascade: theme CSS → user CSS → inline styles.

**Files**: `src/main.rs`, `src/renderer/mod.rs`
