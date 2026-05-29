# SVG Animation Support for Bevy Sprite Export — Design

- **Status:** Approved design (pre-implementation)
- **Date:** 2026-05-29
- **Branch:** `feat/animation`
- **Repository status:** This is a permanent personal fork of resvg. It must **never** be pushed to the upstream `linebender/resvg` repository. All animation work lives on `feat/animation`.

## 1. Goal and use case

Add SMIL-based animation support to this fork of resvg, scoped specifically to **exporting Bevy sprite animations**. The author writes a single animated SVG file (using standard SMIL animation elements, previewable in any browser), and the library renders it into a sequence of frames sampled across the animation timeline. The frame count is caller-controlled so the same source can produce a chunky 12-frame sprite or a smooth 60-frame sprite.

This is **not** a full SMIL specification implementation. It is a pragmatic subset chosen to cover the common needs of game sprite animation.

Primary outputs:
1. An in-memory frame sequence (`Vec<tiny_skia::Pixmap>`).
2. A packed **uniform-grid sprite sheet** (single PNG) directly consumable by Bevy's `TextureAtlasLayout::from_grid` with no sidecar metadata file.

## 2. Background: relevant resvg architecture

resvg deliberately splits parsing and rendering into two crates:

- **`usvg`** parses SVG into an immutable, `Arc`-backed `Tree`. Shapes (`<rect>`, `<circle>`, …) are flattened into `tiny_skia_path::Path` data at parse time, and every node's absolute transform and bounding boxes are precomputed. The tree is immutable after parsing.
- **`resvg`** rasterizes a `Tree` to a `tiny_skia::Pixmap` via `resvg::render(&Tree, Transform, &mut PixmapMut)`. Rendering is stateless and has no concept of time.

There is currently **no** animation, time, or frame concept anywhere. SMIL elements are silently dropped: the svgtree parser discards any element/attribute whose name is not in the generated `EId`/`AId` enums.

The two facts that drive the whole design:
1. The `Tree` is **static and fully baked**, so "render frame at time *t*" means "produce the static tree corresponding to the animation frozen at *t*, then render it normally."
2. Every converter attribute read funnels through `SvgNode::attribute` / `resolve_transform`, giving a **single interception point** for substituting animated values without touching the dozens of element-specific converters.

Key existing dependency: **`kurbo` 0.13** is already a `usvg` dependency (used for `textPath` curve layout). All curve and spline math reuses kurbo — no new dependency, and explicitly **not** Bevy itself (resvg is engine-agnostic and only emits pixels).

## 3. Requirements (confirmed decisions)

| Decision | Choice |
|---|---|
| Animation source format | **SMIL subset** (`<animate>`, `<animateTransform>`, `<animateMotion>`, `<set>`, `<mpath>`) |
| Animatable scope | Transforms, opacity & color, discrete show/hide, geometry attributes, motion-path |
| Curve/spline math | Reuse **kurbo** (already in-tree); never hand-roll; never depend on Bevy |
| Output forms | In-memory `Vec<Pixmap>` + uniform-grid sprite-sheet PNG |
| Interpolation fidelity | Full `calcMode` set: `discrete`, `linear`, `paced`, `spline` (+ `keyTimes`, `keySplines`) |
| Rendering architecture | **Approach 1 — Freeze + convert** (re-run conversion per frame with time injected) |
| CLI | Library-only for v1 (CLI flags are a future follow-up) |

## 4. Architecture: Approach 1 — Freeze + convert

Pipeline:

1. **Parse once (`AnimatedSvg::parse`)** — parse the SVG with animation elements retained; extract a typed timeline and compute the natural duration; cache the source bytes.
2. **Per frame (`tree_at(time)`)** — clone `Options`, set `animation_time = Some(time)`, and re-run usvg's existing conversion. The attribute choke point substitutes each animation's value at *t*; `resolve_transform` composes `<animateTransform>`/`<animateMotion>` contributions. The result is an ordinary static `usvg::Tree`.
3. **Render (`resvg::render`)** — rasterize each frozen tree to a `Pixmap` with **unmodified** resvg.
4. **Pack** — assemble frames into a uniform-grid sprite sheet.

Why this approach:
- Geometry, motion-path, transform, opacity and color all work through the existing, battle-tested converters and post-processing (geometry is re-tessellated for free because `<rect>` etc. read their dimensions through the choke point).
- resvg's renderer is **untouched**, preserving the parse/render separation that is central to the project.
- Non-animated behavior is **byte-for-byte unchanged**: the machinery engages only when `animation_time` is `Some` and animation elements are present, so the existing ~1600-test regression suite is unaffected.
- Reproducibility is preserved: evaluation is pure math over *t* with no clocks or randomness.

Cost: conversion re-runs per frame (re-doing text shaping and `<use>` expansion each frame). For offline sprite export at 12–120 frames this is negligible relative to rasterization, and it is optimizable later (retain the parsed document, or cache static branches) without changing the public API.

## 5. Module layout and file changes

**`usvg`:**
- `crates/usvg/codegen/elements.txt` — add `animate`, `animateMotion`, `animateTransform`, `set`, `mpath`.
- `crates/usvg/codegen/attributes.txt` — add the SMIL attributes not already present: `attributeName`, `attributeType`, `begin`, `dur`, `end`, `repeatCount`, `repeatDur`, `calcMode`, `keyTimes`, `keySplines`, `keyPoints`, `values`, `from`, `to`, `by`, `additive`, `accumulate`, `path`. (`fill`, `rotate`, `href` already exist and are reused with SMIL meaning in animation context.)
- Regenerate `crates/usvg/src/parser/svgtree/names.rs` via the codegen binary (`cargo run` in `crates/usvg/codegen`). This file is generated; do not edit by hand.
- `crates/usvg/src/parser/svgtree/parse.rs` — retain animation elements/attributes (they are no longer dropped once present in `EId`/`AId`).
- `crates/usvg/src/parser/svgtree/mod.rs` — add `animation_time: Option<f64>` to `Document`; make `attribute`, `try_attribute`, `has_attribute`, `find_attribute`, and `resolve_transform` animation-aware (guarded by `animation_time.is_some()`).
- `crates/usvg/src/parser/options.rs` — add `animation_time: Option<f64>` to `Options` (default `None`).
- `crates/usvg/src/parser/animation/` — **new module**: SMIL parsing into typed structures, timing/duration computation, and the interpolation engine.
- `crates/usvg/src/lib.rs` (or a new `animation` module) — public `AnimatedSvg` type.

**`resvg`:**
- `crates/resvg/src/animation.rs` — **new module**: `FrameOptions`, `TimeSpan`, `SpriteSheet`, `SheetOptions`, `render_frames`, `pack_sprite_sheet`, `render_sprite_sheet`.
- `crates/resvg/src/lib.rs` — re-export the animation API.
- `crates/resvg/examples/animation.rs` — worked example (spinner → sprite sheet PNG).

Naming convention for all new code: full, descriptive identifiers (no abbreviations, no single-character names beyond conventional loop counters); no decorative/banner comments.

## 6. Public API

In `usvg`:

```rust
/// A reusable handle that parses an SVG and its SMIL timing once,
/// then produces a frozen static `Tree` for any instant.
pub struct AnimatedSvg { /* owns source bytes + cached duration + is_animated flag */ }

impl AnimatedSvg {
    pub fn parse(data: &[u8], options: &Options) -> Result<AnimatedSvg, Error>;
    pub fn is_animated(&self) -> bool;
    /// Natural timeline length in seconds (one loop for indefinite repeats).
    pub fn duration(&self) -> f64;
    /// Produce the static tree corresponding to the animation frozen at `time` (seconds).
    pub fn tree_at(&self, time: f64, options: &Options) -> Result<Tree, Error>;
}

// New field on the existing Options struct (default None => unchanged static behavior).
// pub animation_time: Option<f64>
```

In `resvg`:

```rust
pub struct TimeSpan {
    pub start: f64,
    pub end: f64,
}

pub struct FrameOptions {
    /// Number of frames to emit (e.g. 12 for chunky sprites, 60 for smooth).
    pub frame_count: usize,
    /// Sampling window; defaults to 0..duration.
    pub time_span: Option<TimeSpan>,
    /// Output pixel size per frame; defaults to the tree size. Smaller = downscaled sprites.
    pub size: Option<tiny_skia::IntSize>,
    /// false (default) => seamless loop (frame N not duplicated at the loop point).
    /// true => both endpoints included (one-shot clips).
    pub endpoint_inclusive: bool,
    /// Root transform applied to every frame.
    pub transform: tiny_skia::Transform,
}

pub fn render_frames(
    animation: &usvg::AnimatedSvg,
    options: &usvg::Options,
    frame_options: &FrameOptions,
) -> Result<Vec<tiny_skia::Pixmap>, Error>;

pub struct SpriteSheet {
    pub pixmap: tiny_skia::Pixmap,
    pub columns: u32,
    pub rows: u32,
    pub frame_width: u32,
    pub frame_height: u32,
    pub frame_count: usize,
}

pub struct SheetOptions {
    /// Columns in the grid; None => ceil(sqrt(frame_count)) (near-square).
    pub columns: Option<u32>,
    /// Inter-cell spacing in pixels (default 0).
    pub padding: u32,
}

pub fn pack_sprite_sheet(
    frames: &[tiny_skia::Pixmap],
    sheet_options: &SheetOptions,
) -> Option<SpriteSheet>;

pub fn render_sprite_sheet(
    animation: &usvg::AnimatedSvg,
    options: &usvg::Options,
    frame_options: &FrameOptions,
    sheet_options: &SheetOptions,
) -> Result<SpriteSheet, Error>;
```

End-to-end usage:

```rust
let options = usvg::Options::default();
let data = std::fs::read("spinner.svg")?;

let animation = usvg::AnimatedSvg::parse(&data, &options)?;
let sheet = resvg::render_sprite_sheet(
    &animation, &options,
    &resvg::FrameOptions { frame_count: 12, ..Default::default() },
    &resvg::SheetOptions { columns: None, padding: 0 },
)?;
sheet.pixmap.save_png("spinner_sheet.png")?;
// Bevy: TextureAtlasLayout::from_grid(
//   uvec2(sheet.frame_width, sheet.frame_height), sheet.columns, sheet.rows, None, None)
```

## 7. SMIL subset and timeline model

### 7.1 Elements and target binding

Parsed elements: `<animate>`, `<animateTransform>`, `<animateMotion>`, `<set>`, `<mpath>`.

**Target binding is child-of-target only (v1):** an animation animates the element it is nested inside. During conversion of an element, we inspect its animation children. This requires no id-resolution. The single cross-reference supported is `<animateMotion>`'s path, via inline `path="M…"` or `<mpath xlink:href="#trackId">` pointing at a `<path>`.

### 7.2 Typed structures (in `parser/animation/`)

```rust
struct ParsedAnimation {
    kind: AnimationKind,
    timing: Timing,
    values: ValueList,                          // from/to/by normalized into a value list
    key_times: Option<Vec<f64>>,
    calc_mode: CalcMode,                        // Discrete | Linear | Paced | Spline
    key_splines: Option<Vec<kurbo::CubicBez>>,  // one easing curve per value segment
    additive: Additive,                         // Replace (default) | Sum
    accumulate: Accumulate,                     // None (default) | Sum
}

enum AnimationKind {
    Attribute { name: AId },                       // <animate>/<set> on a named attribute
    Transform { transform_type: TransformType },   // translate | scale | rotate | skewX | skewY
    Motion    { path: MotionPath, rotate: MotionRotate },
}

struct Timing {
    begin: f64,                  // seconds (numeric offset; default 0)
    duration: Option<f64>,       // one iteration; None = indefinite
    repeat: Repeat,              // Count(f64) | Indefinite
    end: Option<f64>,
    fill: AnimationFill,         // Remove (default) | Freeze
}

enum CalcMode { Discrete, Linear, Paced, Spline }
enum Additive { Replace, Sum }
enum Accumulate { None, Sum }
```

### 7.3 Timing and duration semantics

- **`begin`**: numeric clock offset only (`"0.5s"`, `"250ms"`, `"1"`); a list takes the earliest. Event/syncbase begins (`begin="click"`, `begin="other.end"`) are a non-goal: warn and treat as 0.
- **`dur`**: clock value; `"indefinite"` holds the start value.
- **Active duration**: `duration × repeatCount` when finite; `repeatCount="indefinite"` (or `repeatDur="indefinite"`) is treated as **one clean iteration** for timeline-length purposes, so a looping sprite exports exactly one loop. `end` can cap the active end.
- **`fill`**: `freeze` holds the final value past the active end; `remove` (default) reverts to the base value. Before `begin`, the base value applies.
- **Local time / iteration**: `active_time = time − begin`; iteration `= floor(active_time / duration)` (capped by `repeatCount`); iteration progress `= (active_time mod duration) / duration ∈ [0,1]`.
- **`accumulate="sum"`**: add `iteration × (last_value − first_value)` onto the result (numeric, transform, motion).
- **Total timeline duration** = `max(begin + active_duration)` across all animations. `FrameOptions.time_span` overrides it (wider span captures multiple loops; sub-range exports a slice).

## 8. Freeze-convert mechanism

### 8.1 Time injection

`AnimatedSvg::tree_at(time, options)` clones `options`, sets `animation_time = Some(time)`, and runs the normal conversion. `svgtree::parse_tree` copies `animation_time` onto the `Document`. Because every `SvgNode` holds `&Document`, the current time is reachable from the universal attribute accessor.

**Guard invariant:** when `animation_time` is `None`, every new branch is skipped, producing byte-for-byte identical output and preserving static-parse performance.

### 8.2 Attribute substitution (single choke point)

```rust
// SvgNode::attribute
pub fn attribute<T: FromValue>(&self, attribute_id: AId) -> Option<T> {
    if let Some(time) = self.document.animation_time {
        if let Some(animated_value) = self.evaluate_animated_attribute(attribute_id, time) {
            return T::parse(*self, attribute_id, &animated_value);
        }
    }
    // existing static path, unchanged
}
```

`evaluate_animated_attribute(attribute_id, time)` scans this node's animation children for one whose `attributeName` matches `attribute_id`, evaluates it at `time`, and returns the value **formatted as the string the existing typed parser already expects** (`opacity → "0.73"`, `fill → "#ff8800"`, `width → "18.5"`). This reuses every existing typed parser/validator with no duplication. `has_attribute`, `try_attribute`, and `find_attribute` receive the same treatment so presence checks, inherited paint (`fill`/`stroke`), and animated `visibility`/`display` (the `<set>` show/hide case via `is_visible_element`) all resolve correctly.

Rationale for the string round-trip: it maximally reuses existing parsing/validation, and the animatable attribute set is bounded and string-expressible. Hot attributes can be specialized to typed evaluation later without API change.

### 8.3 Transform composition

`resolve_transform` composes transforms (which do not round-trip through a string):

1. Start from the statically parsed `transform` attribute (identity if absent) — this is the underlying value.
2. For each active `<animateTransform>` in document order, compute its matrix at *t* (by `type`: `translate`, `scale`, `rotate` with optional center, `skewX`, `skewY`) and post-multiply. `additive="replace"` (default) replaces the underlying value for the first contribution; `additive="sum"` multiplies onto it; subsequent contributions layer in order. `accumulate="sum"` adds repeat-iteration offsets.
3. Add the `<animateMotion>` contribution: `translate` to the point at *t* along the motion path, plus `rotate` to the path tangent when `rotate="auto"`/`"auto-reverse"` (or a fixed angle).

## 9. Interpolation engine (pure functions)

Each animation normalizes to `(key_times[], values[])`: `from`/`to` → `[from, to]`; `from`/`by` → `[from, from+by]`; `to`-only → `[base, to]`; `values="a;b;c"` → those values with even `key_times` unless `keyTimes` is given.

Given iteration progress `p ∈ [0,1]`:

| `calcMode` | Behavior |
|---|---|
| `discrete` | Step to `values[i]` for the active segment (drives `<set>` and show/hide). |
| `linear` (default) | Locate the segment, lerp by local progress. |
| `spline` | Remap local progress through the segment's cubic-Bézier `keySplines[i]`, then lerp. |
| `paced` | Ignore `key_times`; place values by cumulative distance (per-type metric) for constant velocity, then lerp. |

Per-type interpolation (`lerp`):
- **number / length** (opacity, width, r, stroke-width, x, y, cx, cy, rx, ry, gradient stop offset): scalar lerp.
- **color** (fill, stroke): per-channel lerp in **sRGB** (pragmatic default; linearRGB is a non-goal). Output an `rgb()`/hex string.
- **paint**: only color↔color interpolates; gradient/`url()` paints use a discrete swap (warn).
- **transform params**: lerp the tuple by `type`, then build the matrix.
- **motion**: position + tangent along the path via kurbo arc-length parameterization.

`accumulate="sum"`: add `iteration × (last − first)`.

**kurbo usage** (the "use a library" requirement; same pattern as `text/layout.rs`):
- Motion-path: convert the path to `kurbo::BezPath`; use `ParamCurveArclen` for length-parameterized position and `ParamCurveDeriv` for the tangent. This also makes `calcMode="paced"` on motion essentially free.
- Spline easing: `kurbo::CubicBez` evaluates/solves the timing curve (the same solve CSS `cubic-bezier()` uses).

## 10. Output

### 10.1 Frame sampling

`render_frames` derives sample times from `frame_count`, `time_span` (default `0..duration`), and `endpoint_inclusive`:

- `endpoint_inclusive = false` (default, seamless loop): `time_i = start + (i / frame_count) · span_length`, `i ∈ 0..frame_count`. The loop point is omitted so frame *N* does not duplicate frame 0.
- `endpoint_inclusive = true`: `time_i = start + (i / (frame_count − 1)) · span_length` (both ends; `frame_count == 1` yields `start`).

Each `time_i` → `animation.tree_at(time_i)` → `resvg::render` into a `Pixmap` of `size` (default tree size).

Degenerate cases: `frame_count == 0` is an error; a non-animated SVG or `duration ≤ 0` yields `frame_count` identical static frames (warn when `frame_count > 1`).

### 10.2 Sprite-sheet packing

Frames are uniform, so packing is a grid: `columns = sheet_options.columns.unwrap_or(ceil(sqrt(frame_count)))`; `rows = ceil(frame_count / columns)`. Cell *i* is drawn at `(column · (frame_width + padding), row · (frame_height + padding))`; trailing cells stay transparent. `SpriteSheet` carries `columns`, `rows`, `frame_width`, `frame_height`, `frame_count` — everything Bevy's `TextureAtlasLayout::from_grid` needs (with optional `padding`).

## 11. Error handling and edge cases

Following resvg's lenient philosophy (render what you can):
- A single malformed animation (bad timing/values, missing `<mpath>` target, mismatched `keyTimes`/`keySplines` counts) is logged and skipped; the rest of the SVG still renders. It never fails the whole render.
- `Result::Err` is reserved for hard failures: unparseable SVG, I/O errors, `frame_count == 0`.
- Interpolation guards against non-finite results (consistent with the recent `f32_bound` fix), clamping where appropriate.
- Determinism/reproducibility is preserved (pure functions of *t*).
- **Invariant:** `animation_time = None` ⇒ unchanged behavior; all new code is additive and guarded.

## 12. Testing strategy

- **Unit (usvg, pure functions; test-driven, red→green):** interpolation across every `calcMode` plus `keyTimes` and `keySplines`; timing (active duration, repeat, `fill` freeze/remove, `begin` offset); value formatting per type.
- **usvg integration:** a fixture SVG (spinning + fading rect) asserting `tree_at(0.0)`, `tree_at(0.25)`, `tree_at(0.5)` produce expected node transform (rotate ≈ 180° at half), opacity, and re-tessellated geometry.
- **resvg integration:** `render_frames` length and per-frame dimensions; pixel probes at key frames (e.g. a region covered only when rotated); `pack_sprite_sheet` grid math (cell *(row, column)* equals frame *i*); a few minimal golden PNGs (resvg already does PNG regression).
- **Regression:** the full existing suite must pass unchanged.
- **Example:** `crates/resvg/examples/animation.rs` (spinner → sprite sheet) doubles as a smoke test and documentation.

## 13. Non-goals for v1 (clean future extensions)

- Path-`d` morphing (outline interpolation of `<path>`).
- `href`-targeting an animation to a non-parent element.
- Event/syncbase/wallclock `begin`, `restart`, `min`/`max`.
- CSS `@keyframes` / `transition` animation.
- linearRGB color interpolation and the `color-interpolation` property.
- Deprecated `<animateColor>` (use `<animate>` on a color attribute instead).
- Per-frame conversion caching (retain the parsed document, or cache static branches) — a pure performance optimization with no API impact.
- CLI flags for animation export.
- Loose per-frame PNG file output.

## 14. Key code references

Discovered during exploration; anchors for implementation:

- Workspace members: `Cargo.toml` (`crates/usvg`, `crates/resvg`, `crates/c-api`, `crates/usvg/codegen`).
- `usvg::Tree` / `Node` / `Group` / `Path`: `crates/usvg/src/tree/mod.rs` (`Tree` ~1584, `Node` ~897, `Group` ~1026, `Path` ~1273, `Image` ~1505).
- `Transform` re-export: `crates/usvg/src/tree/geom.rs:6` (`tiny_skia_path::Transform`).
- Parser entry points: `crates/usvg/src/parser/mod.rs` (`from_data` ~98, `from_str` ~146, `from_xmltree` ~159).
- svgtree drop of unknown elements/attributes: `crates/usvg/src/parser/svgtree/parse.rs` (~134–144, 172–189, 239–241).
- svgtree node/attribute accessors (choke point): `crates/usvg/src/parser/svgtree/mod.rs` (`SvgNode::attribute` ~279, `has_attribute` ~338, `find_attribute` ~369).
- Codegen: `crates/usvg/codegen/elements.txt`, `crates/usvg/codegen/attributes.txt`, `crates/usvg/codegen/main.rs`; output `crates/usvg/src/parser/svgtree/names.rs`.
- Converter: `crates/usvg/src/parser/converter.rs` (`State` ~28, `Cache` ~45, `convert_children` ~565, `convert_element` ~577, `convert_element_impl` ~609, `convert_group` ~739, `resolve_transform` use ~755).
- Options: `crates/usvg/src/parser/options.rs`.
- Render entry: `crates/resvg/src/lib.rs:34` (`render`); node traversal `crates/resvg/src/render.rs`.
- kurbo precedent: `crates/usvg/src/text/layout.rs:5`, `crates/usvg/src/parser/text.rs:6` (`ParamCurve`, `ParamCurveArclen`, `ParamCurveDeriv`).
