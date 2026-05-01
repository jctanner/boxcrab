# KaTeX Specification

A language-agnostic specification of how KaTeX works internally,
derived from the reference implementation at `./references/KaTeX/` (v0.16.45).

KaTeX is a fast math typesetting library that converts LaTeX math notation
into rendered HTML+MathML output, without requiring a browser or JavaScript
runtime for the core logic.

---

## Table of Contents

1. [High-Level Architecture](#1-high-level-architecture)
2. [Public API](#2-public-api)
3. [Settings & Configuration](#3-settings--configuration)
4. [Pipeline Overview](#4-pipeline-overview)
5. [Lexer (Tokenizer)](#5-lexer-tokenizer)
6. [Macro Expander (Gullet)](#6-macro-expander-gullet)
7. [Parser (Stomach)](#7-parser-stomach)
8. [Parse Node Types (AST)](#8-parse-node-types-ast)
9. [Function Definition System](#9-function-definition-system)
10. [Environment System](#10-environment-system)
11. [Style System](#11-style-system)
12. [Options (Render Context)](#12-options-render-context)
13. [Build Tree (Orchestration)](#13-build-tree-orchestration)
14. [HTML Builder](#14-html-builder)
15. [MathML Builder](#15-mathml-builder)
16. [DOM Tree Nodes](#16-dom-tree-nodes)
17. [Font Metrics System](#17-font-metrics-system)
18. [Symbol Tables](#18-symbol-tables)
19. [Spacing Rules](#19-spacing-rules)
20. [Unit System](#20-unit-system)
21. [Delimiter Rendering](#21-delimiter-rendering)
22. [Stretchy Elements](#22-stretchy-elements)
23. [SVG Geometry](#23-svg-geometry)
24. [Error Handling](#24-error-handling)
25. [CSS Requirements](#25-css-requirements)
26. [Unicode Support](#26-unicode-support)
27. [Porting Considerations](#27-porting-considerations)

---

## 1. High-Level Architecture

KaTeX is fundamentally a **compiler** that transforms LaTeX math strings into
an intermediate tree, then renders that tree to HTML spans and/or MathML elements.

Unlike mermaid-cli, KaTeX does NOT require a browser or JavaScript runtime for
its core logic. The entire pipeline -- lexing, parsing, macro expansion, layout
computation, and output generation -- is pure computation over strings, numbers,
and tree structures. The only "rendering" is constructing a tree of tagged
elements with CSS classes, heights, depths, and inline styles.

```
LaTeX String
    |
    v
[Lexer] --> Token stream
    |
    v
[MacroExpander] --> Expanded token stream (macros resolved)
    |
    v
[Parser] --> Parse tree (AST of ParseNodes)
    |
    v
[Build Tree] --> DOM tree (Spans, SymbolNodes, MathNodes)
    |
    +---> [HTML Builder] --> HTML spans with CSS classes + inline styles
    +---> [MathML Builder] --> MathML elements (<math>, <mrow>, etc.)
    |
    v
Output: HTML string or DOM nodes
```

The architecture follows TeX's terminology:
- **Mouth** = Lexer (produces tokens)
- **Gullet** = MacroExpander (expands macros)
- **Stomach** = Parser (digests tokens into parse nodes)

### Key Design Principles

1. **Immutable Options**: The `Options` object is never mutated. Style/size changes
   produce new `Options` instances via `.having*()` methods.
2. **Builder dispatch**: Each parse node type has registered `htmlBuilder` and
   `mathmlBuilder` functions. `buildGroup()` dispatches to the correct builder.
3. **Dual output**: By default, both HTML (for visual display) and MathML
   (for accessibility/semantics) are generated and wrapped together.
4. **TeX fidelity**: Font metrics, spacing rules, and style transitions are taken
   directly from TeX's Computer Modern fonts and the TeXbook's algorithms.

---

## 2. Public API

### `render(expression, baseNode, options)`

Parses `expression`, builds the DOM tree, and appends it as a child of `baseNode`.
Clears `baseNode.textContent` first.

### `renderToString(expression, options) -> string`

Parses and builds, returning the full HTML+MathML markup as a string.

### `renderToDomTree(expression, options) -> DomSpan`

Returns the internal DOM tree (a `Span` node) without converting to string or
appending to the real DOM. Useful for custom output backends.

### `renderToHTMLTree(expression, options) -> DomSpan`

Like `renderToDomTree`, but produces HTML only (no MathML).

### `__parse(expression, options) -> ParseNode[]`

Returns the raw parse tree. Unstable API.

### Error Behavior

If `throwOnError` is true (default), parse errors throw `ParseError`.
If false, the expression is rendered as an error span:
- CSS class: `katex-error`
- `title` attribute: the error message
- `style.color`: the `errorColor` setting (default: `#cc0000`)
- Content: the original expression text

### Quirks Mode Detection

If `document.compatMode !== "CSS1Compat"`, KaTeX disables rendering entirely
and throws, because its CSS layout requires standards mode.

---

## 3. Settings & Configuration

| Setting | Type | Default | Purpose |
|---------|------|---------|---------|
| `displayMode` | boolean | `false` | Display (block) vs inline math |
| `output` | enum | `"htmlAndMathml"` | `"html"`, `"mathml"`, or `"htmlAndMathml"` |
| `leqno` | boolean | `false` | Left-aligned equation numbers |
| `fleqn` | boolean | `false` | Flush-left display equations |
| `throwOnError` | boolean | `true` | Throw `ParseError` vs render error inline |
| `errorColor` | string | `"#cc0000"` | Color for error rendering |
| `macros` | object | `{}` | Custom macro definitions (`\name` -> expansion) |
| `minRuleThickness` | number | `0.04` | Minimum thickness for fraction bars, sqrt vinculums |
| `colorIsTextColor` | boolean | `false` | If true, `\color` acts like `\textcolor` |
| `strict` | mixed | `false` | Strictness level: `true`, `"warn"`, `"error"`, `"ignore"`, or function |
| `trust` | mixed | `false` | Allow `\href`, `\url`, `\htmlClass`, etc. Boolean or function |
| `maxSize` | number | `Infinity` | Maximum size (em) for user-specified sizes |
| `maxExpand` | number | `1000` | Maximum number of macro expansions (prevents infinite loops) |
| `globalGroup` | boolean | `false` | If true, definitions persist (no group wrapping) |

### Trust System

Certain commands (`\href`, `\url`, `\includegraphics`, `\htmlClass`, `\htmlId`,
`\htmlStyle`, `\htmlData`) are security-sensitive. They are only processed when
`trust` is `true` or when a trust function returns `true` for the specific context.

Trust context provides: `{ command, url?, protocol?, class?, id?, style?, attributes? }`

---

## 4. Pipeline Overview

The full rendering pipeline, step by step:

```
1. Construct Settings from user options
2. Call parseTree(expression, settings):
   a. Create Parser (which creates MacroExpander, which creates Lexer)
   b. If colorIsTextColor: alias \color to \textcolor
   c. Begin implicit group (unless globalGroup)
   d. parser.parseExpression(false) -- recursive descent
   e. Expect EOF
   f. End implicit group
   g. Return ParseNode[]
3. Call buildTree(tree, expression, settings):
   a. Create Options from Settings (display/text style, maxSize, minRuleThickness)
   b. Based on output mode:
      - "mathml": buildMathML(tree) only
      - "html": buildHTML(tree) only, wrap in span.katex
      - "htmlAndMathml": both, wrap in span.katex
   c. If displayMode: wrap in span.katex-display (+ leqno/fleqn classes)
   d. Return DomSpan
4. Convert to output:
   - .toNode() for DOM insertion
   - .toMarkup() for HTML string
```

---

## 5. Lexer (Tokenizer)

The lexer converts a raw LaTeX string into a stream of `Token` objects.

### Token Structure

```
Token {
  text: string       -- the token text (e.g. "x", "\\frac", " ", "EOF")
  loc: SourceLocation -- { lexer, start, end } for error reporting
  noexpand?: boolean  -- if true, skip macro expansion
  treatAsRelax?: boolean -- treat as \relax
}
```

### Tokenization Regex

A single complex regex matches all token types in one pass:

| Capture Group | Matches |
|---------------|---------|
| Group 1 | Regular whitespace: `[ \r\n\t]+` |
| Group 2 | Control space: `\\` followed by whitespace |
| Group 3 | Everything else (single codepoints, surrogate pairs, \verb, control words/symbols) |
| Group 4 | Left delimiter of `\verb*` |
| Group 5 | Left delimiter of `\verb` |
| Group 6 | Control word: `\\[a-zA-Z@]+` (trailing whitespace consumed but not included) |

Characters NOT matched: control characters (0x00-0x1f except whitespace),
bare backslash at end, BMP private use area (U+E000-U+F8FF), bare surrogates.

### Special Character Handling

- **Combining diacritical marks** (U+0300-U+036F): Attached to the preceding character
  as a single token. Later transformed into `\accent` parse nodes by the parser.
- **Comments** (`%`): Category code 14. Everything from `%` to the next newline is
  skipped. If no newline follows, a strict-mode warning is issued.
- **Active characters** (`~`): Category code 13. Treated as a macro.
- **Category codes**: Only two are supported: 14 (comment, default for `%`) and
  13 (active, default for `~`). Others can be set via `lexer.setCatcode()`.

### Backtracking

The lexer supports backtracking by manipulating `tokenRegex.lastIndex`. The
MacroExpander also uses a token stack for pushback.

### EOF

When `lastIndex === input.length`, returns `Token("EOF", ...)`.

---

## 6. Macro Expander (Gullet)

The MacroExpander sits between the lexer and the parser. It expands macros until
only non-macro tokens remain.

### Architecture

```
MacroExpander {
  lexer: Lexer              -- the token source
  stack: Token[]            -- pushback stack (LIFO, tokens in REVERSE order)
  macros: Namespace         -- scoped macro definitions
  expansionCount: number    -- tracks expansion count for maxExpand limit
  mode: "math" | "text"    -- current mode
}
```

### Token Flow

```
Lexer --> [stack] --> MacroExpander.expandNextToken() --> Parser
```

When the stack is empty, a new token is pulled from the lexer. Macro expansion
pushes replacement tokens onto the stack.

### Key Methods

- `future()` -- peek at next token without consuming (like TeX's `\futurelet`)
- `popToken()` -- remove and return next unexpanded token
- `pushToken(token)` / `pushTokens(tokens)` -- push tokens onto the stack
- `expandOnce()` -- expand a single macro one level
- `expandNextToken()` -- fully expand until a non-macro token is found
- `consumeArgs(numArgs)` -- parse N brace-delimited arguments for a macro
- `scanArgument(optional)` -- scan for an optional `[...]` or required `{...}` argument

### Namespace (Group Scoping)

Macros are stored in a `Namespace` with TeX-style group nesting:

```
Namespace {
  current: Map<string, MacroDefinition>
  builtins: Map<string, MacroDefinition>    -- built-in macros
  undefStack: Array<Map>                     -- undo stack for group exits
}
```

- `beginGroup()` -- push a new scope frame
- `endGroup()` -- pop scope, restoring overwritten values
- `set(name, value, global)` -- define a macro (local or global)
- `get(name)` -- look up a macro (current scope, then builtins)
- `has(name)` -- check existence

Global `set` iterates through all scope frames to set the value everywhere.

### Expansion Limit

Each expansion increments `expansionCount`. When it exceeds `settings.maxExpand`
(default: 1000), a `ParseError` is thrown to prevent infinite macro recursion.

### Built-in Macros

Defined in `macros.ts` (~41KB). These include:
- Standard LaTeX commands: `\not`, `\dots`, `\mathbb`, `\binom`, etc.
- Environment shorthands: `\matrix`, `\pmatrix`, etc.
- Spacing: `\,`, `\:`, `\;`, `\!`, `\quad`, `\qquad`
- Accents: `\hat`, `\bar`, `\vec`, `\dot`, `\ddot`, etc.
- Text mode: `\text`, `\textbf`, `\textit`, etc.
- Many more (thousands of definitions)

Macros can be:
- **Simple string replacements**: `"\\name" -> "replacement tokens"`
- **Parameterized**: `"\\name#1#2" -> "expansion with #1 and #2"`
- **Functional**: A function that receives the MacroExpander context and
  produces tokens programmatically

---

## 7. Parser (Stomach)

The parser is a recursive descent parser that produces an AST of `ParseNode` objects.

### Parser State

```
Parser {
  mode: "math" | "text"      -- current parsing mode
  gullet: MacroExpander       -- token source (with macro expansion)
  settings: Settings          -- configuration
  leftrightDepth: number      -- nesting depth for \left/\right validation
  nextToken: Token | null     -- one-token lookahead cache
}
```

### Core Parsing Methods

#### `parse() -> ParseNode[]`

Main entry point. Wraps expression in an implicit group, calls
`parseExpression(false)`, expects EOF.

#### `parseExpression(breakOnInfix, breakOnTokenText?) -> ParseNode[]`

Parses a sequence of atoms. Loops calling `parseAtom()` until:
- EOF
- `}`, `\endgroup`, `\end`, `\right`, or `&`
- The specified break token
- An infix operator (if `breakOnInfix` is true)

After collecting atoms, handles:
1. **Ligatures** (text mode only): `--` -> en-dash, `---` -> em-dash,
   ``` `` ``` -> open quote, `''` -> close quote
2. **Infix operators**: If any `infix` node found (e.g. `\over`), splits the
   expression into numerator/denominator and rewrites as a fraction

#### `parseAtom(breakOnTokenText?) -> ParseNode | null`

Parses a single atom, which is a base group optionally followed by
superscripts/subscripts:

```
1. Parse base group via parseGroup()
2. Loop checking for:
   - \limits / \nolimits (on operators)
   - ^ (superscript) -- parse argument via handleSupSubscript()
   - _ (subscript) -- parse argument via handleSupSubscript()
   - ' (prime) -- collect multiple primes, combine with any ^ superscript
   - Unicode superscript/subscript characters -- converted to regular sub/sup
3. If super or subscript found, wrap in "supsub" node
```

In text mode, superscripts and subscripts are not parsed.

#### `parseGroup(name, breakOnTokenText?) -> ParseNode | null`

Parses a group, which is one of:
- **Braced group** `{...}`: Opens namespace scope, parses expression, closes with `}`
- **\begingroup...\endgroup**: Same but produces a "semisimple" group
- **Function call**: If the token matches a registered function, parse it with args
- **Symbol**: A single character/symbol lookup

#### `parseFunction(breakOnTokenText?, name?) -> ParseNode | null`

If the current token matches a registered function:
1. Consume the command token
2. Validate context (text vs math mode, argument position)
3. Parse arguments via `parseArguments()`
4. Call the function's handler

#### `parseArguments(func, funcData)`

For a function with `numArgs` required and `numOptionalArgs` optional arguments:
1. For each argument position, determine its type (`argType`)
2. Parse via `parseGroupOfType()` which dispatches based on type:
   - `"color"` -- parse color string (hex or name)
   - `"size"` -- parse measurement (number + unit)
   - `"url"` -- parse URL with special catcode handling
   - `"math"` / `"text"` -- switch mode, parse group
   - `"hbox"` -- text mode group wrapped in text styling
   - `"raw"` -- raw string (no parsing)
   - `"primitive"` -- required group, no special handling
   - `null` / `"original"` -- standard argument group

#### `parseSymbol() -> ParseNode | null`

Parses a single symbol:
1. Handle `\verb` / `\verb*` (literal text)
2. Apply Unicode symbol replacements
3. Strip combining diacritical marks (converting `i` -> dotless `ı`, `j` -> dotless `ȷ`)
4. Look up in symbol table to determine group (mathord, textord, atom, etc.)
5. Handle non-ASCII characters (charCode >= 0x80) as text ordinals
6. Wrap combining marks as nested `accent` nodes

---

## 8. Parse Node Types (AST)

Every parse node has at minimum:
```
{
  type: NodeType,          -- discriminant
  mode: "math" | "text",  -- mode when parsed
  loc?: SourceLocation     -- source position (optional)
}
```

### Complete Node Type Catalog

#### Structural Nodes

| Type | Purpose | Key Fields |
|------|---------|------------|
| `ordgroup` | Braced group `{...}` | `body: ParseNode[]`, `semisimple?: boolean` |
| `supsub` | Superscript/subscript | `base`, `sup?`, `sub?` |
| `tag` | Equation tag | `body: ParseNode[]`, `tag: ParseNode[]` |
| `styling` | Style change (`\displaystyle`, etc.) | `style: StyleStr`, `body: ParseNode[]` |
| `sizing` | Size change (`\large`, etc.) | `size: 1..11`, `body: ParseNode[]` |
| `font` | Font change (`\mathbf`, etc.) | `font: string`, `body: ParseNode` |
| `color` | Colored content | `color: string`, `body: ParseNode[]` |
| `text` | Text mode content | `body: ParseNode[]`, `font?: string` |

#### Symbol Nodes

| Type | Purpose |
|------|---------|
| `atom` | Character with atom family (bin, rel, open, close, punct, inner) |
| `mathord` | Math ordinary symbol |
| `textord` | Text ordinary symbol |
| `spacing` | Whitespace token |
| `op-token` | Operator token (raw) |
| `accent-token` | Accent token (raw) |

#### Math Structure Nodes

| Type | Purpose | Key Fields |
|------|---------|------------|
| `genfrac` | Generalized fraction | `numer`, `denom`, `hasBarLine`, `leftDelim?`, `rightDelim?`, `barSize?` |
| `sqrt` | Square root | `body`, `index?` (nth root) |
| `leftright` | `\left...\right` delimiters | `left`, `right`, `body[]`, `rightColor?` |
| `middle` | `\middle` delimiter | `delim` |
| `accent` | Over-accent (`\hat`, etc.) | `label`, `base`, `isStretchy?`, `isShifty?` |
| `accentUnder` | Under-accent | `label`, `base`, `isStretchy?`, `isShifty?` |
| `horizBrace` | Over/underbrace | `label`, `isOver`, `base` |
| `overline` | `\overline` | `body` |
| `underline` | `\underline` | `body` |
| `xArrow` | Extensible arrow | `label`, `body` (above), `below?` |

#### Layout Nodes

| Type | Purpose | Key Fields |
|------|---------|------------|
| `array` | Matrix/table environments | `body[][]`, `cols?`, `rowGaps[]`, `hLinesBeforeRow`, `tags?` |
| `cr` | Line break `\\` | `newLine: boolean`, `size?` |
| `kern` | Horizontal space | `dimension: Measurement` |
| `rule` | Horizontal rule | `width`, `height`, `shift?` |
| `raisebox` | Vertical shift | `dy: Measurement`, `body` |
| `lap` | Overlap (`\llap`, `\rlap`) | `alignment`, `body` |
| `phantom` | Invisible spacer | `body[]` |
| `vphantom` | Vertical phantom | `body` |
| `smash` | Height/depth suppression | `body`, `smashHeight`, `smashDepth` |
| `vcenter` | Vertical center on axis | `body` |

#### Operator Nodes

| Type | Purpose | Key Fields |
|------|---------|------------|
| `op` | Large operator (`\sum`, `\int`) | `limits`, `symbol`, `name` or `body[]` |
| `operatorname` | Named operator (`\sin`, `\log`) | `body[]`, `limits`, `alwaysHandleSupSub` |

#### Special Nodes

| Type | Purpose | Key Fields |
|------|---------|------------|
| `delimsizing` | Sized delimiter (`\big`, `\Big`) | `size: 1..4`, `mclass`, `delim` |
| `mclass` | Math class override (`\mathbin`, etc.) | `mclass`, `body[]`, `isCharacterBox` |
| `enclose` | Box/enclose (`\boxed`, `\cancel`) | `label`, `backgroundColor?`, `borderColor?`, `body` |
| `mathchoice` | Style-dependent content | `display[]`, `text[]`, `script[]`, `scriptscript[]` |
| `href` | Hyperlink | `href`, `body[]` |
| `html` | Raw HTML attributes | `attributes`, `body[]` |
| `htmlmathml` | Separate HTML/MathML | `html[]`, `mathml[]` |
| `includegraphics` | Image | `alt`, `width`, `height`, `totalheight`, `src` |
| `url` | URL | `url` |
| `verb` | Verbatim text | `body: string`, `star: boolean` |
| `pmb` | Poor man's bold | `mclass`, `body[]` |
| `infix` | Infix operator (`\over`) | `replaceWith`, `size?`, `token?` |
| `internal` | Internal (filtered out) | (no fields) |
| `raw` | Raw string | `string` |
| `environment` | Environment start | `name`, `nameGroup` |

---

## 9. Function Definition System

Functions are the primary extension mechanism. Each function definition (`FunctionSpec`)
provides:

```
FunctionSpec {
  type: NodeType              -- the parse node type this function produces
  names: string[]             -- LaTeX command names (e.g. ["\\frac", "\\dfrac"])
  numArgs: number             -- number of required arguments
  numOptionalArgs: number     -- number of optional arguments (default: 0)
  argTypes?: ArgType[]        -- types for each argument
  allowedInText?: boolean     -- can be used in \text{} (default: false)
  allowedInMath?: boolean     -- can be used in math mode (default: true)
  allowedInArgument?: boolean -- can appear as a function argument (default: true)
  primitive?: boolean         -- uses primitive argument parsing
  infix?: boolean             -- is an infix operator (e.g. \over)

  handler(context, args, optArgs) -> ParseNode  -- parse-time handler
  htmlBuilder(node, options) -> HtmlDomNode      -- HTML output builder
  mathmlBuilder(node, options) -> MathNode        -- MathML output builder
}
```

### Argument Types

| ArgType | Parsing Behavior |
|---------|-----------------|
| `"color"` | Parse hex color or color name |
| `"size"` | Parse number + unit (e.g. `3pt`, `1.5em`) |
| `"url"` | Parse URL (special catcode handling for `%` and `~`) |
| `"math"` | Switch to math mode, parse group |
| `"text"` | Switch to text mode, parse group |
| `"hbox"` | Parse text group, wrap in text styling |
| `"raw"` | Collect raw string tokens (no parsing) |
| `"primitive"` | Required group, used for `\sqrt` etc. |
| `"original"` / `null` | Standard argument parsing |

### Function Files

Each major function category has its own file in `src/functions/`:

| File | Commands |
|------|----------|
| `accent.ts` | `\hat`, `\bar`, `\vec`, `\dot`, `\tilde`, `\widehat`, `\widetilde`, `\overline`, `\overleftarrow`, `\overrightarrow`, etc. |
| `accentunder.ts` | `\underline`, `\underleftarrow`, `\underrightarrow`, `\underbrace`, etc. |
| `arrow.ts` | `\xrightarrow`, `\xleftarrow`, `\xhookrightarrow`, etc. |
| `color.ts` | `\color`, `\textcolor`, `\colorbox`, `\fcolorbox` |
| `delimsizing.ts` | `\big`, `\Big`, `\bigg`, `\Bigg` (and left/right/middle variants) |
| `enclose.ts` | `\boxed`, `\cancel`, `\bcancel`, `\xcancel`, `\sout`, `\angl`, `\phase` |
| `font.ts` | `\mathbf`, `\mathit`, `\mathbb`, `\mathcal`, `\mathfrak`, `\mathsf`, `\mathtt`, `\boldsymbol`, etc. |
| `genfrac.ts` | `\frac`, `\dfrac`, `\tfrac`, `\binom`, `\dbinom`, `\tbinom`, `\cfrac`, `\over`, `\atop`, `\above` |
| `horizBrace.ts` | `\overbrace`, `\underbrace`, `\overlinesegment`, `\underlinesegment` |
| `kern.ts` | `\kern`, `\mkern`, `\hskip`, `\mskip` |
| `op.ts` | `\sum`, `\prod`, `\int`, `\lim`, `\sin`, `\cos`, `\log`, etc. |
| `operatorname.ts` | `\operatorname`, `\operatorname*` |
| `sizing.ts` | `\tiny`, `\scriptsize`, `\small`, `\normalsize`, `\large`, `\Large`, `\LARGE`, `\huge`, `\Huge` |
| `sqrt.ts` | `\sqrt` |
| `styling.ts` | `\displaystyle`, `\textstyle`, `\scriptstyle`, `\scriptscriptstyle` |
| `supsub.ts` | `^`, `_` (superscript, subscript) |
| `text.ts` | `\text`, `\textrm`, `\textsf`, `\texttt`, `\textbf`, `\textit`, etc. |

---

## 10. Environment System

Environments are blocks delimited by `\begin{name}...\end{name}`.

### Array Environment (`environments/array.ts`)

Supports these environments:

| Environment | Delimiters | Description |
|-------------|-----------|-------------|
| `matrix` | none | Undelimited matrix |
| `pmatrix` | `(...)` | Parenthesized matrix |
| `bmatrix` | `[...]` | Bracketed matrix |
| `Bmatrix` | `{...}` | Braced matrix |
| `vmatrix` | `\|...\|` | Single-bar matrix |
| `Vmatrix` | `\|\|...\|\|` | Double-bar matrix |
| `smallmatrix` | none | Inline-sized matrix |
| `array` | none | General array with column specs |
| `align` / `align*` | none | Aligned equations |
| `gather` / `gather*` | none | Centered equations |
| `equation` / `equation*` | none | Single equation |
| `multline` / `multline*` | none | Multi-line equation |
| `split` | none | Split inside equation |

Column specifications: `c` (center), `l` (left), `r` (right), `|` (vertical rule).

Horizontal rules: `\hline`, `\hdashline`, `\cline{i-j}`.

Row separation: `\\` with optional `[size]` gap.

### Commutative Diagram (`environments/cd.ts`)

Support for `CD` environment with arrow labels via `@` syntax.

---

## 11. Style System

KaTeX implements TeX's 8 math styles:

| ID | Name | Size | Cramped | Usage |
|----|------|------|---------|-------|
| 0 | D (Display) | 0 | false | `$$...$$`, `\displaystyle` |
| 1 | Dc (Display cramped) | 0 | true | Denominator of display fraction |
| 2 | T (Text) | 1 | false | `$...$`, `\textstyle` |
| 3 | Tc (Text cramped) | 1 | true | Denominator of text fraction |
| 4 | S (Script) | 2 | false | Superscript |
| 5 | Sc (Script cramped) | 2 | true | Subscript |
| 6 | SS (ScriptScript) | 3 | false | Super-superscript |
| 7 | SSc (ScriptScript cramped) | 3 | true | Sub-subscript |

### Style Transitions

These are fixed lookup tables mapping current style ID to new style ID:

```
sup:     [S,  Sc, S,  Sc, SS, SSc, SS, SSc]   -- superscript
sub:     [Sc, Sc, Sc, Sc, SSc, SSc, SSc, SSc]  -- subscript
fracNum: [T,  Tc, S,  Sc, SS, SSc, SS, SSc]    -- fraction numerator
fracDen: [Tc, Tc, Sc, Sc, SSc, SSc, SSc, SSc]  -- fraction denominator
cramp:   [Dc, Dc, Tc, Tc, Sc, Sc, SSc, SSc]    -- cramped version
text:    [D,  Dc, T,  Tc, T,  Tc, T,   Tc]     -- text version
```

### Cramped vs Uncramped

Cramped styles reduce the height available for superscripts (they are placed
lower). This happens in denominators and subscripts.

### Tight Spacing

Styles with `size >= 2` (Script, ScriptScript) use `tightSpacings` instead of
regular `spacings`. In tight mode, most inter-atom spacing is suppressed.

---

## 12. Options (Render Context)

The `Options` object carries all rendering state through the tree-building process.
It is **immutable** -- methods return new instances.

### Properties

```
Options {
  style: StyleInterface      -- current math style (D, T, S, SS + cramped)
  color: string | undefined  -- inherited text color
  size: 1..11                -- font size index (6 = normalsize = 10pt)
  textSize: 1..11            -- base text size (for relative calculations)
  phantom: boolean           -- if true, content is invisible
  font: string               -- math font ("mathbf", "mathit", etc.)
  fontFamily: string         -- text font family ("textsf", "texttt", etc.)
  fontWeight: FontWeight     -- "textbf" | "textmd" | ""
  fontShape: FontShape       -- "textit" | "textup" | ""
  sizeMultiplier: number     -- derived from size index (0.5 to 2.488)
  maxSize: number            -- cap on user-specified sizes
  minRuleThickness: number   -- minimum line thickness
}
```

### Size Multipliers

| Size Index | Point Size | Multiplier | LaTeX Command |
|-----------|-----------|------------|---------------|
| 1 | 5pt | 0.5 | `\tiny` |
| 2 | 6pt | 0.6 | |
| 3 | 7pt | 0.7 | `\scriptsize` |
| 4 | 8pt | 0.8 | `\footnotesize` |
| 5 | 9pt | 0.9 | `\small` |
| 6 | 10pt | 1.0 | `\normalsize` (base) |
| 7 | 12pt | 1.2 | `\large` |
| 8 | 14.4pt | 1.44 | `\Large` |
| 9 | 17.28pt | 1.728 | `\LARGE` |
| 10 | 20.74pt | 2.074 | `\huge` |
| 11 | 24.88pt | 2.488 | `\Huge` |

### Size-Style Mapping

When style changes (e.g. entering a superscript), the effective font size changes
according to a mapping table. For example, at normalsize (6):

```
text style:         size 6 (10pt)
script style:       size 3 (7pt)
scriptscript style: size 1 (5pt)
```

This table (`sizeStyleMap`) has 11 entries, one per base size.

### Transition Methods (all return new Options)

- `havingStyle(style)` -- change math style
- `havingCrampedStyle()` -- cramp current style
- `havingSize(size)` -- change size index
- `havingBaseStyle(style)` -- change style and adjust sizing classes
- `havingBaseSizing()` -- remove size overrides
- `withColor(color)` -- set color
- `withPhantom()` -- enable phantom mode
- `withFont(font)` -- set math font
- `withTextFontFamily(family)` / `withTextFontWeight(weight)` / `withTextFontShape(shape)`
- `sizingClasses(oldOptions)` -- CSS classes for transitioning between sizes
- `fontMetrics()` -- get font metrics for current size (cached)

---

## 13. Build Tree (Orchestration)

`buildTree()` is the top-level orchestrator:

```
buildTree(tree, expression, settings) -> DomSpan:
  1. options = optionsFromSettings(settings)
     - style: DISPLAY if displayMode, else TEXT
     - maxSize, minRuleThickness from settings
  2. If output === "mathml":
     - return buildMathML(tree, expression, options, displayMode, true)
  3. If output === "html":
     - htmlNode = buildHTML(tree, options)
     - katexNode = span.katex([htmlNode])
  4. Else (htmlAndMathml):
     - mathMLNode = buildMathML(tree, expression, options, displayMode, false)
     - htmlNode = buildHTML(tree, options)
     - katexNode = span.katex([mathMLNode, htmlNode])
  5. return displayWrap(katexNode, settings)
     - If displayMode: wrap in span.katex-display (+ leqno, fleqn classes)
```

---

## 14. HTML Builder

The HTML builder converts parse nodes into a tree of `Span` and `SymbolNode`
elements with CSS classes, heights, depths, and inline styles.

### `buildHTML(tree, options) -> DomSpan`

1. Strip off outer `tag` wrapper if present
2. Call `buildExpression(tree, options, "root")`
3. Split expression at breakable points (after `mbin` or `mrel` classes)
4. Wrap each chunk in a `.base` span with a `.strut` span for vertical extent
5. Handle `\newline` by starting a new chunk
6. Handle equation tags/numbers
7. Wrap everything in `span.katex-html[aria-hidden="true"]`

### `buildExpression(nodes, options, isRealGroup, surrounding) -> HtmlDomNode[]`

1. Build each parse node via `buildGroup()`
2. Flatten `DocumentFragment` results
3. Combine consecutive `SymbolNode`s via `tryCombineChars()`
4. If real group (not partial):
   a. **Bin cancellation** (TeXbook Rules 5-6): Binary operators become ordinary
      when adjacent to: `leftmost`, `mbin`, `mopen`, `mrel`, `mop`, `mpunct`
      (left side) or `rightmost`, `mrel`, `mclose`, `mpunct` (right side)
   b. **Spacing insertion**: Based on the math class of adjacent nodes, insert
      glue (thin/medium/thick space) per the spacing table

### `buildGroup(group, options) -> HtmlDomNode`

Dispatches to the registered `htmlBuilder` for `group.type`. If the size changed
between parent and child options, wraps in a sizing span.

### Struts

Each `.base` span gets a `.strut` child prepended with:
- `height = body.height + body.depth`
- `vertical-align = -body.depth`

This ensures the browser renders the element at the correct vertical extent,
regardless of font metrics.

### Line Breaking

The expression is broken into unbreakable `.base` chunks at:
- After `mbin` (binary operator)
- After `mrel` (relation)
- After `.allowbreak`
- At `\newline`

`\nobreak` prevents breaking after the current operator.

---

## 15. MathML Builder

The MathML builder converts parse nodes into MathML elements wrapped in
`<semantics>` with a `<annotation>` containing the original LaTeX source.

### `buildMathML(tree, expression, options, displayMode, forMathmlOnly) -> DomSpan`

1. Build expression nodes via `buildExpression()`
2. Wrap in `<mrow>` if needed (unless single `<mrow>` or `<mtable>`)
3. Create `<annotation encoding="application/x-tex">` with source text
4. Wrap in `<semantics>`
5. Wrap in `<math xmlns="http://www.w3.org/1998/Math/MathML">`
   - Add `display="block"` if displayMode
6. Wrap in `span.katex-mathml` (or `span.katex` if MathML-only output)

### MathML Node Types

```
"math", "annotation", "semantics", "mtext", "mn", "mo", "mi",
"mspace", "mover", "munder", "munderover", "msup", "msub", "msubsup",
"mfrac", "mroot", "msqrt", "mtable", "mtr", "mtd", "mlabeledtr",
"mrow", "menclose", "mstyle", "mpadded", "mphantom", "mglyph"
```

### Font Variants

MathML uses `mathvariant` attribute for font changes:

| KaTeX Font | MathML Variant |
|-----------|---------------|
| `mathbf` | `bold` |
| `mathit` | `italic` |
| `mathbb` | `double-struck` |
| `mathcal` / `mathscr` | `script` |
| `mathfrak` | `fraktur` |
| `mathsf` | `sans-serif` |
| `mathtt` | `monospace` |
| `boldsymbol` | `bold` (textord) or `bold-italic` (other) |
| `mathsfit` | `sans-serif-italic` |

### Node Concatenation

The MathML builder concatenates adjacent nodes of the same type:
- Adjacent `<mtext>` with same `mathvariant` -> merged
- Adjacent `<mn>` -> merged (e.g. "1" + "2" = "12")
- `<mn>` followed by `<mi>.</mi>` -> merged (decimal point)
- `\not` operator (`̸`) overlaid on following character via combining mark

---

## 16. DOM Tree Nodes

KaTeX builds a virtual DOM tree of these node types, each with `toNode()` (real
DOM) and `toMarkup()` (HTML string) methods.

### HTML Nodes

| Class | Purpose | Key Properties |
|-------|---------|----------------|
| `Span` | HTML `<span>` container | `classes[]`, `children[]`, `height`, `depth`, `maxFontSize`, `style: CssStyle`, `attributes` |
| `SymbolNode` | Single glyph `<span>` | `text`, `height`, `depth`, `italic`, `skew`, `width`, `maxFontSize`, `classes`, `style` |
| `Anchor` | HTML `<a>` hyperlink | `href`, `classes[]`, `children[]`, `height`, `depth` |
| `DocumentFragment` | Container (no element) | `children[]` (flattened into parent during build) |

### SVG Nodes

| Class | Purpose | Key Properties |
|-------|---------|----------------|
| `SvgNode` | `<svg>` container | `children[]` (PathNode/LineNode), `attributes` |
| `PathNode` | `<path>` element | `pathName` (key into svgGeometry paths), `alternate?` |
| `LineNode` | `<line>` element | attributes via constructor |

### Height and Depth

Every HTML node carries `height` (above baseline, in em) and `depth` (below
baseline, in em). These are computed from font metrics and propagated upward
through the tree. They determine strut sizing and vertical alignment.

### CSS Style

Inline styles are set via the `style` property, which maps to CSS properties:
- `height`, `top`, `verticalAlign` -- positioning
- `marginLeft`, `marginRight`, `paddingLeft` -- spacing
- `color`, `backgroundColor`, `borderColor` -- colors
- `borderTopWidth`, `borderBottomWidth`, `borderRightWidth` -- borders
- `minWidth`, `width` -- sizing
- `fontSize` -- for size changes

---

## 17. Font Metrics System

KaTeX uses pre-extracted metrics from TeX's Computer Modern fonts.

### Global Font Parameters (`fontMetrics.ts`)

These are TeX's "sigma" and "xi" parameters, organized into 3 arrays
(one per size range: text, script, scriptscript):

| Parameter | Values | Purpose |
|-----------|--------|---------|
| `slant` | 0.250 | Italic slant angle |
| `xHeight` | 0.431 | Height of lowercase "x" |
| `quad` | 1.0 / 1.171 / 1.472 | Width of an em (varies by size) |
| `num1` - `num3` | 0.677 - 0.925 | Numerator vertical position |
| `denom1` - `denom2` | 0.345 - 1.025 | Denominator vertical position |
| `sup1` - `sup3` | 0.289 - 0.503 | Superscript vertical position |
| `sub1` - `sub2` | 0.150 - 0.400 | Subscript vertical position |
| `supDrop` | 0.353 - 0.494 | Superscript drop when both sup+sub |
| `subDrop` | 0.050 - 0.100 | Subscript drop when both sup+sub |
| `delim1` - `delim2` | 1.010 - 2.390 | Delimiter sizes |
| `axisHeight` | 0.250 | Height of fraction bar axis |
| `defaultRuleThickness` | 0.04 - 0.049 | Default line thickness |
| `bigOpSpacing1` - `5` | 0.111 - 0.611 | Large operator spacing |
| `sqrtRuleThickness` | 0.04 | Sqrt vinculum thickness |
| `ptPerEm` | 10.0 | Points per em |
| `doubleRuleSep` | 0.2 | Array column separator |
| `arrayRuleWidth` | 0.04 | Array border width |
| `fboxsep` | 0.3 | Box padding |
| `fboxrule` | 0.04 | Box border width |

### Character Metrics (`fontMetricsData.js`)

Pre-generated data mapping font name + character code to:
```
[depth, height, italic_correction, skew, width]
```

All values in em units.

**Fonts covered** (10 total):
- `Main-Regular`, `Main-Bold`, `Main-BoldItalic`
- `Math-Italic`, `Math-BoldItalic`
- `Size1-Regular`, `Size2-Regular`, `Size3-Regular`, `Size4-Regular`
- `AMS-Regular`
- `SansSerif-Regular`, `SansSerif-Bold`, `SansSerif-Italic`
- `Typewriter-Regular`
- `Caligraphic-Regular`, `Caligraphic-Bold`
- `Fraktur-Regular`, `Fraktur-Bold`
- `Script-Regular`

### Character Metrics Lookup

`getCharacterMetrics(character, fontName, mode)`:
1. Get the character's code point
2. Look up in `metricMap[fontName][codePoint]`
3. If not found, check `extraCharacterMap` for fallback metrics
4. Return `CharacterMetrics { depth, height, italic, skew, width }` or null

### Extra Character Map

Maps accented Latin/Cyrillic characters to a base character whose metrics
serve as approximations (e.g. `Å` -> `A`, `Ð` -> `D`).

---

## 18. Symbol Tables

The symbol table (`symbols.ts`) maps LaTeX commands to their rendering properties.

### Structure

```
symbols[mode][name] = {
  font: "main" | "ams",     -- which font contains the glyph
  group: Group,              -- atom classification
  replace?: string           -- Unicode replacement character
}
```

### Groups (Atom Classes)

| Group | CSS Class | Description |
|-------|-----------|-------------|
| `bin` | `mbin` | Binary operator (+, -, ×) |
| `rel` | `mrel` | Relation (=, <, >, ≡) |
| `open` | `mopen` | Opening delimiter ((, [, {) |
| `close` | `mclose` | Closing delimiter (), ], }) |
| `punct` | `mpunct` | Punctuation (,, ;) |
| `inner` | `minner` | Inner (fraction bar, etc.) |
| `op` | `mop` | Large operator |
| `mathord` | `mord` | Math ordinary (letters, digits) |
| `textord` | `mord` | Text ordinary |
| `spacing` | `mspace` | Whitespace |
| `accent-token` | -- | Accent mark (used during parsing) |

### Replacement

When a symbol has a `replace` property, the LaTeX command is rendered using the
Unicode character. For example: `\phi` -> `φ` (U+03D5), `\equiv` -> `≡` (U+2261).

---

## 19. Spacing Rules

Inter-atom spacing follows TeX's rules (TeXbook, Chapter 18).

### Normal Spacing (Display/Text Style)

Matrix of spacing between atom classes. Empty cells = no space.

```
             mord  mop   mbin  mrel  mopen mclose mpunct minner
   mord      --    thin  med   thick --    --     --     thin
   mop       thin  thin  --    thick --    --     --     thin
   mbin      med   med   --    --    med   --     --     med
   mrel      thick thick --    --    thick --     --     thick
   mopen     --    --    --    --    --    --     --     --
   mclose    --    thin  med   thick --    --     --     thin
   mpunct    thin  thin  --    thick thin  thin   thin   thin
   minner    thin  thin  med   thick thin  --     thin   thin
```

### Space Values

| Name | Size | TeX Equivalent |
|------|------|---------------|
| thinspace | 3mu (3/18 em) | `\,` |
| mediumspace | 4mu (4/18 em) | `\:` |
| thickspace | 5mu (5/18 em) | `\;` |

### Tight Spacing (Script/ScriptScript Style)

In tight mode, almost all spacing is suppressed. Only `mord-mop` and `mop-mord`
thin spacing is preserved.

### Bin Cancellation (TeXbook Rules 5 & 6)

Binary operators (`mbin`) change to ordinary (`mord`) based on context:

- **Left cancellation**: `mbin` becomes `mord` if preceded by:
  `leftmost`, `mbin`, `mopen`, `mrel`, `mop`, `mpunct`
- **Right cancellation**: `mbin` becomes `mord` if followed by:
  `rightmost`, `mrel`, `mclose`, `mpunct`

---

## 20. Unit System

KaTeX supports all TeX units, converting to CSS em for output.

### Absolute Units

| Unit | TeX Points | Description |
|------|-----------|-------------|
| `pt` | 1 | TeX point |
| `mm` | 7227/2540 | Millimeter |
| `cm` | 7227/254 | Centimeter |
| `in` | 72.27 | Inch |
| `bp` | 803/800 | Big (PostScript) point |
| `pc` | 12 | Pica (12pt) |
| `dd` | 1238/1157 | Didot point |
| `cc` | 14856/1157 | Cicero (12 didot) |
| `nd` | 685/642 | New didot |
| `nc` | 1370/107 | New cicero |
| `sp` | 1/65536 | Scaled point (TeX internal) |
| `px` | 803/800 | Pixel (same as bp) |

### Relative Units

| Unit | Relative To |
|------|------------|
| `em` | Current font size (`quad` metric) |
| `ex` | x-height of current font (`xHeight` metric) |
| `mu` | 1/18 of an em (math unit, scales with script size) |

### Conversion Formula

```
absolute:  value * (ptPerUnit / ptPerEm) / sizeMultiplier
mu:        value * cssEmPerMu
em:        value * quad * (unitSizeMultiplier / currentSizeMultiplier)
ex:        value * xHeight * (unitSizeMultiplier / currentSizeMultiplier)
```

For relative units in script/scriptscript style, the reference is the
*text-style* font at the current size, not the current style.

### Output Format

Values are rounded to 4 decimal places and output as CSS em: `makeEm(n)` -> `"X.XXXXem"`.

Results are clamped to `options.maxSize`.

---

## 21. Delimiter Rendering

Delimiters (parentheses, brackets, braces, etc.) are rendered at three levels
of complexity, selected based on required height.

### Three Delimiter Strategies

#### 1. Small Delimiters (`makeSmallDelim`)

Standard font glyphs from Main-Regular, restyled to text, script, or
scriptscript size. Used when the delimiter is close to the base text size.

#### 2. Large Delimiters (`makeLargeDelim`)

Fixed larger glyphs from Size1, Size2, Size3, or Size4 fonts. These are
predesigned glyphs at specific larger sizes.

#### 3. Stacked Delimiters (`makeStackedDelim`)

Built from pieces: top, repeating extension, and bottom. Used for
arbitrarily large delimiters. Rendered as an SVG inside the HTML.

### Selection Functions

- `sizedDelim(size, delim, mode, options)` -- fixed size (for `\big`, `\Big`, etc.)
- `customSizedDelim(delim, size, mode, options)` -- target a specific height
- `leftRightDelim(delim, height, depth, mode, options)` -- for `\left`/`\right`,
  automatically selects appropriate strategy

### Delimiter Sizing Sequence

For `\left`/`\right`, KaTeX tries in order:
1. Small delimiters at each style (text, script, scriptscript)
2. Large delimiters (Size1 through Size4)
3. Stacked delimiter construction

The goal is to match or exceed the target height while using the simplest
rendering possible.

---

## 22. Stretchy Elements

Stretchy elements (wide accents, extensible arrows, braces) adapt their width
to match the content they span.

### Types

- **Stretchy accents**: `\widehat`, `\widetilde`, `\overline`, `\overleftarrow`,
  `\overrightarrow`, etc.
- **Extensible arrows**: `\xrightarrow`, `\xleftarrow`, `\xleftrightarrow`, etc.
- **Horizontal braces**: `\overbrace`, `\underbrace`

### Rendering Technique

Stretchy elements use SVG with a specific technique:
1. Create an SVG element much wider than needed (400em)
2. Set `overflow: hidden` on the container
3. Size the container to match the content width
4. The visible portion shows only the correctly-sized element

This avoids distorting arrowheads while allowing the shaft to stretch.

---

## 23. SVG Geometry

SVG path data is stored in `svgGeometry.ts` (561 lines).

### Scale

All paths use a viewBox-to-em scale of **1000:1** (1000 SVG units = 1em).

### Sqrt Paths

Different sqrt radical paths for different sizes:
- `sqrtMain` -- standard size
- `sqrtSize1` through `sqrtSize4` -- progressively larger
- `sqrtTall` -- for very tall content

Each path accepts an `extraVinculum` parameter that adjusts the vinculum
(horizontal line) thickness.

### Anti-Aliasing

All SVG paths include a **double brush stroke**: a second identical path drawn
on top of the first. This combats anti-aliasing artifacts on low-resolution
displays by causing more pixels to render as fully black rather than gray.

### Delimiter Paths

`tallDelim` contains piece-based path data for constructing stacked delimiters.

---

## 24. Error Handling

### ParseError Class

```
ParseError extends Error {
  name: "ParseError"
  position: number | undefined    -- start position in input
  length: number | undefined      -- length of problematic span
  rawMessage: string              -- error message without context
  message: string                 -- full message with input context
}
```

### Error Context

The error message includes a window of surrounding input with the problematic
token underlined using combining underscores (U+0332):

```
KaTeX parse error: Undefined control sequence: \foo at position 5: \bar \f̲o̲o̲ \baz
```

Context shows up to 15 characters before and after, with `...` for truncation.

### Error Rendering

When `throwOnError` is `false`:
- Parse errors produce a `span.katex-error` containing the original expression
- The span has `title` = error message, `style.color` = `errorColor`
- Other errors (non-ParseError) are always re-thrown

---

## 25. CSS Requirements

### Root Structure

```html
<span class="katex-display">          <!-- only in displayMode -->
  <span class="katex">
    <span class="katex-mathml">...</span>  <!-- hidden MathML for A11y -->
    <span class="katex-html" aria-hidden="true">
      <span class="base">
        <span class="strut" style="height:...;vertical-align:..."></span>
        <!-- rendered content -->
      </span>
    </span>
  </span>
</span>
```

### Key CSS Classes

| Class | Purpose |
|-------|---------|
| `.katex` | Root container. Sets font: `KaTeX_Main`, size: `1.21em` |
| `.katex-display` | Display mode wrapper. Centers content, adds margins |
| `.katex-mathml` | Hidden MathML (`position: absolute`, clipped) |
| `.katex-html` | Visible HTML output |
| `.base` | Unbreakable chunk |
| `.strut` | Invisible element that sets height/depth |
| `.mord`, `.mop`, `.mbin`, `.mrel`, `.mopen`, `.mclose`, `.mpunct`, `.minner` | Atom class styling |
| `.mtight` | Tight (script/scriptscript) spacing |
| `.sizing` / `.reset-size{N}` / `.size{N}` | Size transition classes |
| `.delimsizing` | Delimiter sizing containers |
| `.nulldelimiter` | Empty delimiter placeholder |
| `.svg-align` | SVG alignment |
| `.fbox`, `.fcolorbox` | Framed boxes |
| `.overline-line`, `.underline-line` | Over/underline rules |

### Required Fonts

KaTeX requires its own web fonts:

| Font Name | Variants | Usage |
|-----------|----------|-------|
| `KaTeX_Main` | Regular, Bold, BoldItalic, Italic | Primary text font |
| `KaTeX_Math` | Italic, BoldItalic | Math italic |
| `KaTeX_AMS` | Regular | AMS symbols (blackboard bold, etc.) |
| `KaTeX_Caligraphic` | Regular, Bold | `\mathcal` |
| `KaTeX_Fraktur` | Regular, Bold | `\mathfrak` |
| `KaTeX_SansSerif` | Regular, Bold, Italic | `\mathsf` |
| `KaTeX_Script` | Regular | `\mathscr` |
| `KaTeX_Size1` through `KaTeX_Size4` | Regular | Large delimiters |
| `KaTeX_Typewriter` | Regular | `\mathtt` |

---

## 26. Unicode Support

### Unicode Symbol Replacement

`unicodeSymbols.js`: Pre-evaluated mapping of accented Unicode characters to
their base character + accent command. For example, `é` -> `e` + `\acute`.

### Unicode Accent Mapping

`unicodeAccents.js`: Maps combining diacritical marks (U+0300-U+036F) to LaTeX
accent commands:

```
U+0300 (combining grave)    -> \` (text) / \grave (math)
U+0301 (combining acute)    -> \' (text) / \acute (math)
U+0302 (combining circumflex) -> \^ (text) / \hat (math)
U+0303 (combining tilde)    -> \~ (text) / \tilde (math)
...
```

### Unicode Script Ranges

`unicodeScripts.ts`: Defines which Unicode ranges are supported:
- Latin, Cyrillic, Greek, CJK characters
- Used to determine if a character is recognized

### Unicode Super/Subscripts

`unicodeSupOrSub.ts`: Maps Unicode superscript/subscript characters to their
regular equivalents:
- `²` -> `2` (superscript)
- `₂` -> `2` (subscript)
- `ⁿ` -> `n` (superscript)
- etc.

These are converted to standard `^` / `_` notation during parsing.

### Wide Characters

`wide-character.ts`: CJK and other wide character support. Determines which
font to use for characters outside the standard Latin/Math range.

---

## 27. Porting Considerations

KaTeX is highly portable because it has **no runtime dependencies** on a browser,
DOM, or JavaScript engine for its core logic. The entire pipeline is pure
computation.

### What is Straightforward to Port

1. **Lexer**: Simple regex-based tokenizer. The regex is complex but can be
   decomposed into a state machine.
2. **Parser**: Standard recursive descent. No special language features needed.
3. **Macro expander**: Token stack + namespace scoping. Straightforward data structures.
4. **Style system**: 8 fixed styles with lookup-table transitions.
5. **Options**: Immutable struct with factory methods.
6. **Font metrics**: Static lookup tables (can be embedded as data).
7. **Symbol tables**: Static mappings.
8. **Spacing rules**: 8x8 matrix lookup.
9. **Unit conversion**: Simple arithmetic.

### What Requires Attention

1. **Output format**: KaTeX outputs HTML spans with CSS classes. A port could:
   - Output the same HTML (for web contexts)
   - Output SVG directly (for native rendering)
   - Output a custom render tree for the target platform
2. **Font metrics data**: The `fontMetricsData.js` file (2077 lines) contains
   pre-extracted metrics. These need to be converted to the target language's
   data format, or loaded from a file.
3. **SVG paths**: The `svgGeometry.ts` (561 lines) contains SVG path strings
   for sqrt radicals and delimiters. These can be used as-is if outputting SVG,
   or would need to be converted to drawing commands for native rendering.
4. **Macro definitions**: `macros.ts` (~41KB) contains thousands of macro
   definitions. Many are simple string replacements, but some are functional
   (require access to the MacroExpander API).
5. **CSS**: The visual rendering depends on specific CSS classes and KaTeX fonts.
   A non-web port would need to replicate this layout logic natively.
6. **DOM tree serialization**: The `toMarkup()` and `toNode()` methods are
   web-specific. A port needs equivalent output generation.

### Dependency-Free Core

The only external dependency is the built-in `RegExp` engine. Everything else --
parsing, layout computation, metrics -- is self-contained. This makes KaTeX
one of the most portable components in the mermaid-cli ecosystem.

### For Native Rendering (e.g. Rust)

If the goal is to render math formulas to pixels or SVG without a browser:

1. Port the lexer, parser, macro expander, and build tree logic
2. Replace HTML output with direct SVG generation:
   - Use font metrics to compute positions
   - Place glyphs at computed (x, y) coordinates
   - Use the SVG path data from svgGeometry for radicals/delimiters
3. Include the KaTeX font files (or use the metrics data to work with
   system-installed Computer Modern fonts)
4. The key challenge is text layout: computing where each glyph goes based on
   height, depth, italic correction, and kern values. KaTeX delegates this to
   the browser's CSS engine via inline styles and spans. A native port must
   implement this positioning directly.
