# Structurizr DSL Rendering Specification

A language-agnostic specification for parsing and rendering Structurizr DSL
files as native diagrams. Written for implementors building a renderer in Rust
or other languages without depending on Java or browser-based tooling.

Scoped to the C4 Model subset actually used by the ODH corpus (69 files).
Full DSL features are documented where relevant for future extension.

---

## Table of Contents

1. [Overview](#1-overview)
2. [High-Level Pipeline](#2-high-level-pipeline)
3. [Lexical Analysis](#3-lexical-analysis)
4. [Grammar and Parsing](#4-grammar-and-parsing)
5. [Workspace Model](#5-workspace-model)
6. [Element Types](#6-element-types)
7. [Relationships](#7-relationships)
8. [Views](#8-views)
9. [View Resolution](#9-view-resolution)
10. [Styles and Theming](#10-styles-and-theming)
11. [Tag System](#11-tag-system)
12. [Layout](#12-layout)
13. [Shape Rendering](#13-shape-rendering)
14. [Relationship Rendering](#14-relationship-rendering)
15. [Text Rendering](#15-text-rendering)
16. [Color System](#16-color-system)
17. [Coordinate System and Viewport](#17-coordinate-system-and-viewport)
18. [Output Formats](#18-output-formats)
19. [ODH Corpus Profile](#19-odh-corpus-profile)
20. [Full DSL Feature Catalog](#20-full-dsl-feature-catalog)

---

## 1. Overview

Structurizr DSL is a text-based language for describing software architecture
using the C4 Model. A `.dsl` file defines a **workspace** containing a
**model** (elements and relationships) and **views** (diagrams rendered from
subsets of the model with styling).

The C4 Model defines four abstraction levels:

```
Level 1: System Context  — People and software systems
Level 2: Container        — Deployable units within a system
Level 3: Component        — Internal structure within a container
Level 4: Code             — Classes/functions (not in DSL)
```

Each view type corresponds to one level, showing the focal element and its
immediate context (people, systems, containers, or components that interact
with it).

The fundamental rendering flow:

```
DSL Text → Lex → Parse → Workspace Model
  → Select View → Resolve Elements → Apply Styles
  → Layout → Render Shapes + Edges + Labels → Output
```

### 1.1 Design Principles

- **Model-first**: The DSL defines a model, then views select subsets of it.
  A single model produces multiple diagrams.
- **Tag-driven styling**: Elements get default tags from their type
  (`"Element"`, `"Person"`, `"Software System"`, etc.) plus user-defined tags.
  Styles are matched by tag, not by element identity.
- **Convention over configuration**: Shapes, colors, and layout have sensible
  defaults. A minimal DSL file with no styles block still produces a readable
  diagram.

---

## 2. High-Level Pipeline

### 2.1 Pipeline Stages

```
1. Lex         — Tokenize DSL text into tokens
2. Parse       — Build workspace AST from token stream
3. Build Model — Construct element/relationship graph
4. Resolve     — For each view, determine which elements to include
5. Style       — Resolve tag-based styles for each visible element
6. Layout      — Assign positions to elements and route edges
7. Render      — Draw shapes, edges, and labels to output format
```

### 2.2 Data Flow

```
DSL Text
  │
  ▼
Tokens: [workspace, {, model, {, person, "Name", ...}]
  │
  ▼
Workspace {
    Model {
        elements: [Person, SoftwareSystem, Container, Component, ...]
        relationships: [(source, target, desc, tech), ...]
    }
    Views {
        view_definitions: [SystemContext, Container, Component, ...]
        styles: { element_styles, relationship_styles }
    }
}
  │
  ▼  (per view)
ResolvedView {
    elements: [(element, style, position), ...]
    relationships: [(relationship, style, route), ...]
}
  │
  ▼
Rendered Output (SVG, PNG, or native widget tree)
```

---

## 3. Lexical Analysis

### 3.1 Character Set

DSL files are UTF-8 text. No BOM handling is required.

### 3.2 Token Types

```
Token:
    Keyword     — workspace, model, views, styles, person, softwareSystem,
                   container, component, element, group, include, exclude,
                   autoLayout, systemContext, systemlandscape, dynamic,
                   deployment, filtered, image, custom, configuration,
                   branding, terminology, theme, themes, properties,
                   perspectives, shape, background, stroke, strokeWidth,
                   color, colour, icon, opacity, border, fontSize, width,
                   height, metadata, description, thickness, dashed,
                   routing, style, position, enterprise, deploymentNode,
                   infrastructureNode, containerInstance,
                   softwareSystemInstance, healthCheck, relationship
    Identifier  — [a-zA-Z_][a-zA-Z0-9_-]*
    String      — "..." (double-quoted, supports escaped quotes \")
    Arrow       — ->
    OpenBrace   — {
    CloseBrace  — }
    Wildcard    — *
    Directive   — !include, !docs, !adrs, !constant, !ref, !identifiers,
                   !impliedRelationships, !extend, !plugin, !script
    Comment     — // or # to end of line; /* ... */ multi-line
    Newline     — line terminator (significant for statement boundaries)
```

### 3.3 Tokenization Rules

1. Skip whitespace (space, tab) within a line
2. Comments: `//` or `#` to end of line; `/* ... */` for multi-line
3. Strings: `"` opens, `"` closes. Internal `\"` is an escaped quote.
   Strings may span multiple lines.
4. The arrow `->` is a single token
5. Keywords are case-insensitive for the core set (`workspace`, `model`,
   `person`, etc.)
6. Identifiers and string literals are case-sensitive
7. Newlines are significant: they terminate statements. A statement is one
   logical line. `{` and `}` are both statement terminators and block
   delimiters.

### 3.4 Line-Oriented Parsing

The DSL is primarily line-oriented. Each non-blank, non-comment line is one
statement. The tokenizer splits each line into tokens. Braces `{` and `}` may
appear on the same line as other tokens or on their own line:

```
// These are equivalent:
person "Alice" "A user" {
    ...
}

person "Alice" "A user"
{
    ...
}
```

---

## 4. Grammar and Parsing

### 4.1 Top-Level Grammar

```
workspace      = "workspace" [name] [description] "{" workspace_body "}"
workspace_body = { model | views | configuration_block }*

name           = STRING
description    = STRING
```

### 4.2 Model Grammar

```
model          = "model" "{" model_body "}"
model_body     = { person_def | system_def | relationship | group_def
                 | enterprise_def | deployment_env | ref_stmt
                 | constant_def }*

person_def     = [IDENT "="] "person" STRING [STRING] [STRING]
                 ["{" element_body "}"]

system_def     = [IDENT "="] "softwareSystem" STRING [STRING] [STRING]
                 ["{" system_body "}"]

system_body    = { container_def | relationship | group_def }*

container_def  = [IDENT "="] "container" STRING [STRING] [STRING] [STRING]
                 ["{" container_body "}"]

container_body = { component_def | relationship | group_def }*

component_def  = [IDENT "="] "component" STRING [STRING] [STRING] [STRING]
                 ["{" component_body "}"]

component_body = { relationship | properties_block | perspectives_block }*
```

### 4.3 Positional Parameters

Element definitions use positional string parameters:

| Element | Param 1 | Param 2 | Param 3 | Param 4 |
|---------|---------|---------|---------|---------|
| person | name | description | tags | — |
| softwareSystem | name | description | tags | — |
| container | name | description | technology | tags |
| component | name | description | technology | tags |

Tags in the positional parameter are comma-separated within a single string:
`"External,Database"`.

### 4.4 Identifier Assignment

Elements can be assigned to identifiers for relationship references:

```
identifier = element_keyword "Name" ...
```

Identifier rules:
- Pattern: `[a-zA-Z_][a-zA-Z0-9_-]*`
- By default, identifiers are globally scoped (flat)
- With `!identifiers hierarchical`, identifiers are scoped to their parent

### 4.5 Relationship Grammar

```
relationship = IDENT "->" IDENT [STRING] [STRING] [STRING]
```

Parameters: `source -> destination [description] [technology] [tags]`

Relationships can appear:
- At model level (between any elements)
- Inside element blocks (source is the containing element)
- Inside view blocks (for dynamic views)

### 4.6 Views Grammar

```
views          = "views" "{" views_body "}"
views_body     = { view_def | styles_block | configuration | theme_def }*

view_def       = system_context_view | container_view | component_view
               | system_landscape_view | dynamic_view | deployment_view
               | filtered_view | image_view | custom_view

system_context_view = "systemContext" IDENT [STRING] [STRING]
                      "{" view_body "}"

container_view = "container" IDENT [STRING] [STRING]
                 "{" view_body "}"

component_view = "component" IDENT [STRING] [STRING]
                 "{" view_body "}"

system_landscape_view = "systemlandscape" [STRING] [STRING]
                        "{" view_body "}"

view_body      = { include_stmt | exclude_stmt | autolayout_stmt
                 | animation_block | title_stmt | description_stmt
                 | properties_block }*

include_stmt   = "include" (WILDCARD | IDENT | expression)+
exclude_stmt   = "exclude" (IDENT | expression)+
autolayout_stmt = "autoLayout" [direction] [rank_sep] [node_sep]
direction      = "tb" | "bt" | "lr" | "rl"
rank_sep       = NUMBER
node_sep       = NUMBER
```

### 4.7 Styles Grammar

```
styles_block   = "styles" "{" styles_body "}"
styles_body    = { element_style | relationship_style }*

element_style  = "element" STRING "{" element_style_body "}"
element_style_body = { shape_prop | background_prop | color_prop
                     | stroke_prop | stroke_width_prop | border_prop
                     | opacity_prop | font_size_prop | width_prop
                     | height_prop | icon_prop | metadata_prop
                     | description_prop }*

relationship_style = "relationship" STRING "{" rel_style_body "}"
rel_style_body = { thickness_prop | color_prop | dashed_prop
                 | opacity_prop | routing_prop | style_prop
                 | font_size_prop | width_prop | position_prop }*
```

---

## 5. Workspace Model

### 5.1 Data Structures

After parsing, the workspace is represented as:

```
Workspace:
    name:           Option<String>
    description:    Option<String>
    model:          Model
    views:          Views

Model:
    people:         Vec<Person>
    software_systems: Vec<SoftwareSystem>
    deployment_envs:  Vec<DeploymentEnvironment>
    relationships:  Vec<Relationship>  // all relationships across model

Views:
    system_landscape_views: Vec<SystemLandscapeView>
    system_context_views:   Vec<SystemContextView>
    container_views:        Vec<ContainerView>
    component_views:        Vec<ComponentView>
    dynamic_views:          Vec<DynamicView>
    deployment_views:       Vec<DeploymentView>
    filtered_views:         Vec<FilteredView>
    styles:                 Styles
    configuration:          ViewConfiguration
```

### 5.2 Element Hierarchy

```
Person
    tags: Vec<String>
    properties: HashMap<String, String>

SoftwareSystem
    containers: Vec<Container>
    tags: Vec<String>

Container
    components: Vec<Component>
    technology: Option<String>
    tags: Vec<String>

Component
    technology: Option<String>
    tags: Vec<String>
```

### 5.3 Identifier Registry

The parser maintains a map from identifiers to elements:

```
identifiers: HashMap<String, ElementRef>
```

Where `ElementRef` is a reference to any model element. This is used to
resolve relationship sources/targets and view element references.

---

## 6. Element Types

### 6.1 Person

Represents a human user or actor.

```
person "Name" ["Description"] ["Tags"]
```

- Default tags: `"Element"`, `"Person"`
- Default shape: `Person` (stick figure)
- Rendered as a simplified human silhouette with name below

### 6.2 Software System

The highest level of abstraction — a separately deployable thing.

```
softwareSystem "Name" ["Description"] ["Tags"]
```

- Default tags: `"Element"`, `"Software System"`
- Default shape: `Box` (rounded rectangle)
- May contain containers (Level 2 decomposition)

### 6.3 Container

A deployable unit within a software system (web app, database, microservice).

```
container "Name" ["Description"] ["Technology"] ["Tags"]
```

- Default tags: `"Element"`, `"Container"`
- Default shape: `Box`
- Technology is displayed as a subtitle (e.g., "Java Spring", "PostgreSQL")
- May contain components (Level 3 decomposition)

### 6.4 Component

An internal grouping within a container (module, service class, etc.).

```
component "Name" ["Description"] ["Technology"] ["Tags"]
```

- Default tags: `"Element"`, `"Component"`
- Default shape: `Box`
- Technology displayed as subtitle

### 6.5 Custom Element

For elements that don't fit the C4 hierarchy.

```
element "Name" ["Metadata"] ["Description"] ["Tags"]
```

- Default tags: `"Element"`
- Used in custom views

### 6.6 Group

Visual grouping of elements (no structural significance in the model).

```
group "Group Name" {
    // element definitions or references
}
```

- Rendered as a dashed border around contained elements
- Does not affect relationship routing

---

## 7. Relationships

### 7.1 Definition

```
source -> destination ["Description"] ["Technology"] ["Tags"]
```

- Source and destination are identifiers referencing model elements
- Description: human-readable label (e.g., "Sends inference requests")
- Technology: protocol/mechanism (e.g., "HTTPS/443", "gRPC/8085")
- Tags: comma-separated, default tag is `"Relationship"`

### 7.2 Implied Relationships

When `!impliedRelationships` is set, the system auto-creates parent-level
relationships from child-level ones. For example, if Component A in
Container X calls Component B in Container Y, an implied relationship is
created between Container X and Container Y, and between their parent
Software Systems.

### 7.3 Relationship Rendering

Relationships are rendered as labeled arrows between elements. The label
typically shows description on the first line and technology (in brackets or
smaller text) on the second.

```
┌──────────┐         "Sends requests"        ┌──────────┐
│  Client   │ ──────────────────────────────→ │  Server   │
│           │         [HTTPS/443]             │           │
└──────────┘                                  └──────────┘
```

---

## 8. Views

### 8.1 System Context View

Shows a single software system and everything that connects to it.

```
systemContext <system_identifier> ["key"] ["description"] {
    include *
    autoLayout [direction] [rankSep] [nodeSep]
}
```

**Included elements when `include *`:**
- The focal software system
- All people that have relationships with it
- All other software systems that have relationships with it
- All relationships between included elements

**Not included:**
- Internal containers/components of the focal system
- Internal containers/components of other systems

### 8.2 Container View

Shows the containers within a software system and external context.

```
container <system_identifier> ["key"] ["description"] {
    include *
    autoLayout [direction] [rankSep] [nodeSep]
}
```

**Included elements when `include *`:**
- All containers within the focal software system
- All people that have relationships with any container
- All external software systems that have relationships with any container
- All relationships between included elements

**Not included:**
- The focal software system itself (it becomes the boundary box)
- Internal components of containers

### 8.3 Component View

Shows components within a container.

```
component <container_identifier> ["key"] ["description"] {
    include *
    autoLayout [direction] [rankSep] [nodeSep]
}
```

**Included elements when `include *`:**
- All components within the focal container
- All other containers in the same system that have relationships
- All people and external systems with relationships
- All relationships between included elements

### 8.4 System Landscape View

Shows all people and software systems in the model.

```
systemlandscape ["key"] ["description"] {
    include *
    autoLayout
}
```

### 8.5 Dynamic View

Shows a sequence of interactions.

```
dynamic <scope> ["key"] ["description"] {
    source -> destination "Description"
    ...
    autoLayout
}
```

Scope can be `*` (unscoped), a software system, or a container.

### 8.6 Deployment View

Shows deployment infrastructure.

```
deployment <system> <environment> ["key"] ["description"] {
    include *
    autoLayout
}
```

### 8.7 Include/Exclude Expressions

Views support filtering with these expressions:

| Expression | Meaning |
|-----------|---------|
| `*` | All elements visible at this view's abstraction level |
| `identifier` | A specific element |
| `element.tag==X` | Elements with tag X |
| `element.tag!=X` | Elements without tag X |
| `relationship==*` | All relationships |
| `relationship.tag==X` | Relationships with tag X |

---

## 9. View Resolution

### 9.1 Element Visibility Algorithm

For a given view, determine which elements and relationships to render:

```
resolve_view(view, model):
    elements = {}
    
    // Apply includes
    for each include_expr in view.includes:
        if include_expr == "*":
            elements = default_elements_for_view_type(view, model)
        elif include_expr is identifier:
            elements.insert(lookup(include_expr))
        elif include_expr is tag_expression:
            elements.extend(filter_by_tag(model, include_expr))
    
    // Apply excludes
    for each exclude_expr in view.excludes:
        elements.remove(matching_elements(exclude_expr))
    
    // Resolve relationships
    relationships = model.relationships.filter(|r|
        elements.contains(r.source) && elements.contains(r.target)
    )
    
    return (elements, relationships)
```

### 9.2 Default Elements by View Type

```
system_context(system):
    - The system itself
    - All people connected to system (directly or via children)
    - All other software systems connected to system
    
container(system):
    - All containers in system
    - All people connected to any container
    - All external software systems connected to any container

component(container):
    - All components in container
    - All other containers in the same system connected to any component
    - All people connected to any component
    - All external software systems connected to any component
```

### 9.3 Relationship Propagation

When a view shows elements at a higher abstraction than where the
relationship is defined, relationships are propagated upward:

- If Container A → Container B exists, and a System Context view shows
  System X (containing A) and System Y (containing B), a relationship
  System X → System Y is shown.
- Duplicate propagated relationships are merged.
- Description and technology from the most specific relationship are used.

---

## 10. Styles and Theming

### 10.1 Style Resolution

Styles are resolved per-element using tag matching:

```
resolve_style(element, styles):
    result = default_style()
    
    for tag in element.all_tags():  // includes default + custom tags
        if styles.has_element_style(tag):
            result.merge(styles.element_style(tag))
    
    return result
```

Later tag matches override earlier ones. The order is: default tags first
(`"Element"`, then the type tag like `"Person"`), then custom tags in
declaration order.

### 10.2 Element Style Properties

```
ElementStyle:
    shape:       Shape         // Visual shape (default: Box)
    background:  Color         // Fill color (default: #438dd5)
    color:       Color         // Text color (default: #ffffff)
    stroke:      Color         // Border color (default: derived from background)
    stroke_width: u32          // Border width 1-10 (default: 2)
    border:      BorderStyle   // solid | dashed | dotted (default: solid)
    opacity:     u32           // 0-100 (default: 100)
    font_size:   u32           // Pixels (default: 24)
    width:       u32           // Fixed width in pixels (default: auto)
    height:      u32           // Fixed height in pixels (default: auto)
    icon:        Option<String> // URL or path to icon image
    metadata:    bool          // Show metadata/technology (default: true)
    description: bool          // Show description (default: true)
```

### 10.3 Relationship Style Properties

```
RelationshipStyle:
    thickness:  u32            // Line width (default: 2)
    color:      Color          // Line color (default: #707070)
    dashed:     bool           // Dashed line (default: true)
    opacity:    u32            // 0-100 (default: 100)
    routing:    Routing        // Direct | Orthogonal | Curved (default: Direct)
    style:      LineStyle      // Solid | Dashed | Dotted (default: Dashed)
    font_size:  u32            // Label font size (default: 24)
    width:      u32            // Label max width (default: 200)
    position:   u32            // Label position 0-100 along edge (default: 50)
```

### 10.4 Default Styles

When no styles block is provided, the Structurizr defaults are:

| Element Type | Background | Color | Shape |
|-------------|-----------|-------|-------|
| Person | #08427b | #ffffff | Person |
| Software System | #1168bd | #ffffff | Box |
| Container | #438dd5 | #ffffff | Box |
| Component | #85bbf0 | #000000 | Box |

Relationships default to `#707070`, dashed, thickness 2.

### 10.5 Theme Support

```
theme default                    // Structurizr default theme
theme <url>                      // Load theme from URL
themes <url1> <url2> ...         // Multiple themes, applied in order
```

Themes provide style definitions loaded from JSON. They are applied before
the workspace's own styles block, so local styles override theme styles.

---

## 11. Tag System

### 11.1 Default Tags

Every element receives default tags based on its type:

| Element Type | Default Tags |
|-------------|-------------|
| Person | `"Element"`, `"Person"` |
| Software System | `"Element"`, `"Software System"` |
| Container | `"Element"`, `"Container"` |
| Component | `"Element"`, `"Component"` |
| Deployment Node | `"Element"`, `"Deployment Node"` |
| Infrastructure Node | `"Element"`, `"Infrastructure Node"` |
| Relationship | `"Relationship"` |

### 11.2 Custom Tags

Additional tags are specified as the last positional parameter:

```
softwareSystem "Prometheus" "Metrics" "External"
// Tags: ["Element", "Software System", "External"]

container "DB" "Storage" "PostgreSQL" "Database,Primary"
// Tags: ["Element", "Container", "Database", "Primary"]
```

Multiple custom tags are comma-separated within the string.

### 11.3 Style Matching

Styles are defined per-tag. An element with multiple tags receives merged
styles from all matching tag rules. Properties from later-matching tags
override earlier ones.

Example:
```
styles {
    element "Software System" {
        background #1168bd
    }
    element "External" {
        background #999999
    }
}
```

A software system tagged `"External"` gets background `#999999` because
`"External"` is applied after `"Software System"`.

---

## 12. Layout

### 12.1 Auto Layout

The `autoLayout` directive enables automatic positioning:

```
autoLayout [direction] [rankSeparation] [nodeSeparation]
```

| Parameter | Default | Description |
|-----------|---------|-------------|
| direction | `tb` | `tb` (top→bottom), `bt`, `lr` (left→right), `rl` |
| rankSeparation | 300 | Pixels between ranks (layers) |
| nodeSeparation | 300 | Pixels between nodes in the same rank |

### 12.2 Layout Algorithm

Structurizr uses a hierarchical (Sugiyama-style) layout:

```
1. Rank Assignment
   - Assign each node to a rank (layer) based on dependency depth
   - Direction determines axis: tb/bt = vertical ranks, lr/rl = horizontal

2. Ordering
   - Within each rank, order nodes to minimize edge crossings
   - Barycenter heuristic: position node at average of connected nodes' positions

3. Coordinate Assignment
   - Assign x,y coordinates respecting rankSeparation and nodeSeparation
   - Center nodes within their rank

4. Edge Routing
   - Route edges between node connection points
   - For multi-rank edges, add bend points at rank boundaries
```

### 12.3 Node Sizing

Node dimensions are determined by content:

```
node_width  = max(min_width, text_width + 2 * padding)
node_height = name_height + description_height + technology_height + 2 * padding

// Typical minimums:
min_width  = 200
min_height = 120
padding    = 20
```

Person shapes have additional height for the head circle.

### 12.4 Boundary Boxes

In container and component views, the focal element (software system or
container) is rendered as a boundary box enclosing its children:

```
┌─────────────── System Name ───────────────┐
│                                           │
│  ┌───────────┐          ┌───────────┐     │
│  │ Container │ ───────→ │ Container │     │
│  │     A     │          │     B     │     │
│  └───────────┘          └───────────┘     │
│                                           │
└───────────────────────────────────────────┘
```

The boundary box:
- Has a dashed border
- Shows the system/container name as a title
- Is sized to fit all contained elements plus margin
- Is NOT a layout node — it wraps the results of the internal layout

---

## 13. Shape Rendering

### 13.1 Shape Catalog

```
Shape:
    Box                   — Rectangle with slightly rounded corners
    RoundedBox            — Rectangle with prominently rounded corners
    Circle                — Circle
    Ellipse               — Ellipse
    Hexagon               — Regular hexagon
    Cylinder              — 3D cylinder (database icon)
    Component             — Component notation (box with two small rectangles)
    Person                — Simplified human figure
    Robot                 — Robot icon
    Folder                — Folder icon
    WebBrowser            — Browser window chrome
    MobileDevicePortrait  — Phone outline (vertical)
    MobileDeviceLandscape — Phone outline (horizontal)
    Pipe                  — Horizontal pipe/tube
    Diamond               — Diamond/rhombus
```

### 13.2 Box Shape (Default)

The default shape for most elements. Anatomy:

```
┌──────────────────────────┐
│                          │
│       «stereotype»       │   ← optional, from element type
│        Element Name      │   ← bold, primary label
│       [Technology]       │   ← technology in brackets, smaller
│                          │
│    Description text      │   ← smaller, may wrap
│    that may wrap         │
│                          │
└──────────────────────────┘
```

Rendering details:
- Corner radius: 5px
- Border: 2px solid (color from stroke style)
- Fill: background color
- Text: centered, color from style
- Name: bold, font_size from style
- Technology: regular weight, ~75% of font_size, in square brackets
- Description: regular weight, ~75% of font_size
- Padding: 20px all sides

### 13.3 Person Shape

A simplified human silhouette above a box:

```
     ┌───┐
     │   │        ← head (circle)
     └───┘
    ┌─────┐
    │     │       ← body (trapezoid or rounded shape)
    └─────┘
   Person Name    ← name below body
  [Description]   ← description below name
```

Rendering details:
- Head: circle, diameter ~50px, filled with element background color
- Body: rounded rectangle or trapezoid, ~80px wide × 60px tall
- Name and description below the figure
- Total shape height includes silhouette + text

### 13.4 Cylinder Shape

A 3D cylinder representing databases or storage:

```
    ╭──────────╮
    │          │   ← top ellipse
    ├──────────┤
    │          │
    │   Name   │   ← body rectangle
    │  [Tech]  │
    │          │
    ╰──────────╯   ← bottom ellipse
```

Rendering details:
- Top and bottom: elliptical arcs
- Ellipse height: ~20px
- Body: rectangle connecting the ellipses
- Text centered in the body
- Fill: background color for all surfaces

### 13.5 Hexagon Shape

Regular hexagon:

```
      ╱──────╲
     ╱        ╲
    │   Name   │
    │  [Tech]  │
     ╲        ╱
      ╲──────╱
```

### 13.6 WebBrowser Shape

Browser window chrome with content area:

```
┌──────────────────────┐
│ ● ● ●  ┌──────────┐ │  ← title bar with dots and address bar
├─────────┴──────────┤─┤
│                      │
│       Name           │  ← content area
│      [Tech]          │
│                      │
└──────────────────────┘
```

### 13.7 RoundedBox Shape

Like Box but with larger corner radius (~10-15px).

### 13.8 Shape Intersection Points

Each shape must provide an intersection function for edge routing. Given
a line from the center of the shape to an external point, the function
returns the point where the line crosses the shape boundary.

```
intersect(shape_bounds, external_point) → boundary_point
```

Common implementations:
- **Rectangle**: line-rectangle intersection (4 edge tests)
- **Circle/Ellipse**: line-ellipse parametric intersection
- **Hexagon**: line-polygon intersection (6 edges)
- **Cylinder**: line-rectangle intersection on the body portion
- **Person**: line-rectangle intersection on a bounding box

---

## 14. Relationship Rendering

### 14.1 Arrow Anatomy

```
┌────────┐                                    ┌────────┐
│ Source  │──── "Description" ──────────────→ │ Target  │
│        │     [Technology]                   │         │
└────────┘                                    └────────┘
         ╰─ start point    label      end point ─╯
              (on shape     (midpoint    (on shape
               boundary)    of edge)      boundary)
```

### 14.2 Edge Routing

```
1. Compute start point: intersect(source_shape, target_center)
2. Compute end point: intersect(target_shape, source_center)
3. For direct routing: straight line between points
4. For orthogonal routing: axis-aligned segments with bends
5. For curved routing: smooth curve through control points
```

### 14.3 Arrowheads

The default arrowhead is a filled triangle at the destination end:

```
Triangle arrowhead:
    Base width: 10px
    Height: 10px
    Filled with relationship color
    Oriented to match the final edge segment direction
```

No arrowhead at the source end (relationships are unidirectional in C4).

### 14.4 Edge Labels

Labels are positioned at the midpoint of the edge path:

```
label_position:
    x = (start.x + end.x) / 2
    y = (start.y + end.y) / 2

label_layout:
    Line 1: description (regular weight)
    Line 2: [technology] (in brackets, smaller font)
```

For multi-segment edges, the label is placed at the midpoint of the
total path length.

### 14.5 Line Styles

| Style | SVG | Description |
|-------|-----|-------------|
| Solid | — | Continuous line |
| Dashed | `stroke-dasharray: 5,5` | Default for relationships |
| Dotted | `stroke-dasharray: 2,2` | Short dashes |

---

## 15. Text Rendering

### 15.1 Element Labels

Each element displays up to four text sections:

```
1. Stereotype    — «Person» or similar (optional, from type)
2. Name          — Primary label (bold)
3. Technology    — [Technology] in brackets (if defined)
4. Description   — Full description text (may wrap)
```

### 15.2 Text Layout

```
measure_text(text, font_family, font_size, bold) → (width, height)

layout_element_text(element, max_width):
    lines = []
    
    if show_stereotype:
        lines.append(wrap("«" + type_name + "»", max_width, small_font))
    
    lines.append(wrap(element.name, max_width, bold_font))
    
    if element.technology:
        lines.append(wrap("[" + technology + "]", max_width, small_font))
    
    if show_description and element.description:
        lines.append(wrap(element.description, max_width, small_font))
    
    total_height = sum(line.height for line in lines) + spacing * (len - 1)
    return lines, total_height
```

### 15.3 Text Wrapping

Long text wraps within the element's content area:

```
wrap(text, max_width, font):
    words = text.split_whitespace()
    lines = []
    current_line = ""
    
    for word in words:
        test = current_line + " " + word
        if measure_text(test, font).width > max_width:
            lines.push(current_line)
            current_line = word
        else:
            current_line = test
    
    lines.push(current_line)
    return lines
```

### 15.4 Font Configuration

Default fonts:
- Family: `"Open Sans"`, fallback `"Arial"`, `"Helvetica"`, `sans-serif`
- Name size: 24px bold
- Technology/description size: 18px regular
- Relationship label: 20px regular

These can be overridden by the `fontSize` style property, which sets the
name size. Technology and description sizes are derived as ~75% of the
name size.

---

## 16. Color System

### 16.1 Color Formats

Colors are specified as:
- Hex RGB: `#rrggbb` (e.g., `#438dd5`)
- Hex RGBA: `#rrggbbaa` (e.g., `#438dd580` for 50% opacity)
- Named CSS colors: `white`, `black`, etc.

### 16.2 Color Derivation

When stroke color is not explicitly set, it is derived from the background
color by darkening it:

```
derive_stroke(background):
    // Darken by ~20%
    r = background.r * 0.8
    g = background.g * 0.8
    b = background.b * 0.8
    return Color(r, g, b)
```

### 16.3 Opacity

Element and relationship opacity (0-100) is applied as alpha to all
colors (fill, stroke, text):

```
apply_opacity(color, opacity):
    alpha = opacity / 100.0
    return color.with_alpha(alpha)
```

---

## 17. Coordinate System and Viewport

### 17.1 Coordinate Space

Layout operates in abstract pixel coordinates:
- Origin (0, 0) is top-left
- X increases rightward
- Y increases downward
- All measurements are in logical pixels

### 17.2 Viewport Calculation

After layout, the viewport is computed to fit all content:

```
compute_viewport(elements, relationships, padding):
    min_x = min(e.x - e.width/2 for e in elements)
    min_y = min(e.y - e.height/2 for e in elements)
    max_x = max(e.x + e.width/2 for e in elements)
    max_y = max(e.y + e.height/2 for e in elements)
    
    // Include edge labels in bounds
    for r in relationships:
        expand bounds to include label position
    
    viewport = {
        x: min_x - padding,
        y: min_y - padding,
        width: (max_x - min_x) + 2 * padding,
        height: (max_y - min_y) + 2 * padding,
    }
```

### 17.3 Scaling

For SVG output, set the viewBox to the viewport and let the SVG scale
to fit its container. For raster/native output, apply a scale factor
based on the target DPI or window size.

---

## 18. Output Formats

### 18.1 SVG Output

```xml
<svg xmlns="http://www.w3.org/2000/svg"
     viewBox="x y width height">
  <defs>
    <marker id="arrowhead" ...>
      <path d="M 0 0 L 10 5 L 0 10 z" />
    </marker>
  </defs>
  
  <!-- Boundary boxes (if container/component view) -->
  <g class="boundaries">
    <rect class="boundary" ... />
    <text class="boundary-label" ... />
  </g>
  
  <!-- Relationship edges -->
  <g class="relationships">
    <g class="relationship">
      <path d="..." marker-end="url(#arrowhead)" />
      <text class="relationship-label">Description</text>
      <text class="relationship-technology">[Technology]</text>
    </g>
  </g>
  
  <!-- Element shapes -->
  <g class="elements">
    <g class="element" transform="translate(x,y)">
      <!-- shape-specific SVG (rect, path, etc.) -->
      <text class="element-name">Name</text>
      <text class="element-technology">[Tech]</text>
      <text class="element-description">Desc</text>
    </g>
  </g>
</svg>
```

### 18.2 Native Widget Tree (for Rust GUI)

For a native renderer (e.g., using wgpu, skia, or a widget toolkit), the
output is a positioned scene graph:

```
Scene:
    background: Color
    elements: Vec<RenderedElement>
    relationships: Vec<RenderedRelationship>
    boundaries: Vec<RenderedBoundary>

RenderedElement:
    bounds: Rect           // x, y, width, height
    shape: Shape
    fill: Color
    stroke: Color
    stroke_width: f32
    opacity: f32
    texts: Vec<TextBlock>  // name, tech, description

RenderedRelationship:
    points: Vec<Point>     // path points
    color: Color
    thickness: f32
    line_style: LineStyle
    arrowhead: ArrowHead
    label: TextBlock
    technology: TextBlock

RenderedBoundary:
    bounds: Rect
    label: TextBlock
    border_style: Dashed
```

### 18.3 PNG Output

Render SVG or native scene to a raster image. Recommended default DPI: 144
(2x for retina). Minimum resolution: 72 DPI.

---

## 19. ODH Corpus Profile

Analysis of the 69 `.dsl` files in `odh.diagrams/` reveals a focused subset
of the full Structurizr DSL. An implementation targeting this corpus can
start with these features.

### 19.1 Constructs Used

| Feature | Usage | Priority |
|---------|-------|----------|
| `workspace` | 69/69 files | Required |
| `model` | 69/69 files | Required |
| `views` | 69/69 files | Required |
| `styles` | 69/69 files | Required |
| `person` | 152 instances | Required |
| `softwareSystem` | 707 instances | Required |
| `container` | 574 instances | Required |
| `component` | 100 instances | Required |
| `->` relationships | All files | Required |
| `systemContext` view | 69 views | Required |
| `container` view | 69 views | Required |
| `component` view | 7 views | Required |
| `include *` | All views | Required |
| `autoLayout` (no args) | All views | Required |
| `element` styles | All files | Required |
| `!include` / `!script` | 0 uses | Not needed |
| `configuration` | 0 uses | Not needed |
| `deployment` views | 0 uses | Not needed |
| `dynamic` views | 0 uses | Not needed |
| `filtered` views | 0 uses | Not needed |

### 19.2 Tags Used

64 unique tags across the corpus. The most common:

| Tag | Semantics | Typical Color |
|-----|-----------|---------------|
| `External` | External dependency | #999999 (grey) |
| `External Service` | External cloud service | #f5a623 (orange) |
| `External Platform` | External platform | #999999 |
| `Internal RHOAI` | Internal RHOAI component | #7ed321 (green) |
| `Internal Platform` | Internal platform | #7ed321 |
| `Database` | Database | shape: Cylinder |
| `Person` | Human actor | shape: Person |
| `Hardware` | Hardware component | shape: Hexagon |
| `WebBrowser` / `WebApp` | Web interface | shape: WebBrowser |

### 19.3 Style Properties Used

Only a subset of available style properties appears:

| Property | Frequency | Notes |
|----------|-----------|-------|
| `background` | All files | Hex color |
| `color` | All files | Text color, always hex |
| `shape` | ~50% of files | Person, Cylinder, Hexagon, RoundedBox, WebBrowser |
| `fontSize` | Rare | Only a few files |

Not used: `stroke`, `strokeWidth`, `border`, `opacity`, `icon`, `width`,
`height`, `metadata`, `description`.

### 19.4 Typical File Template

```
workspace {
    model {
        user = person "Data Scientist" "Description"
        admin = person "Platform Admin" "Description"

        system = softwareSystem "Component Name" "Description" {
            container1 = container "Name" "Desc" "Technology"
            container2 = container "Name" "Desc" "Technology"
        }

        externalA = softwareSystem "Kubernetes" "Description" "External"
        externalB = softwareSystem "Prometheus" "Metrics" "External Platform"

        user -> system "Uses" "HTTPS/443"
        system -> externalA "Talks to" "HTTPS/443"
        container1 -> container2 "Calls" "gRPC/8085"
    }

    views {
        systemContext system "SystemContext" {
            include *
            autoLayout
        }

        container system "Containers" {
            include *
            autoLayout
        }

        styles {
            element "Software System" {
                background #1168bd
                color #ffffff
            }
            element "Person" {
                background #08427b
                color #ffffff
                shape person
            }
            element "Container" {
                background #438dd5
                color #ffffff
            }
            element "External" {
                background #999999
                color #ffffff
            }
            element "External Service" {
                background #f5a623
                color #ffffff
            }
            element "Internal RHOAI" {
                background #7ed321
                color #ffffff
            }
            element "Database" {
                shape cylinder
            }
        }
    }
}
```

### 19.5 Minimum Viable Implementation

To render the ODH corpus, implement in this order:

1. **Parser**: workspace → model → views → styles
2. **Elements**: person, softwareSystem, container, component
3. **Relationships**: simple `->` with description and technology
4. **Views**: systemContext and container with `include *`
5. **Styles**: background, color, shape (5 shapes: Box, Person, Cylinder,
   Hexagon, WebBrowser)
6. **Layout**: autoLayout with default tb direction, rank/node sep 300
7. **Rendering**: Box shape + Person shape + Cylinder shape + arrows

---

## 20. Full DSL Feature Catalog

Features beyond the ODH corpus, documented for completeness.

### 20.1 Directives

| Directive | Purpose |
|-----------|---------|
| `!include <path\|url>` | Include another DSL file |
| `!docs <path>` | Import documentation from directory |
| `!adrs <path>` | Import Architecture Decision Records |
| `!constant <name> <value>` | Define constant for `${name}` substitution |
| `!ref <identifier>` | Reference an existing element |
| `!identifiers <flat\|hierarchical>` | Set identifier scoping |
| `!impliedRelationships` | Auto-create parent-level relationships |
| `!extend` | Extend an existing workspace |
| `!plugin <fqn>` | Load a Java plugin |
| `!script <lang>` | Execute inline script |

### 20.2 Deployment Model

```
deploymentEnvironment "Production" {
    deploymentNode "AWS" "Cloud" "Amazon Web Services" {
        deploymentNode "EC2" "Server" {
            containerInstance webApp
        }
    }
    infrastructureNode "Load Balancer" "Distributes traffic" "AWS ALB"
}
```

### 20.3 Configuration

```
configuration {
    scope <landscape|softwaresystem|none>
    visibility <private|public>
}

branding {
    logo <path|url>
    font <name> [url]
}

terminology {
    person "Actor"
    softwareSystem "Application"
    container "Module"
}
```

### 20.4 Properties and Perspectives

```
element "Name" {
    properties {
        "key" "value"
    }
    perspectives {
        "Security" "Description of security perspective"
    }
}
```

### 20.5 Animation

```
systemContext system "Animated" {
    include *
    animation {
        system
        user
    }
    animation {
        externalSystem
    }
}
```

Animation blocks define steps for progressive diagram reveal.

### 20.6 All Available Shapes

| Shape | Typical Use |
|-------|------------|
| Box | Default for systems, containers, components |
| RoundedBox | Alternative container shape |
| Circle | Special elements |
| Ellipse | Alternative to circle |
| Hexagon | Infrastructure, hardware |
| Cylinder | Databases, storage |
| Component | UML component notation |
| Person | Human actors |
| Robot | Automated actors |
| Folder | File/folder systems |
| WebBrowser | Web applications |
| MobileDevicePortrait | Mobile apps (portrait) |
| MobileDeviceLandscape | Mobile apps (landscape) |
| Pipe | Queues, streams |
| Diamond | Decision points |

---

## Appendix A: Structurizr DSL Reference Sources

- **DSL Parser Source**: `references/structurizr-dsl/src/main/java/com/structurizr/dsl/`
- **Token Definitions**: `StructurizrDslTokens.java`
- **Main Parser**: `StructurizrDslParser.java`
- **Element Parsers**: `PersonParser.java`, `SoftwareSystemParser.java`,
  `ContainerParser.java`, `ComponentParser.java`
- **View Parsers**: `SystemContextViewParser.java`, `ContainerViewParser.java`,
  `ComponentViewParser.java`
- **Style Parsers**: `ElementStyleParser.java`, `RelationshipStyleParser.java`
- **Auto Layout**: `AutoLayoutParser.java`
- **Test DSL Files**: `references/structurizr-dsl/src/test/dsl/`
- **Core Model Library**: `com.structurizr:structurizr-client` (external dep)
- **Export Library**: `structurizr/export` on GitHub (separate repository)

## Appendix B: Color Palette Reference (ODH Corpus)

Common colors used across the 69 DSL files:

| Hex | Usage |
|-----|-------|
| `#08427b` | Person elements (dark blue) |
| `#1168bd` | Primary software system (medium blue) |
| `#438dd5` | Containers (light blue) |
| `#85bbf0` | Components (lighter blue) |
| `#999999` | External/grey elements |
| `#f5a623` | External services (orange) |
| `#7ed321` | Internal RHOAI (green) |
| `#ffffff` | White text |
| `#000000` | Black text (on light backgrounds) |
| `#707070` | Default relationship color |

## Appendix C: Comparison with Mermaid

| Aspect | Structurizr DSL | Mermaid |
|--------|----------------|---------|
| Model | Separate model and views | Diagram text IS the model |
| Multiple diagrams | One model → many views | One text → one diagram |
| Element types | Fixed C4 hierarchy | Arbitrary nodes |
| Relationships | Typed with technology | Simple labeled edges |
| Layout | autoLayout (Sugiyama) | dagre/elk/cose |
| Styling | Tag-based styles block | Inline syntax + CSS classes |
| Shapes | 15 domain-specific | ~70 geometric |
| Output | Needs external renderer | Built-in SVG |
