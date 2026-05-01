# DOMPurify Specification

## 1. Purpose

DOMPurify is an HTML/SVG/MathML sanitizer. It takes untrusted markup, parses it into a DOM tree, walks every node and attribute, removes anything not on an allowlist, and serializes the result back to a safe string (or returns a safe DOM node). Its job is to prevent XSS (cross-site scripting) attacks by stripping dangerous elements, attributes, and URI schemes while preserving safe content.

In the mermaid ecosystem, DOMPurify sanitizes user-provided diagram content before it is rendered as SVG/HTML.

## 2. Architecture Overview

```
Input (string or DOM Node)
    |
    v
[Config Resolution] -- merge defaults + user overrides + profiles
    |
    v
[DOM Parsing] -- browser DOMParser or createHTMLDocument
    |
    v
[Tree Walking] -- NodeIterator over all nodes depth-first
    |                |
    |                +-> [Element Sanitization] -- tag allowlist/blocklist check
    |                |       |
    |                |       +-> namespace validation
    |                |       +-> DOM clobbering detection
    |                |       +-> forbidden content removal
    |                |
    |                +-> [Attribute Sanitization] -- per-attribute validation
    |                        |
    |                        +-> name allowlist/blocklist check
    |                        +-> URI scheme validation
    |                        +-> data: URI gating
    |                        +-> template expression removal
    |                        +-> named property prefixing (clobbering protection)
    |                        +-> XML safety checks
    |
    v
[Output Serialization] -- innerHTML/outerHTML or DOM return
```

## 3. Factory Pattern

DOMPurify uses a factory function `createDOMPurify(window?)` that creates an isolated sanitizer instance. The default export is `createDOMPurify()` called with the global `window`. Each instance carries its own configuration state, hooks, and internal caches.

This allows creating multiple independent sanitizer instances with different configurations, and also allows DOMPurify to work in environments that provide a custom window-like object (e.g., jsdom).

The factory captures from the window object:
- `document` (for DOM parsing)
- `DocumentFragment`, `HTMLTemplateElement`
- `Node`, `Element`, `NodeFilter`
- `NamedNodeMap`, `HTMLFormElement`
- `DOMParser`
- `trustedTypes` (if available)

## 4. Configuration System

### 4.1 Config Resolution

Configuration is parsed once per `sanitize()` call (or set persistently via `setConfig()`). The `_parseConfig(cfg)` function:

1. Deep-clones the config object to prevent external mutation
2. Resolves each option against its default
3. Freezes the final config

### 4.2 Configuration Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `ALLOWED_TAGS` | string[] | all safe HTML+SVG+MathML | Restrict to only these element names |
| `ALLOWED_ATTR` | string[] | all safe attrs | Restrict to only these attribute names |
| `ALLOWED_NAMESPACES` | string[] | HTML, SVG, MathML URIs | Restrict to these XML namespaces |
| `ADD_TAGS` | string[] or function | none | Extend the default tag allowlist |
| `ADD_ATTR` | string[] or function | none | Extend the default attribute allowlist |
| `FORBID_TAGS` | string[] | none | Blocklist specific tags (overrides allowlist) |
| `FORBID_ATTR` | string[] | none | Blocklist specific attributes |
| `FORBID_CONTENTS` | string[] | see below | Elements whose children are removed with the element |
| `ADD_FORBID_CONTENTS` | string[] | none | Extend the forbidden contents list |
| `ADD_DATA_URI_TAGS` | string[] | none | Extend which elements may have data: URIs |
| `ADD_URI_SAFE_ATTR` | string[] | none | Extend which attributes are URI-safe |
| `ALLOW_ARIA_ATTR` | bool | true | Allow `aria-*` attributes |
| `ALLOW_DATA_ATTR` | bool | true | Allow `data-*` attributes |
| `ALLOW_UNKNOWN_PROTOCOLS` | bool | false | Allow non-standard URI schemes |
| `ALLOW_SELF_CLOSE_IN_ATTR` | bool | true | Allow `/>` in attribute values |
| `ALLOWED_URI_REGEXP` | RegExp | see Section 9 | Custom URI validation regex |
| `SAFE_FOR_TEMPLATES` | bool | false | Strip `{{}}`, `<%%>`, `${}` template expressions |
| `SAFE_FOR_XML` | bool | true | Strip risky characters in comments/attributes |
| `WHOLE_DOCUMENT` | bool | false | Return full document including `<html>` |
| `RETURN_DOM` | bool | false | Return a DOM node instead of string |
| `RETURN_DOM_FRAGMENT` | bool | false | Return a DocumentFragment |
| `RETURN_TRUSTED_TYPE` | bool | false | Return TrustedHTML if supported |
| `FORCE_BODY` | bool | false | Glue elements to document.body |
| `IN_PLACE` | bool | false | Sanitize a DOM node in place (mutating) |
| `KEEP_CONTENT` | bool | true | Keep text content when removing a tag |
| `SANITIZE_DOM` | bool | true | Enable DOM clobbering protection |
| `SANITIZE_NAMED_PROPS` | bool | false | Prefix `id`/`name` with `user-content-` |
| `NAMESPACE` | string | `http://www.w3.org/1999/xhtml` | Default namespace |
| `PARSER_MEDIA_TYPE` | string | `text/html` | Parser mode: `text/html` or `application/xhtml+xml` |
| `CUSTOM_ELEMENT_HANDLING` | object | none | Rules for custom elements (see Section 11) |
| `USE_PROFILES` | object/false | false | Predefined tag/attr sets (see Section 5.3) |
| `TRUSTED_TYPES_POLICY` | Policy | auto | Custom Trusted Types policy |
| `HTML_INTEGRATION_POINTS` | Record | `{annotation-xml: true}` | MathML elements that allow HTML |
| `MATHML_TEXT_INTEGRATION_POINTS` | Record | `{mi,mo,mn,ms,mtext: true}` | MathML text integration points |

### 4.3 Config Interactions

- `SAFE_FOR_TEMPLATES=true` forces `ALLOW_DATA_ATTR=false`
- `RETURN_DOM_FRAGMENT=true` forces `RETURN_DOM=true`
- `USE_PROFILES` overrides `ALLOWED_TAGS` and `ALLOWED_ATTR` completely
- `FORBID_TAGS`/`FORBID_ATTR` always override allowlists
- If `KEEP_CONTENT=true`, `#text` is added to `ALLOWED_TAGS`
- If `WHOLE_DOCUMENT=true`, `html`, `head`, `body` are added to `ALLOWED_TAGS`
- If `table` is allowed, `tbody` is automatically added

## 5. Tag Allowlists

### 5.1 HTML Tags (121 tags)

```
a, abbr, acronym, address, area, article, aside, audio, b, bdi, bdo, big,
blink, blockquote, body, br, button, canvas, caption, center, cite, code,
col, colgroup, content, data, datalist, dd, decorator, del, details, dfn,
dialog, dir, div, dl, dt, element, em, fieldset, figcaption, figure, font,
footer, form, h1-h6, head, header, hgroup, hr, html, i, img, input, ins,
kbd, label, legend, li, main, map, mark, marquee, menu, menuitem, meter,
nav, nobr, ol, optgroup, option, output, p, picture, pre, progress, q, rp,
rt, ruby, s, samp, search, section, select, shadow, slot, small, source,
spacer, span, strike, strong, style, sub, summary, sup, table, tbody, td,
template, textarea, tfoot, th, thead, time, tr, track, tt, u, ul, var,
video, wbr
```

### 5.2 SVG Tags (48 tags)

```
svg, a, altglyph, altglyphdef, altglyphitem, animatecolor, animatemotion,
animatetransform, circle, clippath, defs, desc, ellipse, enterkeyhint,
exportparts, filter, font, g, glyph, glyphref, hkern, image, inputmode,
line, lineargradient, marker, mask, metadata, mpath, part, path, pattern,
polygon, polyline, radialgradient, rect, stop, style, switch, symbol, text,
textpath, title, tref, tspan, view, vkern
```

**SVG Filter Tags** (25 tags):
```
feBlend, feColorMatrix, feComponentTransfer, feComposite, feConvolveMatrix,
feDiffuseLighting, feDisplacementMap, feDistantLight, feDropShadow, feFlood,
feFuncA, feFuncB, feFuncG, feFuncR, feGaussianBlur, feImage, feMerge,
feMergeNode, feMorphology, feOffset, fePointLight, feSpecularLighting,
feSpotLight, feTile, feTurbulence
```

**SVG Disallowed Tags** (known but blocked by default):
```
animate, color-profile, cursor, discard, font-face, font-face-format,
font-face-name, font-face-src, font-face-uri, foreignobject, hatch,
hatchpath, mesh, meshgradient, meshpatch, meshrow, missing-glyph, script,
set, solidcolor, unknown, use
```

### 5.3 MathML Tags (30 tags)

```
math, menclose, merror, mfenced, mfrac, mglyph, mi, mlabeledtr,
mmultiscripts, mn, mo, mover, mpadded, mphantom, mroot, mrow, ms, mspace,
msqrt, mstyle, msub, msup, msubsup, mtable, mtd, mtext, mtr, munder,
munderover, mprescripts
```

**MathML Disallowed Tags**:
```
maction, maligngroup, malignmark, mlongdiv, mscarries, mscarry, msgroup,
mstack, msline, msrow, semantics, annotation, annotation-xml, mprescripts, none
```

### 5.4 USE_PROFILES Shorthand

When `USE_PROFILES` is provided, it replaces `ALLOWED_TAGS` and `ALLOWED_ATTR` entirely:

| Profile | Tags Added | Attrs Added |
|---------|-----------|-------------|
| `html: true` | All HTML tags | All HTML attrs |
| `svg: true` | All SVG tags | All SVG + XML attrs |
| `svgFilters: true` | All SVG filter tags | All SVG + XML attrs |
| `mathMl: true` | All MathML tags | All MathML + XML attrs |

All profiles start with `#text` as the base tag set.

## 6. Attribute Allowlists

### 6.1 HTML Attributes (117 attrs)

```
accept, action, align, alt, autocapitalize, autocomplete,
autopictureinpicture, autoplay, background, bgcolor, border, capture,
cellpadding, cellspacing, checked, cite, class, clear, color, cols,
colspan, controls, controlslist, coords, crossorigin, datetime, decoding,
default, dir, disabled, disablepictureinpicture, disableremoteplayback,
download, draggable, enctype, enterkeyhint, exportparts, face, for,
headers, height, hidden, high, href, hreflang, id, inert, inputmode,
integrity, ismap, kind, label, lang, list, loading, loop, low, max,
maxlength, media, method, min, minlength, multiple, muted, name, nonce,
noshade, novalidate, nowrap, open, optimum, part, pattern, placeholder,
playsinline, popover, popovertarget, popovertargetaction, poster, preload,
pubdate, radiogroup, readonly, rel, required, rev, reversed, role, rows,
rowspan, spellcheck, scope, selected, shape, size, sizes, slot, span,
srclang, start, src, srcset, step, style, summary, tabindex, title,
translate, type, usemap, valign, value, width, wrap, xmlns
```

### 6.2 SVG Attributes (189 attrs)

All SVG presentation attributes, geometry attributes, and filter attributes. Includes: `accent-height`, `d`, `fill`, `stroke`, `transform`, `viewbox`, `x`, `y`, `width`, `height`, etc.

### 6.3 MathML Attributes (56 attrs)

```
accent, accentunder, align, bevelled, close, columnalign, columnlines,
columnspacing, columnspan, denomalign, depth, dir, display, displaystyle,
encoding, fence, frame, height, href, id, largeop, length, linethickness,
lquote, lspace, mathbackground, mathcolor, mathsize, mathvariant, maxsize,
minsize, movablelimits, notation, numalign, open, rowalign, rowlines,
rowspacing, rowspan, rspace, rquote, scriptlevel, scriptminsize,
scriptsizemultiplier, selection, separator, separators, stretchy,
subscriptshift, supscriptshift, symmetric, voffset, width, xmlns
```

### 6.4 XML Attributes (5 attrs)

```
xlink:href, xml:id, xlink:title, xml:space, xmlns:xlink
```

### 6.5 Special Attribute Categories

**URI-Safe Attributes**: Attributes whose values are not checked against URI validation (they're considered inherently safe):
- `alt`, `class`, `for`, `id`, `label`, `name`, `pattern`, `placeholder`, `role`, `summary`, `title`, `value`, `style`, `xmlns`

**Data URI Tags**: Elements that may contain `data:` URIs in `src`/`href`:
- Default: `audio`, `video`, `img`, `source`, `image`, `track`

**Forbidden Content Elements**: When these elements are removed, their children are also removed (not preserved):
- Default: `annotation-xml`, `audio`, `colgroup`, `desc`, `foreignobject`, `head`, `iframe`, `math`, `mi`, `mn`, `mo`, `ms`, `mtext`, `noembed`, `noframes`, `noscript`, `plaintext`, `script`, `style`, `svg`, `template`, `thead`, `title`, `video`, `xmp`

## 7. DOM Parsing

### 7.1 Document Initialization

The `_initDocument(dirty)` function parses the input string into a DOM tree:

1. **Prefix injection**: If the string does not start with whitespace, a single space or `<br/>` is prepended. This prevents the browser from silently dropping leading content.

2. **DOMParser path** (preferred): Uses `new DOMParser().parseFromString(dirtyPayload, PARSER_MEDIA_TYPE)` to create a document. For `text/html`, it parses as HTML. For `application/xhtml+xml`, it parses as strict XML.

3. **Fallback path**: If DOMParser is unavailable, uses `document.implementation.createHTMLDocument('')`, sets `doc.body.innerHTML = dirtyPayload`.

4. **Template stripping**: If `SAFE_FOR_TEMPLATES`, replaces all `{{...}}`, `<%...%>`, `${...}` in the raw input before parsing.

5. **Error detection**: For XML parsing, checks for `<parsererror>` in the result.

6. Returns `doc.body` (or `doc.documentElement` for `WHOLE_DOCUMENT`).

### 7.2 Supported Parser Media Types

- `text/html` (default) - HTML5 parsing rules, case-insensitive
- `application/xhtml+xml` - XML parsing rules, case-sensitive

### 7.3 Case Handling

- In HTML mode: tag names and attribute names are lowercased via `stringToLowerCase`
- In XHTML mode: tag names and attribute names preserve original case via `stringToString` (identity)

## 8. Tree Walking Algorithm

### 8.1 Node Iterator

DOMPurify creates a `NodeIterator` using `document.createNodeIterator()` with the filter:
```
NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_COMMENT | NodeFilter.SHOW_TEXT | NodeFilter.SHOW_PROCESSING_INSTRUCTION | NodeFilter.SHOW_CDATA_SECTION
```

This visits every element, comment, text node, processing instruction, and CDATA section.

### 8.2 Main Loop

```
while (currentNode = nodeIterator.nextNode()):
    1. _sanitizeElements(currentNode)     // check/remove bad tags
    2. _sanitizeAttributes(currentNode)   // check/remove bad attributes
    3. if currentNode.content is DocumentFragment:
         _sanitizeShadowDOM(currentNode.content)  // recurse into shadow DOM
```

### 8.3 Shadow DOM Handling

If an element has a `.content` that is a `DocumentFragment` (i.e., `<template>` elements), DOMPurify recursively sanitizes that fragment using a separate `NodeIterator` with the same element+attribute sanitization logic.

The shadow DOM sanitization has its own hook points: `beforeSanitizeShadowDOM`, `uponSanitizeShadowNode`, `afterSanitizeShadowDOM`.

## 9. Element Sanitization

The `_sanitizeElements(currentNode)` function decides whether to keep, remove, or modify each node.

### 9.1 Algorithm

```
1. Fire hook: beforeSanitizeElements(currentNode)

2. Check DOM clobbering:
   - If _isClobbered(currentNode): force-remove and return

3. Normalize tag name:
   - Apply transformCaseFunc (lowercase for HTML, identity for XHTML)
   - Strip the node name against CUSTOM_ELEMENT regex

4. Check SAFE_FOR_TEMPLATES:
   - If enabled, strip {{...}}, <%...%>, ${...} from textContent of text nodes

5. Check if tag is allowed:
   tagName = transformCaseFunc(currentNode.nodeName)

   a. If FORBID_TAGS[tagName]: remove
   b. If not ALLOWED_TAGS[tagName] and not a custom element passing CUSTOM_ELEMENT_HANDLING:
      - If KEEP_CONTENT and not FORBID_CONTENTS[tagName]:
        * Move children to parent (preserve text content)
        * Special handling: for <template>, move .content children instead
      - Remove the element itself

6. Check namespace validity via _checkValidNamespace():
   - Verify the element's namespace is in ALLOWED_NAMESPACES
   - Verify parent/child namespace transitions are valid

7. Check element type:
   - COMMENT_NODE (8): remove if tag is not explicitly allowed
   - PROCESSING_INSTRUCTION (7): always remove
   - CDATA_SECTION (4): always remove

8. Handle <select> containing <select>: remove inner <select> (browser mXSS vector)

9. Fire hook: afterSanitizeElements(currentNode)
```

### 9.2 DOM Clobbering Detection

The `_isClobbered(element)` function detects DOM clobbering attacks:

```
An element is clobbered if:
  - element.remove is not a function, OR
  - element.nodeName is not a string, OR
  - element.textContent is not a string
```

DOM clobbering occurs when an attacker creates elements with `id` or `name` attributes that shadow built-in DOM properties (e.g., `<form id="remove">` makes `form.remove` return the child element instead of the native method).

### 9.3 Namespace Validation

The `_checkValidNamespace(element)` function enforces valid namespace transitions:

```
1. Parent element determines current namespace context
2. If parent is in HTML namespace:
   - Child may remain HTML
   - Child may enter SVG if tag is <svg>
   - Child may enter MathML if tag is <math>
3. If parent is in MathML namespace:
   - Child re-enters HTML if parent is a MATHML_TEXT_INTEGRATION_POINT
     (mi, mo, mn, ms, mtext) AND child is not a MathML-namespaced tag
4. If parent is in SVG namespace:
   - Child re-enters HTML if parent is an HTML_INTEGRATION_POINT
     (annotation-xml) AND child is not an SVG-namespaced tag
5. Disallow nesting: no MathML inside SVG, no SVG inside MathML
   (except through integration points back to HTML)
```

The three recognized namespaces:
- HTML: `http://www.w3.org/1999/xhtml`
- SVG: `http://www.w3.org/2000/svg`
- MathML: `http://www.w3.org/1998/Math/MathML`

### 9.4 Force Removal

`_forceRemove(node)` removes a node by:
1. Calling `node.parentNode.removeChild(node)` if parent exists
2. Otherwise calling `node.remove()`
3. Recording the removal in `DOMPurify.removed[]`

This is used for clobbered elements and unconditionally dangerous content.

## 10. Attribute Sanitization

The `_sanitizeAttributes(currentNode)` function iterates over all attributes of an element in reverse order and decides whether to keep or remove each one.

### 10.1 Algorithm

```
1. Fire hook: beforeSanitizeAttributes(currentNode)

2. If element is clobbered (_isClobbered): return immediately

3. For each attribute (iterating backwards for safe removal):
   a. Extract: name, namespaceURI, value
   b. Apply transformCaseFunc to get lcName
   c. Trim value (except for value="..." which preserves whitespace)

   d. Fire hook: uponSanitizeAttribute(currentNode, hookEvent)
      - Hook can modify attrValue, set keepAttr=false, or forceKeepAttr=true

   e. DOM Clobbering Protection:
      - If SANITIZE_NAMED_PROPS and (name is "id" or "name"):
        Prefix value with "user-content-"

   f. XML Safety:
      - If SAFE_FOR_XML: remove attribute if value contains
        ((--!?|])>) or </(style|script|title|xmp|textarea|noscript|iframe|noembed|noframes)

   g. SVG Animation Protection:
      - If name is "attributename" and value matches "href": remove

   h. Hook veto:
      - If forceKeepAttr: skip validation, keep attribute
      - If !keepAttr: remove attribute

   i. jQuery Self-Close Protection:
      - If !ALLOW_SELF_CLOSE_IN_ATTR and value contains "/>": remove

   j. Template Safety:
      - If SAFE_FOR_TEMPLATES: strip {{...}}, <%...%>, ${...} from value

   k. Value Validation (_isValidAttribute):
      - See Section 10.2

   l. Trusted Types:
      - If attribute requires TrustedHTML or TrustedScriptURL,
        wrap value through trustedTypesPolicy

   m. Re-clobbering check:
      - After setting modified value, check if element became clobbered
      - If so, force-remove the entire element

4. Fire hook: afterSanitizeAttributes(currentNode)
```

### 10.2 Attribute Value Validation

The `_isValidAttribute(lcTag, lcName, value)` function determines if a specific attribute value is safe:

```
1. Is attribute in ALLOWED_ATTR?  -> continue checking
2. Is attribute an aria-* attr and ALLOW_ARIA_ATTR is true?  -> safe
3. Is attribute a data-* attr and ALLOW_DATA_ATTR is true?  -> safe
4. If attribute is not in allowlist and not in custom element exceptions:
   -> REJECT

5. Is attribute in URI_SAFE_ATTRIBUTES?  -> safe (no URI check needed)
6. Does value pass IS_ALLOWED_URI regex?  -> safe
7. Is it src/xlink:href/href with data: prefix on an allowed DATA_URI_TAG?  -> safe
8. Is ALLOW_UNKNOWN_PROTOCOLS true and value does NOT start with
   script:/data:?  -> safe
9. If value is empty (binary attribute)?  -> safe
10. Otherwise  -> REJECT
```

## 11. URI Validation

### 11.1 Default Allowed URI Regex

```regex
/^(?:(?:(?:f|ht)tps?|mailto|tel|callto|sms|cid|xmpp|matrix):|[^a-z]|[a-z+.\-]+(?:[^a-z+.\-:]|$))/i
```

This allows:
- `http:`, `https:`, `ftp:`, `ftps:` - web protocols
- `mailto:` - email
- `tel:`, `callto:` - telephone
- `sms:` - SMS
- `cid:` - content-ID (for email)
- `xmpp:` - messaging
- `matrix:` - Matrix protocol
- Values starting with non-alpha characters (e.g., `/path`, `#anchor`, `?query`)
- Values without a colon (no protocol)

### 11.2 Script/Data Detection

```regex
/^(?:\w+script|data):/i
```

Matches `javascript:`, `vbscript:`, `data:`, etc. Used as a blocklist when `ALLOW_UNKNOWN_PROTOCOLS` is true (blocks script-like and data protocols even when unknown protocols are generally allowed).

### 11.3 Attribute Whitespace Stripping

Before URI validation, invisible whitespace characters are stripped from the value:
```regex
/[ -   ᠎ -  　]/g
```

This prevents bypasses like `java\tscript:alert(1)`.

## 12. Custom Element Handling

Custom elements (Web Components) are handled through `CUSTOM_ELEMENT_HANDLING`:

### 12.1 Configuration

```
{
  tagNameCheck: RegExp | (tagName) => boolean | null,
  attributeNameCheck: RegExp | (attrName, tagName?) => boolean | null,
  allowCustomizedBuiltInElements: boolean
}
```

### 12.2 Basic Custom Element Detection

A tag is considered a "basic custom element" if:
1. It matches `/^[a-z][.\w]*(-[.\w]+)+$/i` (contains at least one dash, not at the start)
2. It is NOT in the reserved custom element names list

Reserved names (from the HTML spec):
```
annotation-xml, color-profile, font-face, font-face-format,
font-face-name, font-face-src, font-face-uri, missing-glyph
```

### 12.3 Custom Element Validation Flow

If a tag is not in ALLOWED_TAGS:
1. Check if it passes the basic custom element test
2. Check if it passes `tagNameCheck`
3. Check if its attributes pass `attributeNameCheck`
4. If all pass, the element and its attributes are allowed

For `is=""` attributes (customized built-in elements):
- Only allowed if `allowCustomizedBuiltInElements` is true
- The `is` attribute's value must pass `tagNameCheck`

## 13. mXSS Prevention

Mutation XSS (mXSS) occurs when the browser's parser mutates content during DOM construction in a way that produces executable code from seemingly safe input.

### 13.1 Specific Protections

1. **Nested `<select>` removal**: `<select>` inside `<select>` is removed because browsers may reparse inner content in dangerous ways.

2. **Self-closing tags in attributes**: `/>` in attribute values can trigger mXSS in jQuery 3.0. Removed unless `ALLOW_SELF_CLOSE_IN_ATTR` is true.

3. **Comment/CDATA in attributes**: Values containing `-->`, `]]>`, or closing tags for `<style>`, `<script>`, etc. are removed when `SAFE_FOR_XML` is true.

4. **SVG animated href**: An `attributename` attribute with value matching "href" could be used to animate a link to a malicious URL. Always removed.

5. **Namespace confusion**: Elements must be in valid namespace contexts (see Section 9.3). Prevents SVG-in-MathML and MathML-in-SVG confusion attacks.

6. **Template expression stripping**: `{{}}`, `<%%>`, `${}` patterns are removed in `SAFE_FOR_TEMPLATES` mode to prevent server-side template injection.

## 14. DOM Clobbering Protection

### 14.1 Detection

DOM clobbering is detected by checking that native DOM methods haven't been overwritten:

```
element is clobbered if:
  - typeof element.remove !== 'function'
  - typeof element.nodeName !== 'string'  
  - typeof element.textContent !== 'string'
```

### 14.2 SANITIZE_DOM Protection

When `SANITIZE_DOM` is true (default):
- Checks if `id` and `name` attributes would shadow DOM API properties
- Specifically protects against clobbering of `HTMLFormElement` properties
- Uses `lookupGetter` to check prototype chain for genuine getters vs. clobbered values

### 14.3 Namespace Isolation (SANITIZE_NAMED_PROPS)

When `SANITIZE_NAMED_PROPS` is true:
- All `id` and `name` attribute values are prefixed with `user-content-`
- This prevents named properties from conflicting with JavaScript global scope
- Applied during attribute sanitization before value validation

## 15. Hook System

### 15.1 Hook Points

DOMPurify provides 9 hook points, each receiving the current node and a mutable event object:

| Hook | Fired When | Event Data |
|------|-----------|------------|
| `beforeSanitizeElements` | Before checking each element | currentNode |
| `uponSanitizeElement` | During element checking | tagName, allowedTags |
| `afterSanitizeElements` | After element decision | currentNode |
| `beforeSanitizeAttributes` | Before checking attributes | currentNode |
| `uponSanitizeAttribute` | During each attribute check | attrName, attrValue, keepAttr, forceKeepAttr |
| `afterSanitizeAttributes` | After all attributes processed | currentNode |
| `beforeSanitizeShadowDOM` | Before shadow DOM recursion | fragment |
| `uponSanitizeShadowNode` | During shadow DOM node check | currentNode |
| `afterSanitizeShadowDOM` | After shadow DOM recursion | fragment |

### 15.2 Hook Capabilities

- Hooks run as an array of callbacks for each hook point
- `uponSanitizeAttribute` can modify the attribute value, set `keepAttr=false` to remove it, or set `forceKeepAttr=true` to bypass validation
- `uponSanitizeElement` can modify `tagName` and `allowedTags` to influence the decision
- Hooks execute in registration order

### 15.3 Hook Management API

```
addHook(entryPoint, hookFunction)     // Register a hook
removeHook(entryPoint, hookFunction)  // Remove specific hook, or last if no function given
removeHooks(entryPoint)               // Remove all hooks at a point
removeAllHooks()                      // Clear all hooks
```

## 16. Output Modes

### 16.1 String Output (default)

Returns `body.innerHTML` (or `body.outerHTML` if `WHOLE_DOCUMENT`).

If `WHOLE_DOCUMENT` and `!doctype` is in ALLOWED_TAGS, the doctype declaration is prepended:
```
<!DOCTYPE html>
```
The doctype name is validated against `/^html$/i`.

### 16.2 DOM Output (RETURN_DOM)

Returns the `<body>` element directly.

### 16.3 DocumentFragment Output (RETURN_DOM_FRAGMENT)

Creates a new `DocumentFragment`, moves all children from body into it, returns the fragment.

If `shadowroot` or `shadowrootmode` attributes are allowed, the returned node is imported via `document.importNode()` to ensure shadow roots are properly cloned.

### 16.4 TrustedHTML Output (RETURN_TRUSTED_TYPE)

If the browser supports Trusted Types and a policy is configured, the string output is wrapped via `trustedTypesPolicy.createHTML(serializedHTML)`.

### 16.5 In-Place Sanitization (IN_PLACE)

When input is a DOM Node and `IN_PLACE=true`:
- The node is mutated directly (no copy)
- Returns the same node reference
- Throws if the root node's tag is forbidden

### 16.6 Fast Path

If the input is a string that:
- Does not contain `<`
- `RETURN_DOM` is false
- `SAFE_FOR_TEMPLATES` is false  
- `WHOLE_DOCUMENT` is false

Then it skips DOM parsing entirely and returns the string as-is (optionally wrapped in TrustedHTML).

## 17. Prototype Pollution Protection

DOMPurify takes extensive measures against prototype pollution:

### 17.1 Unapply Pattern

All built-in methods are captured at initialization time and called via `unapply()`:

```
const arrayForEach = unapply(Array.prototype.forEach);
const stringToLowerCase = unapply(String.prototype.toLowerCase);
const regExpTest = unapply(RegExp.prototype.test);
// etc.
```

`unapply(fn)` creates a wrapper that:
1. Resets `lastIndex` on RegExp instances (prevents state leakage)
2. Calls the original method via `Reflect.apply` with explicit `this` binding

This prevents attackers from overriding `Array.prototype.forEach` etc.

### 17.2 Config Protection

- Config is deep-cloned via `clone()` before use
- Arrays in config are cleaned via `cleanArray()` to patch sparse/prototype-holed arrays
- Cloned objects have `null` prototype via `Object.create(null)`
- Final config is frozen

### 17.3 Set Construction

Allowlist sets are `Record<string, boolean>` objects with `null` prototype. The `addToSet()` function:
1. Sets prototype to null via `setPrototypeOf(set, null)`
2. Iterates the array, setting `set[element] = true`
3. Returns the modified set

Lookup is via `in` operator or direct property access, which is safe because there's no prototype chain.

## 18. Removed Elements Tracking

`DOMPurify.removed` is an array populated during sanitization with objects describing what was removed:

For removed elements:
```
{ element: Node }
```

For removed attributes:
```
{ attribute: Attr, from: Element }
```

This is reset to `[]` at the start of each `sanitize()` call.

## 19. Public API

```
sanitize(dirty, cfg?)              // Main sanitization function
setConfig(cfg)                     // Persist config across calls
clearConfig()                      // Reset to defaults
isValidAttribute(tag, attr, value) // Check one attribute without full sanitize
addHook(entryPoint, hookFunction)  // Register a sanitization hook
removeHook(entryPoint, hookFunction?) // Remove a hook
removeHooks(entryPoint)            // Remove all hooks at entry point
removeAllHooks()                   // Clear all hooks
version                            // Version string
removed                            // Array of removed elements/attributes
isSupported                        // Whether this environment can run DOMPurify
```

## 20. Relevance to Mermaid Port

### 20.1 What Mermaid Uses DOMPurify For

Mermaid passes user-provided diagram descriptions through DOMPurify before rendering them as SVG. This prevents XSS in diagram labels, titles, and other text content that gets embedded in the SVG output.

### 20.2 Key Configuration for Mermaid

Mermaid typically uses DOMPurify with SVG-heavy configurations:
- SVG tags and attributes must be allowed
- MathML support is needed for KaTeX-rendered equations
- Custom `ADD_TAGS` and `ADD_ATTR` for mermaid-specific elements
- `USE_PROFILES: { svg: true, html: true }` or equivalent

### 20.3 Porting Considerations

For a pure Rust implementation:

1. **No DOM parser needed**: A Rust port does not need a full browser DOM. Instead, use an HTML/XML parser library (like `html5ever` or `lol_html`) to parse markup into a tree structure, then apply the same allowlist/blocklist logic.

2. **Core logic is data-driven**: The heart of DOMPurify is just set lookups against allowlists of tag names, attribute names, and URI patterns. This is straightforward to port.

3. **Regex patterns port directly**: All validation regexes (URI schemes, data attributes, aria attributes, template expressions, whitespace stripping) are standard regex and work identically in Rust.

4. **Hook system is optional**: Hooks are used for extensibility in the JS ecosystem. A Rust port for mermaid's specific needs may not need the full hook system — just hard-code the sanitization rules mermaid requires.

5. **Namespace validation is critical**: The SVG/MathML/HTML namespace transition rules must be faithfully ported, as mermaid output contains all three namespaces.

6. **Template expressions irrelevant**: `SAFE_FOR_TEMPLATES` mode is for server-side template engines and is not needed for mermaid's use case.

7. **Trusted Types irrelevant**: This is a browser-specific API, not applicable to a native Rust renderer.

8. **DOM clobbering irrelevant**: DOM clobbering is a browser-specific attack vector. A Rust implementation that doesn't use a browser DOM doesn't need this protection.

9. **The core portable logic**: Tag allowlist check, attribute allowlist check, URI validation, namespace transition rules, forbidden content removal. This is ~200 lines of actual logic, with the rest being browser API wrappers and defense-in-depth.
