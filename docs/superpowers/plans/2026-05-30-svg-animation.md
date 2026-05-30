# SVG Animation (SMIL → Bevy Sprite Export) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a pragmatic SMIL animation subset to this resvg fork that samples an animated SVG into N frames across its timeline and packs them into a uniform-grid sprite sheet for Bevy.

**Architecture:** "Freeze + convert." `usvg::Options` gains `animation_time: Option<f64>`. When set, after the svgtree `Document` is built we run a **bake pass** that evaluates every SMIL animation at that instant and writes the resulting values into a per-`Document` **override map** keyed by `(NodeId, AId)`. The `SvgNode::attribute`/`has_attribute` choke point consults that map first, so the existing converters produce an ordinary static `Tree` frozen at time *t* — geometry is re-tessellated for free, transforms bake into a single `matrix(...)` override, and `resvg::render` is untouched. `AnimatedSvg` (usvg) caches the source + timeline and yields a `Tree` per instant; `resvg::animation` loops it into frames and packs the sheet. With `animation_time = None`, behavior is byte-for-byte unchanged.

**Tech Stack:** Rust (edition 2024), usvg/resvg/tiny-skia 0.12, svgtypes 0.16, **kurbo 0.13** (already a usvg dependency — used for motion-path arc-length and spline easing; no new deps, no Bevy dependency). Tests via `cargo test`.

**Spec:** `docs/superpowers/specs/2026-05-29-svg-animation-design.md`. This plan refines spec §8.2/§8.3 from "scan children per attribute read" to "pre-bake into an override map" — same architecture, but it resolves the `FromValue` `&'a str` lifetime and avoids per-read scanning.

---

## File structure

**usvg (parse + freeze + timeline):**
- `crates/usvg/codegen/elements.txt` — *modify*: add SMIL element names.
- `crates/usvg/codegen/attributes.txt` — *modify*: add SMIL attribute names.
- `crates/usvg/codegen/main.rs` — *modify*: add `Eq, Hash` to the generated enum derives.
- `crates/usvg/src/parser/svgtree/names.rs` — *regenerated* (do not hand-edit).
- `crates/usvg/src/parser/svgtree/mod.rs` — *modify*: `Document.animation_overrides`, `NodeId` derives, `SvgNode::id` visibility, override-aware `attribute`/`has_attribute`, `insert_animation_override`.
- `crates/usvg/src/parser/svgtree/parse.rs` — *modify*: initialize the new `Document` field.
- `crates/usvg/src/parser/options.rs` — *modify*: add `animation_time: Option<f64>`.
- `crates/usvg/src/parser/mod.rs` — *modify*: run the bake pass in `from_xmltree`; declare `animation` module; re-export `AnimatedSvg`.
- `crates/usvg/src/parser/animation/mod.rs` — *create*: module root + re-exports.
- `crates/usvg/src/parser/animation/timing.rs` — *create*: clock parsing, `Timing`, phase sampling.
- `crates/usvg/src/parser/animation/interpolate.rs` — *create*: `CalcMode`, `Easing`, `interpolate_components`.
- `crates/usvg/src/parser/animation/element.rs` — *create*: `ParsedAnimation` (parse + evaluate), transform/motion matrices.
- `crates/usvg/src/parser/animation/bake.rs` — *create*: `apply_animations`, duration scan.
- `crates/usvg/src/parser/animation/animated_svg.rs` — *create*: public `AnimatedSvg`.

**resvg (frames + sheet):**
- `crates/resvg/src/animation.rs` — *create*: `TimeSpan`, `FrameOptions`, `SpriteSheet`, `SheetOptions`, `render_frames`, `pack_sprite_sheet`, `render_sprite_sheet`.
- `crates/resvg/src/lib.rs` — *modify*: `mod animation; pub use animation::*;`.
- `crates/resvg/examples/animation.rs` — *create*: worked example.
- `crates/resvg/tests/fixtures/spinner.svg` — *create*: test/example fixture.

All new identifiers use full descriptive names (no abbreviations, no single-character names beyond loop counters); no banner/divider comments.

---

## Phase 1 — svgtree foundations

### Task 1: Retain SMIL elements/attributes via codegen

**Files:**
- Modify: `crates/usvg/codegen/elements.txt`
- Modify: `crates/usvg/codegen/attributes.txt`
- Modify: `crates/usvg/codegen/main.rs:123`
- Regenerate: `crates/usvg/src/parser/svgtree/names.rs`
- Test: `crates/usvg/src/parser/svgtree/mod.rs` (inline `#[cfg(test)]`)

- [ ] **Step 1: Append SMIL element names** to the end of `crates/usvg/codegen/elements.txt` (one per line, no blank line at EOF beyond existing):

```
animate
animateMotion
animateTransform
set
mpath
```

- [ ] **Step 2: Append SMIL attribute names** to the end of `crates/usvg/codegen/attributes.txt` (one per line). Only those not already present — `fill`, `rotate`, `href` already exist and are reused:

```
attributeName
attributeType
begin
dur
end
repeatCount
repeatDur
calcMode
keyTimes
keySplines
keyPoints
values
from
to
by
additive
accumulate
path
```

- [ ] **Step 3: Add `Eq, Hash` to generated enums** so `AId`/`EId` can key a HashMap. In `crates/usvg/codegen/main.rs:123` change:

```rust
    writeln!(f, "#[derive(Clone, Copy, PartialEq)]")?;
```
to
```rust
    writeln!(f, "#[derive(Clone, Copy, PartialEq, Eq, Hash)]")?;
```

- [ ] **Step 4: Regenerate `names.rs`**

Run: `(cd crates/usvg/codegen && cargo run)`
Expected: exits 0; `crates/usvg/src/parser/svgtree/names.rs` now contains `Animate`, `AnimateMotion`, `AnimateTransform`, `Set`, `Mpath` in `EId` and the new `AId` variants, with `#[derive(Clone, Copy, PartialEq, Eq, Hash)]`.

- [ ] **Step 5: Write a retention test** confirming `<animate>` children survive into the svgtree. Add to the bottom of `crates/usvg/src/parser/svgtree/mod.rs`:

```rust
#[cfg(test)]
mod animation_retention_tests {
    use super::*;

    #[test]
    fn animate_child_is_retained_in_svgtree() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">
            <rect width="10" height="10">
                <animate attributeName="opacity" from="1" to="0" dur="1s"/>
            </rect>
        </svg>"#;
        let xml = roxmltree::Document::parse(svg).unwrap();
        let doc = Document::parse_tree(&xml, None).unwrap();

        let rect = doc
            .descendants()
            .find(|node| node.tag_name() == Some(EId::Rect))
            .expect("rect should be present");
        let animate = rect
            .children()
            .find(|child| child.tag_name() == Some(EId::Animate))
            .expect("animate child should be retained");
        assert_eq!(animate.attribute::<&str>(AId::AttributeName), Some("opacity"));
    }
}
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p usvg animate_child_is_retained_in_svgtree`
Expected: PASS. (If `attribute::<&str>` does not resolve, the codegen step did not run — re-run Step 4.)

- [ ] **Step 7: Commit**

```bash
git add crates/usvg/codegen/elements.txt crates/usvg/codegen/attributes.txt \
        crates/usvg/codegen/main.rs crates/usvg/src/parser/svgtree/names.rs \
        crates/usvg/src/parser/svgtree/mod.rs
git commit -m "feat(usvg): retain SMIL animation elements/attributes in svgtree"
```

---

### Task 2: Plumb `animation_time` and the override map

**Files:**
- Modify: `crates/usvg/src/parser/options.rs:101` (struct) and `:121` (Default)
- Modify: `crates/usvg/src/parser/svgtree/mod.rs` (`Document`, `NodeId`, `SvgNode::id`, `attribute`, `has_attribute`, `insert_animation_override`)
- Modify: `crates/usvg/src/parser/svgtree/parse.rs:69` (Document init)
- Test: inline in `crates/usvg/src/parser/svgtree/mod.rs`

- [ ] **Step 1: Add the `Options` field.** In `crates/usvg/src/parser/options.rs`, add to the struct after `style_sheet` (line ~100):

```rust
    /// Freezes SMIL animations at this instant (seconds) during parsing.
    ///
    /// `None` (default) parses the SVG statically, exactly as if no animation
    /// support existed. `Some(time)` bakes every animation's value at `time`
    /// into the produced `Tree`.
    pub animation_time: Option<f64>,
```

and in `impl Default` (after `style_sheet: None,`):

```rust
            animation_time: None,
```

- [ ] **Step 2: Make `NodeId` hashable and expose `SvgNode::id`.** In `crates/usvg/src/parser/svgtree/mod.rs`, change the `NodeId` derive (line ~152) from:

```rust
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct NodeId(NonZeroU32);
```
to
```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct NodeId(NonZeroU32);
```

and change `SvgNode::id` (line ~240) from `fn id(&self)` to `pub(crate) fn id(&self)`:

```rust
    #[inline]
    pub(crate) fn id(&self) -> NodeId {
        self.id
    }
```

- [ ] **Step 3: Add the override map to `Document`.** In `crates/usvg/src/parser/svgtree/mod.rs`, extend the struct (line ~24):

```rust
pub struct Document<'input> {
    nodes: Vec<NodeData>,
    attrs: Vec<Attribute<'input>>,
    links: HashMap<String, NodeId>,
    /// Per-frame animated attribute values, keyed by (node, attribute).
    /// Populated by the animation bake pass; consulted by `SvgNode::attribute`.
    animation_overrides: HashMap<(NodeId, AId), String>,
}
```

Add a setter method inside `impl<'input> Document<'input>` (near `append`):

```rust
    pub(crate) fn insert_animation_override(&mut self, node: NodeId, name: AId, value: String) {
        self.animation_overrides.insert((node, name), value);
    }
```

- [ ] **Step 4: Initialize the field** in `crates/usvg/src/parser/svgtree/parse.rs:69`:

```rust
    let mut doc = Document {
        nodes: Vec::new(),
        attrs: Vec::new(),
        links: HashMap::new(),
        animation_overrides: HashMap::new(),
    };
```

- [ ] **Step 5: Consult the override map in the choke point.** In `crates/usvg/src/parser/svgtree/mod.rs`, add a private helper and use it from `attribute` and `has_attribute`. Add inside `impl<'a, 'input: 'a> SvgNode<'a, 'input>`:

```rust
    #[inline]
    fn animated_attribute_value(&self, aid: AId) -> Option<&'a str> {
        if self.doc.animation_overrides.is_empty() {
            return None;
        }
        self.doc
            .animation_overrides
            .get(&(self.id, aid))
            .map(|value| value.as_str())
    }
```

Change `attribute` (line ~279) to consult the override first:

```rust
    pub fn attribute<T: FromValue<'a, 'input>>(&self, aid: AId) -> Option<T> {
        if let Some(animated) = self.animated_attribute_value(aid) {
            return T::parse(*self, aid, animated);
        }

        let value = self
            .attributes()
            .iter()
            .find(|a| a.name == aid)
            .map(|a| a.value.as_str())?;
        // ... existing body unchanged from here ...
```

Change `has_attribute` (line ~338) so animated attributes report as present:

```rust
    pub fn has_attribute(&self, aid: AId) -> bool {
        self.animated_attribute_value(aid).is_some()
            || self.attributes().iter().any(|a| a.name == aid)
    }
```

- [ ] **Step 6: Write a test** that an inserted override is read back through `attribute`. Add to `crates/usvg/src/parser/svgtree/mod.rs`'s test module:

```rust
    #[test]
    fn attribute_reads_animation_override() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">
            <rect width="10" height="10"/>
        </svg>"#;
        let xml = roxmltree::Document::parse(svg).unwrap();
        let mut doc = Document::parse_tree(&xml, None).unwrap();

        let rect_id = doc
            .descendants()
            .find(|node| node.tag_name() == Some(EId::Rect))
            .unwrap()
            .id();
        doc.insert_animation_override(rect_id, AId::Width, "4".to_string());

        let rect = doc
            .descendants()
            .find(|node| node.tag_name() == Some(EId::Rect))
            .unwrap();
        assert_eq!(rect.attribute::<svgtypes::Length>(AId::Width).map(|l| l.number), Some(4.0));
        assert!(rect.has_attribute(AId::Width));
    }
```

- [ ] **Step 7: Run the test**

Run: `cargo test -p usvg attribute_reads_animation_override`
Expected: PASS.

- [ ] **Step 8: Verify static behavior is unchanged**

Run: `cargo test -p usvg`
Expected: PASS (no existing tests regress; `animation_overrides` is empty unless baked).

- [ ] **Step 9: Commit**

```bash
git add crates/usvg/src/parser/options.rs crates/usvg/src/parser/svgtree/mod.rs \
        crates/usvg/src/parser/svgtree/parse.rs
git commit -m "feat(usvg): add animation_time option and per-Document override map"
```

---

## Phase 2 — animation model & interpolation (pure functions)

### Task 3: Module scaffold + clock values + `Timing`

**Files:**
- Create: `crates/usvg/src/parser/animation/mod.rs`
- Create: `crates/usvg/src/parser/animation/timing.rs`
- Modify: `crates/usvg/src/parser/mod.rs:14` (declare module)

- [ ] **Step 1: Declare the module.** In `crates/usvg/src/parser/mod.rs`, add to the `mod` list (after `mod converter;`):

```rust
mod animation;
```

- [ ] **Step 2: Create the module root** `crates/usvg/src/parser/animation/mod.rs`:

```rust
// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A pragmatic SMIL animation subset for sampling animated SVGs into frames.

mod timing;
mod interpolate;
mod element;
mod bake;
mod animated_svg;

pub use animated_svg::AnimatedSvg;
```

- [ ] **Step 3: Write failing tests** for clock parsing and phase sampling. Create `crates/usvg/src/parser/animation/timing.rs`:

```rust
// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clock_values() {
        assert_eq!(parse_clock_value("1s"), Some(1.0));
        assert_eq!(parse_clock_value("250ms"), Some(0.25));
        assert_eq!(parse_clock_value("1.5"), Some(1.5));
        assert_eq!(parse_clock_value("2min"), Some(120.0));
        assert_eq!(parse_clock_value("00:00:02"), Some(2.0));
        assert_eq!(parse_clock_value("garbage"), None);
    }

    #[test]
    fn samples_phases() {
        let timing = Timing {
            begin: 1.0,
            duration: Some(2.0),
            repeat: Repeat::Count(2.0),
            end: None,
            fill: AnimationFill::Freeze,
        };
        assert!(matches!(timing.phase_at(0.5), Phase::Before));
        match timing.phase_at(2.0) {
            Phase::Active { iteration, progress } => {
                assert_eq!(iteration, 0);
                assert!((progress - 0.5).abs() < 1e-9);
            }
            other => panic!("expected active, got {other:?}"),
        }
        match timing.phase_at(4.0) {
            Phase::Active { iteration, progress } => {
                assert_eq!(iteration, 1);
                assert!((progress - 0.5).abs() < 1e-9);
            }
            other => panic!("expected active, got {other:?}"),
        }
        assert!(matches!(timing.phase_at(99.0), Phase::After { frozen: true }));
        assert!((timing.timeline_end() - 5.0).abs() < 1e-9);
    }
}
```

- [ ] **Step 4: Run to verify failure**

Run: `cargo test -p usvg timing::tests`
Expected: FAIL (compile error — `Timing`, `parse_clock_value`, etc. undefined).

- [ ] **Step 5: Implement** at the top of `crates/usvg/src/parser/animation/timing.rs` (above the test module):

```rust
use crate::parser::svgtree::{AId, SvgNode};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Repeat {
    Count(f64),
    Indefinite,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum AnimationFill {
    Remove,
    Freeze,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Timing {
    pub begin: f64,
    /// Duration of a single iteration in seconds; `None` means indefinite.
    pub duration: Option<f64>,
    pub repeat: Repeat,
    pub end: Option<f64>,
    pub fill: AnimationFill,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Phase {
    Before,
    Active { iteration: u32, progress: f64 },
    After { frozen: bool },
}

impl Timing {
    pub fn parse(node: SvgNode) -> Timing {
        let begin = node
            .attribute::<&str>(AId::Begin)
            .and_then(parse_begin_value)
            .unwrap_or(0.0);
        let duration = node
            .attribute::<&str>(AId::Dur)
            .and_then(|text| parse_clock_value(text.trim()));
        let repeat = match node.attribute::<&str>(AId::RepeatCount).map(str::trim) {
            Some("indefinite") => Repeat::Indefinite,
            Some(text) => text.parse::<f64>().ok().map(Repeat::Count).unwrap_or(Repeat::Count(1.0)),
            None => Repeat::Count(1.0),
        };
        let end = node
            .attribute::<&str>(AId::End)
            .and_then(|text| parse_clock_value(text.trim()));
        let fill = match node.attribute::<&str>(AId::Fill).map(str::trim) {
            Some("freeze") => AnimationFill::Freeze,
            _ => AnimationFill::Remove,
        };
        Timing { begin, duration, repeat, end, fill }
    }

    /// Active duration in seconds; `None` means unbounded (indefinite repeat).
    pub fn active_duration(&self) -> Option<f64> {
        let single = self.duration?;
        let by_repeat = match self.repeat {
            Repeat::Count(count) => Some(single * count),
            Repeat::Indefinite => None,
        };
        match (by_repeat, self.end) {
            (Some(active), Some(end)) => Some(active.min((end - self.begin).max(0.0))),
            (Some(active), None) => Some(active),
            (None, Some(end)) => Some((end - self.begin).max(0.0)),
            (None, None) => None,
        }
    }

    /// Timeline length contribution: `begin + active` (or one iteration when
    /// the active duration is unbounded, so an indefinite loop exports once).
    pub fn timeline_end(&self) -> f64 {
        let single = self.duration.unwrap_or(0.0);
        let span = self.active_duration().unwrap_or(single);
        self.begin + span
    }

    pub fn phase_at(&self, time: f64) -> Phase {
        let single = match self.duration {
            Some(value) if value > 0.0 => value,
            _ => return if time >= self.begin { Phase::After { frozen: self.fill == AnimationFill::Freeze } } else { Phase::Before },
        };

        if time < self.begin {
            return Phase::Before;
        }
        let active_time = time - self.begin;

        if let Some(active) = self.active_duration() {
            if active_time >= active {
                return Phase::After { frozen: self.fill == AnimationFill::Freeze };
            }
        }

        let iteration = (active_time / single).floor();
        let progress = (active_time / single) - iteration;
        Phase::Active { iteration: iteration as u32, progress }
    }
}

/// SMIL begin: a single numeric offset (event/syncbase values are unsupported).
fn parse_begin_value(text: &str) -> Option<f64> {
    let first = text.split(';').next()?.trim();
    parse_clock_value(first)
}

/// Parses an SMIL clock value: `"1.5"`, `"250ms"`, `"1s"`, `"2min"`, `"1h"`,
/// or full/partial clock `"[[hh:]mm:]ss(.fff)"`.
pub(crate) fn parse_clock_value(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Some(rest) = text.strip_suffix("ms") {
        return rest.trim().parse::<f64>().ok().map(|value| value / 1000.0);
    }
    if let Some(rest) = text.strip_suffix("min") {
        return rest.trim().parse::<f64>().ok().map(|value| value * 60.0);
    }
    if let Some(rest) = text.strip_suffix('s') {
        return rest.trim().parse::<f64>().ok();
    }
    if let Some(rest) = text.strip_suffix('h') {
        return rest.trim().parse::<f64>().ok().map(|value| value * 3600.0);
    }
    if text.contains(':') {
        let mut seconds = 0.0;
        for part in text.split(':') {
            seconds = seconds * 60.0 + part.trim().parse::<f64>().ok()?;
        }
        return Some(seconds);
    }
    text.parse::<f64>().ok()
}
```

- [ ] **Step 6: Run to verify pass**

Run: `cargo test -p usvg timing::tests`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/usvg/src/parser/mod.rs crates/usvg/src/parser/animation/
git commit -m "feat(usvg): add animation module scaffold, clock parsing and timing phases"
```

---

### Task 4: Interpolation engine (`interpolate_components`)

**Files:**
- Create: `crates/usvg/src/parser/animation/interpolate.rs`

- [ ] **Step 1: Write failing tests.** Create `crates/usvg/src/parser/animation/interpolate.rs`:

```rust
// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#[cfg(test)]
mod tests {
    use super::*;

    fn scalars(values: &[f64]) -> Vec<Vec<f64>> {
        values.iter().map(|value| vec![*value]).collect()
    }

    #[test]
    fn linear_midpoint() {
        let values = scalars(&[0.0, 10.0]);
        let result = interpolate_components(&values, None, CalcMode::Linear, None, 0.5);
        assert!((result[0] - 5.0).abs() < 1e-9);
    }

    #[test]
    fn discrete_steps() {
        let values = scalars(&[0.0, 1.0, 2.0]);
        assert_eq!(interpolate_components(&values, None, CalcMode::Discrete, None, 0.0)[0], 0.0);
        assert_eq!(interpolate_components(&values, None, CalcMode::Discrete, None, 0.5)[0], 1.0);
        assert_eq!(interpolate_components(&values, None, CalcMode::Discrete, None, 0.99)[0], 2.0);
    }

    #[test]
    fn key_times_remap() {
        // Value reaches 10 only in the last 20% of the timeline.
        let values = scalars(&[0.0, 0.0, 10.0]);
        let key_times = vec![0.0, 0.8, 1.0];
        let result = interpolate_components(&values, Some(&key_times), CalcMode::Linear, None, 0.9);
        assert!((result[0] - 5.0).abs() < 1e-9);
    }

    #[test]
    fn paced_constant_velocity() {
        // Uneven value spacing; paced ignores key_times and spaces by distance.
        let values = scalars(&[0.0, 1.0, 10.0]);
        let result = interpolate_components(&values, None, CalcMode::Paced, None, 0.5);
        // Halfway by distance along [0..1..10] (total 10) is value 5.0.
        assert!((result[0] - 5.0).abs() < 1e-6);
    }

    #[test]
    fn spline_ease_is_monotonic_endpoints() {
        let values = scalars(&[0.0, 1.0]);
        let easing = vec![Easing::new(0.42, 0.0, 0.58, 1.0)];
        let start = interpolate_components(&values, None, CalcMode::Spline, Some(&easing), 0.0)[0];
        let end = interpolate_components(&values, None, CalcMode::Spline, Some(&easing), 1.0)[0];
        let mid = interpolate_components(&values, None, CalcMode::Spline, Some(&easing), 0.5)[0];
        assert!((start - 0.0).abs() < 1e-6);
        assert!((end - 1.0).abs() < 1e-6);
        assert!(mid > 0.0 && mid < 1.0);
    }

    #[test]
    fn multi_component_lerp() {
        let values = vec![vec![0.0, 100.0, 200.0], vec![10.0, 0.0, 0.0]];
        let result = interpolate_components(&values, None, CalcMode::Linear, None, 0.5);
        assert_eq!(result, vec![5.0, 50.0, 100.0]);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p usvg interpolate::tests`
Expected: FAIL (undefined `interpolate_components`, `CalcMode`, `Easing`).

- [ ] **Step 3: Implement** above the test module in `interpolate.rs`:

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum CalcMode {
    Discrete,
    Linear,
    Paced,
    Spline,
}

/// A keySplines easing segment: a cubic Bézier timing curve from (0,0) to (1,1)
/// with control points (x1,y1) and (x2,y2).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Easing {
    curve: kurbo::CubicBez,
}

impl Easing {
    pub fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Easing {
        Easing {
            curve: kurbo::CubicBez::new(
                kurbo::Point::new(0.0, 0.0),
                kurbo::Point::new(x1, y1),
                kurbo::Point::new(x2, y2),
                kurbo::Point::new(1.0, 1.0),
            ),
        }
    }

    /// Maps an input progress `x` in [0,1] to the eased output `y`.
    fn solve(&self, x: f64) -> f64 {
        use kurbo::ParamCurve;
        let x = x.clamp(0.0, 1.0);
        // Newton's method on bezier_x(parameter) = x, then read bezier_y.
        let mut parameter = x;
        for _ in 0..8 {
            let point = self.curve.eval(parameter);
            let error = point.x - x;
            if error.abs() < 1e-6 {
                break;
            }
            let derivative = self.curve.eval((parameter + 1e-4).min(1.0)).x
                - self.curve.eval((parameter - 1e-4).max(0.0)).x;
            if derivative.abs() < 1e-9 {
                break;
            }
            parameter = (parameter - error * (2e-4 / derivative)).clamp(0.0, 1.0);
        }
        self.curve.eval(parameter).y
    }
}

/// Interpolates a list of equal-length component vectors at `progress` in [0,1].
pub(crate) fn interpolate_components(
    values: &[Vec<f64>],
    key_times: Option<&[f64]>,
    calc_mode: CalcMode,
    key_splines: Option<&[Easing]>,
    progress: f64,
) -> Vec<f64> {
    if values.is_empty() {
        return Vec::new();
    }
    if values.len() == 1 {
        return values[0].clone();
    }
    let progress = progress.clamp(0.0, 1.0);
    let count = values.len();

    if calc_mode == CalcMode::Discrete {
        // Each value occupies an equal time slice unless key_times says otherwise.
        let index = match key_times {
            Some(times) if times.len() == count => {
                let mut chosen = 0;
                for (i, start) in times.iter().enumerate() {
                    if progress >= *start {
                        chosen = i;
                    }
                }
                chosen
            }
            _ => ((progress * count as f64).floor() as usize).min(count - 1),
        };
        return values[index.min(count - 1)].clone();
    }

    let times = resolve_key_times(values, key_times, calc_mode);
    let last = count - 1;

    // Locate the active segment: largest `segment` with times[segment] <= progress.
    let mut segment = 0;
    while segment < last && progress > times[segment + 1] {
        segment += 1;
    }
    let segment = segment.min(last - 1);

    let span = (times[segment + 1] - times[segment]).max(1e-12);
    let mut local = ((progress - times[segment]) / span).clamp(0.0, 1.0);

    if calc_mode == CalcMode::Spline {
        if let Some(splines) = key_splines {
            if let Some(easing) = splines.get(segment) {
                local = easing.solve(local);
            }
        }
    }

    lerp_vectors(&values[segment], &values[segment + 1], local)
}

fn resolve_key_times(values: &[Vec<f64>], key_times: Option<&[f64]>, calc_mode: CalcMode) -> Vec<f64> {
    if calc_mode == CalcMode::Paced {
        return paced_key_times(values);
    }
    match key_times {
        Some(times) if times.len() == values.len() => times.to_vec(),
        _ => even_key_times(values.len()),
    }
}

fn even_key_times(count: usize) -> Vec<f64> {
    if count <= 1 {
        return vec![0.0];
    }
    (0..count).map(|i| i as f64 / (count - 1) as f64).collect()
}

fn paced_key_times(values: &[Vec<f64>]) -> Vec<f64> {
    let mut distances = vec![0.0];
    let mut total = 0.0;
    for window in values.windows(2) {
        total += vector_distance(&window[0], &window[1]);
        distances.push(total);
    }
    if total <= 0.0 {
        return even_key_times(values.len());
    }
    distances.iter().map(|distance| distance / total).collect()
}

fn vector_distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y) * (x - y))
        .sum::<f64>()
        .sqrt()
}

fn lerp_vectors(a: &[f64], b: &[f64], local: f64) -> Vec<f64> {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| x + (y - x) * local)
        .collect()
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p usvg interpolate::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/usvg/src/parser/animation/interpolate.rs
git commit -m "feat(usvg): add component interpolation (discrete/linear/paced/spline)"
```

---

### Task 5: Parse one animation element into `ParsedAnimation`

**Files:**
- Create: `crates/usvg/src/parser/animation/element.rs`

- [ ] **Step 1: Write failing tests.** Create `crates/usvg/src/parser/animation/element.rs`:

```rust
// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::svgtree::{Document, EId};

    fn first_animation(svg: &str) -> ParsedAnimation {
        let xml = roxmltree::Document::parse(svg).unwrap();
        let doc = Document::parse_tree(&xml, None).unwrap();
        let node = doc
            .descendants()
            .find(|node| matches!(
                node.tag_name(),
                Some(EId::Animate) | Some(EId::AnimateTransform)
                    | Some(EId::AnimateMotion) | Some(EId::Set)
            ))
            .unwrap();
        ParsedAnimation::parse(node).unwrap()
    }

    #[test]
    fn parses_opacity_animate() {
        let animation = first_animation(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">
                <rect width="10" height="10">
                    <animate attributeName="opacity" values="1;0.3;1" dur="1s"/>
                </rect>
            </svg>"#,
        );
        match animation.target {
            AnimationTarget::Attribute(AId::Opacity) => {}
            other => panic!("unexpected target {other:?}"),
        }
        assert_eq!(animation.values, vec![vec![1.0], vec![0.3], vec![1.0]]);
    }

    #[test]
    fn parses_rotate_transform() {
        let animation = first_animation(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">
                <g>
                    <animateTransform attributeName="transform" type="rotate"
                        from="0 5 5" to="90 5 5" dur="1s"/>
                </g>
            </svg>"#,
        );
        assert!(matches!(animation.target, AnimationTarget::Transform(TransformType::Rotate)));
        assert_eq!(animation.values, vec![vec![0.0, 5.0, 5.0], vec![90.0, 5.0, 5.0]]);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p usvg element::tests`
Expected: FAIL (undefined types).

- [ ] **Step 3: Implement** above the test module in `element.rs`:

```rust
use std::str::FromStr;

use crate::parser::svgtree::{AId, EId, SvgNode};
use super::interpolate::{CalcMode, Easing};
use super::timing::Timing;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TransformType {
    Translate,
    Scale,
    Rotate,
    SkewX,
    SkewY,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AnimationTarget {
    /// A named presentation attribute (opacity, fill, width, ...).
    Attribute(AId),
    /// An `animateTransform` of the given type.
    Transform(TransformType),
    /// An `animateMotion` (path supplied separately).
    Motion,
}

/// How interpolated components are rendered back into an attribute string.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ValueFormat {
    /// Single scalar, formatted as a plain number ("18.5").
    Scalar,
    /// Three components clamped to 0..255, formatted as "#rrggbb".
    Color,
    /// A discrete string value taken verbatim (for `<set>` / non-numeric).
    DiscreteString,
}

pub(crate) struct ParsedAnimation {
    pub target: AnimationTarget,
    pub format: ValueFormat,
    pub timing: Timing,
    /// Numeric component vectors per keyframe (empty when `format` is DiscreteString).
    pub values: Vec<Vec<f64>>,
    /// Verbatim string keyframes (used only when `format` is DiscreteString).
    pub discrete_values: Vec<String>,
    pub key_times: Option<Vec<f64>>,
    pub calc_mode: CalcMode,
    pub key_splines: Option<Vec<Easing>>,
    pub additive: bool,
    pub accumulate: bool,
    /// Inline `path`/`mpath` for `animateMotion`, as raw path data.
    pub motion_path: Option<String>,
    pub motion_rotate: MotionRotate,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum MotionRotate {
    None,
    Auto,
    AutoReverse,
    Angle(f64),
}

impl ParsedAnimation {
    pub fn parse(node: SvgNode) -> Option<ParsedAnimation> {
        let tag = node.tag_name()?;

        let calc_mode = match node.attribute::<&str>(AId::CalcMode).map(str::trim) {
            Some("discrete") => CalcMode::Discrete,
            Some("paced") => CalcMode::Paced,
            Some("spline") => CalcMode::Spline,
            _ => CalcMode::Linear,
        };

        let (target, format) = match tag {
            EId::AnimateTransform => {
                let transform_type = match node.attribute::<&str>(AId::Type).map(str::trim) {
                    Some("translate") => TransformType::Translate,
                    Some("scale") => TransformType::Scale,
                    Some("rotate") => TransformType::Rotate,
                    Some("skewX") => TransformType::SkewX,
                    Some("skewY") => TransformType::SkewY,
                    _ => TransformType::Translate,
                };
                (AnimationTarget::Transform(transform_type), ValueFormat::Scalar)
            }
            EId::AnimateMotion => (AnimationTarget::Motion, ValueFormat::Scalar),
            EId::Animate | EId::Set => {
                let name = node.attribute::<&str>(AId::AttributeName)?;
                let aid = AId::from_str(name.trim())?;
                (AnimationTarget::Attribute(aid), value_format_for(aid))
            }
            _ => return None,
        };

        let calc_mode = if tag == EId::Set { CalcMode::Discrete } else { calc_mode };

        let raw_values = collect_value_strings(node);
        let (values, discrete_values) = match format {
            ValueFormat::DiscreteString => (Vec::new(), raw_values),
            _ => (raw_values.iter().map(|text| parse_components(text)).collect(), Vec::new()),
        };

        let key_times = node
            .attribute::<&str>(AId::KeyTimes)
            .map(|text| split_numbers(text, ';'));
        let key_splines = node
            .attribute::<&str>(AId::KeySplines)
            .map(parse_key_splines);
        let additive = node.attribute::<&str>(AId::Additive).map(str::trim) == Some("sum");
        let accumulate = node.attribute::<&str>(AId::Accumulate).map(str::trim) == Some("sum");

        let motion_path = if tag == EId::AnimateMotion {
            node.attribute::<&str>(AId::Path)
                .map(|text| text.to_string())
                .or_else(|| resolve_mpath(node))
        } else {
            None
        };
        let motion_rotate = match node.attribute::<&str>(AId::Rotate).map(str::trim) {
            Some("auto") => MotionRotate::Auto,
            Some("auto-reverse") => MotionRotate::AutoReverse,
            Some(text) => text.parse::<f64>().ok().map(MotionRotate::Angle).unwrap_or(MotionRotate::None),
            None => MotionRotate::None,
        };

        Some(ParsedAnimation {
            target,
            format,
            timing: Timing::parse(node),
            values,
            discrete_values,
            key_times,
            calc_mode,
            key_splines,
            additive,
            accumulate,
            motion_path,
            motion_rotate,
        })
    }
}

/// Picks how an attribute's interpolated value is formatted for re-parsing.
fn value_format_for(aid: AId) -> ValueFormat {
    match aid {
        AId::Fill | AId::Stroke | AId::StopColor | AId::FloodColor => ValueFormat::Color,
        AId::Visibility | AId::Display => ValueFormat::DiscreteString,
        _ => ValueFormat::Scalar,
    }
}

/// Gathers the keyframe value strings from `values`, or `from`/`to`/`by`.
fn collect_value_strings(node: SvgNode) -> Vec<String> {
    if let Some(values) = node.attribute::<&str>(AId::Values) {
        return values
            .split(';')
            .map(|part| part.trim().to_string())
            .filter(|part| !part.is_empty())
            .collect();
    }
    let from = node.attribute::<&str>(AId::From).map(|text| text.trim().to_string());
    let to = node.attribute::<&str>(AId::To).map(|text| text.trim().to_string());
    let by = node.attribute::<&str>(AId::By).map(|text| text.trim().to_string());
    match (from, to, by) {
        (Some(from), Some(to), _) => vec![from, to],
        (Some(from), None, Some(by)) => {
            let from_components = parse_components(&from);
            let by_components = parse_components(&by);
            let summed: Vec<f64> = from_components
                .iter()
                .zip(by_components.iter())
                .map(|(a, b)| a + b)
                .collect();
            vec![from, format_components(&summed)]
        }
        (None, Some(to), _) => vec![to],
        _ => Vec::new(),
    }
}

fn parse_components(text: &str) -> Vec<f64> {
    text.split([' ', ',', '\t', '\n'])
        .filter(|part| !part.is_empty())
        .filter_map(|part| svgtypes::Number::from_str(part).ok().map(|number| number.0))
        .collect()
}

fn format_components(components: &[f64]) -> String {
    components
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

fn split_numbers(text: &str, separator: char) -> Vec<f64> {
    text.split(separator)
        .filter_map(|part| part.trim().parse::<f64>().ok())
        .collect()
}

fn parse_key_splines(text: &str) -> Vec<Easing> {
    text.split(';')
        .filter(|segment| !segment.trim().is_empty())
        .filter_map(|segment| {
            let numbers = split_numbers(segment, ' ');
            if numbers.len() == 4 {
                Some(Easing::new(numbers[0], numbers[1], numbers[2], numbers[3]))
            } else {
                None
            }
        })
        .collect()
}

fn resolve_mpath(node: SvgNode) -> Option<String> {
    let mpath = node.children().find(|child| child.tag_name() == Some(EId::Mpath))?;
    let href = mpath.attribute::<&str>(AId::Href)?;
    let id = href.trim().strip_prefix('#').unwrap_or(href.trim());
    let target = mpath.document().element_by_id(id)?;
    target.attribute::<&str>(AId::D).map(|data| data.to_string())
}
```

> Note: `format_components` and `parse_components` are reused by Task 6 and Task 7; keep them in this file.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p usvg element::tests`
Expected: PASS.

> `resolve_mpath` resolves the `<mpath>` reference by stripping the leading `#` and calling `document().element_by_id` (both public on `SvgNode`/`Document`), then reads the target `<path>`'s `d`. This avoids the IRI-vs-FuncIRI ambiguity of the generic `attribute::<SvgNode>` lookup.

- [ ] **Step 5: Commit**

```bash
git add crates/usvg/src/parser/animation/element.rs
git commit -m "feat(usvg): parse SMIL animation elements into typed ParsedAnimation"
```

---

### Task 6: Evaluate an animation at a time into a `Contribution`

**Files:**
- Modify: `crates/usvg/src/parser/animation/element.rs`

- [ ] **Step 1: Write failing tests.** Append to the `tests` module in `element.rs`:

```rust
    #[test]
    fn evaluates_opacity_midpoint() {
        let animation = first_animation(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">
                <rect width="10" height="10">
                    <animate attributeName="opacity" from="1" to="0" dur="1s"/>
                </rect>
            </svg>"#,
        );
        match animation.evaluate(0.5) {
            Some(Contribution::Attribute { name, value }) => {
                assert_eq!(name, AId::Opacity);
                assert_eq!(value, "0.5");
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn evaluates_rotate_to_matrix() {
        let animation = first_animation(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">
                <g>
                    <animateTransform attributeName="transform" type="rotate"
                        from="0" to="90" dur="1s"/>
                </g>
            </svg>"#,
        );
        match animation.evaluate(0.5) {
            Some(Contribution::Transform(transform)) => {
                // rotate(45deg): sx = cos45 ≈ 0.7071.
                assert!((transform.sx - 0.70710677).abs() < 1e-4);
            }
            other => panic!("unexpected {other:?}"),
        }
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p usvg element::tests::evaluates`
Expected: FAIL (`evaluate`, `Contribution` undefined).

- [ ] **Step 3: Implement** in `element.rs`. Add the `Contribution` enum near the top (after `MotionRotate`):

```rust
use tiny_skia_path::Transform;

#[derive(Debug)]
pub(crate) enum Contribution {
    Attribute { name: AId, value: String },
    Transform(Transform),
}
```

Add to `impl ParsedAnimation` (use motion path lazily; full motion evaluation is Task 7):

```rust
    /// Evaluates the animation at `time`, returning its contribution, or `None`
    /// if the animation has no effect at that instant (before begin, or removed
    /// after its active end).
    pub fn evaluate(&self, time: f64) -> Option<Contribution> {
        use super::timing::Phase;
        match self.timing.phase_at(time) {
            Phase::Before => None,
            Phase::After { frozen: false } => None,
            Phase::After { frozen: true } => self.contribution_at(1.0, 0),
            Phase::Active { iteration, progress } => self.contribution_at(progress, iteration),
        }
    }

    fn contribution_at(&self, progress: f64, iteration: u32) -> Option<Contribution> {
        match &self.target {
            AnimationTarget::Attribute(name) if self.format == ValueFormat::DiscreteString => {
                let index = discrete_index(progress, self.values.len().max(self.discrete_values.len()), self.key_times.as_deref());
                let value = self.discrete_values.get(index)?.clone();
                Some(Contribution::Attribute { name: *name, value })
            }
            AnimationTarget::Attribute(name) => {
                let components = self.sample_components(progress, iteration)?;
                let value = match self.format {
                    ValueFormat::Color => format_color(&components),
                    _ => format_scalar(&components),
                };
                Some(Contribution::Attribute { name: *name, value })
            }
            AnimationTarget::Transform(transform_type) => {
                let components = self.sample_components(progress, iteration)?;
                Some(Contribution::Transform(build_transform(*transform_type, &components)))
            }
            AnimationTarget::Motion => {
                let transform = super::motion::evaluate_motion(self, progress)?;
                Some(Contribution::Transform(transform))
            }
        }
    }

    fn sample_components(&self, progress: f64, iteration: u32) -> Option<Vec<f64>> {
        use super::interpolate::interpolate_components;
        if self.values.is_empty() {
            return None;
        }
        let mut components = interpolate_components(
            &self.values,
            self.key_times.as_deref(),
            self.calc_mode,
            self.key_splines.as_deref(),
            progress,
        );
        if self.accumulate && iteration > 0 {
            let first = &self.values[0];
            let last = &self.values[self.values.len() - 1];
            for i in 0..components.len() {
                let delta = last.get(i).copied().unwrap_or(0.0) - first.get(i).copied().unwrap_or(0.0);
                components[i] += delta * iteration as f64;
            }
        }
        Some(components)
    }
```

Add the formatting and transform-building free functions at the bottom of `element.rs` (before the test module):

```rust
fn format_scalar(components: &[f64]) -> String {
    components.first().copied().unwrap_or(0.0).to_string()
}

fn format_color(components: &[f64]) -> String {
    let channel = |value: f64| value.round().clamp(0.0, 255.0) as u8;
    let red = channel(components.first().copied().unwrap_or(0.0));
    let green = channel(components.get(1).copied().unwrap_or(0.0));
    let blue = channel(components.get(2).copied().unwrap_or(0.0));
    format!("#{red:02x}{green:02x}{blue:02x}")
}

fn discrete_index(progress: f64, count: usize, key_times: Option<&[f64]>) -> usize {
    if count == 0 {
        return 0;
    }
    let times: Vec<f64> = match key_times {
        Some(times) if times.len() == count => times.to_vec(),
        _ => (0..count).map(|i| i as f64 / count as f64).collect(),
    };
    let mut index = 0;
    for (i, start) in times.iter().enumerate() {
        if progress >= *start {
            index = i;
        }
    }
    index.min(count - 1)
}

/// Builds a 2D affine transform from an `animateTransform` type and its
/// interpolated numeric components.
fn build_transform(transform_type: TransformType, components: &[f64]) -> Transform {
    let at = |index: usize| components.get(index).copied().unwrap_or(0.0) as f32;
    match transform_type {
        TransformType::Translate => Transform::from_translate(at(0), at(1)),
        TransformType::Scale => {
            let scale_x = at(0);
            let scale_y = if components.len() >= 2 { at(1) } else { scale_x };
            Transform::from_scale(scale_x, scale_y)
        }
        TransformType::Rotate => {
            let angle = at(0);
            if components.len() >= 3 {
                Transform::from_rotate_at(angle, at(1), at(2))
            } else {
                Transform::from_rotate(angle)
            }
        }
        TransformType::SkewX => {
            let radians = (at(0)).to_radians();
            Transform::from_row(1.0, 0.0, radians.tan(), 1.0, 0.0, 0.0)
        }
        TransformType::SkewY => {
            let radians = (at(0)).to_radians();
            Transform::from_row(1.0, radians.tan(), 0.0, 1.0, 0.0, 0.0)
        }
    }
}
```

Update the module declarations in `animation/mod.rs` to add `mod motion;` (Task 7 fills it; add a temporary stub now so this task compiles):

```rust
mod motion;
```

Create a stub `crates/usvg/src/parser/animation/motion.rs`:

```rust
// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tiny_skia_path::Transform;
use super::element::ParsedAnimation;

pub(crate) fn evaluate_motion(_animation: &ParsedAnimation, _progress: f64) -> Option<Transform> {
    None
}
```

> Verify `Transform::from_rotate`, `from_rotate_at`, `from_scale`, `from_translate`, and field `.sx` exist on `tiny_skia_path::Transform` 0.12 (they do; `from_rotate_at(angle_degrees, cx, cy)`). If a constructor name differs, build via `from_row` using the matrix from the rotation about the center.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p usvg element::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/usvg/src/parser/animation/element.rs crates/usvg/src/parser/animation/mod.rs \
        crates/usvg/src/parser/animation/motion.rs
git commit -m "feat(usvg): evaluate animations into attribute/transform contributions"
```

---

### Task 7: Motion-path evaluation with kurbo

**Files:**
- Modify: `crates/usvg/src/parser/animation/motion.rs`

- [ ] **Step 1: Write failing test.** Replace `motion.rs` test-free stub with a test module plus real implementation. First add the test (append to `motion.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::svgtree::{Document, EId};
    use crate::parser::animation::element::ParsedAnimation;

    fn motion_animation(svg: &str) -> ParsedAnimation {
        let xml = roxmltree::Document::parse(svg).unwrap();
        let doc = Document::parse_tree(&xml, None).unwrap();
        let node = doc
            .descendants()
            .find(|node| node.tag_name() == Some(EId::AnimateMotion))
            .unwrap();
        ParsedAnimation::parse(node).unwrap()
    }

    #[test]
    fn moves_halfway_along_horizontal_path() {
        let animation = motion_animation(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <rect width="10" height="10">
                    <animateMotion path="M0,0 L100,0" dur="1s"/>
                </rect>
            </svg>"#,
        );
        let transform = evaluate_motion(&animation, 0.5).unwrap();
        assert!((transform.tx - 50.0).abs() < 1.0);
        assert!(transform.ty.abs() < 1e-3);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p usvg motion::tests`
Expected: FAIL (stub returns `None`, `.unwrap()` panics).

- [ ] **Step 3: Implement** `motion.rs` (replace the stub body):

```rust
// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use kurbo::{BezPath, ParamCurve, ParamCurveArclen, ParamCurveDeriv, PathSeg};
use tiny_skia_path::Transform;

use super::element::{MotionRotate, ParsedAnimation};

pub(crate) fn evaluate_motion(animation: &ParsedAnimation, progress: f64) -> Option<Transform> {
    let path_data = animation.motion_path.as_deref()?;
    let path = BezPath::from_svg(path_data).ok()?;
    let segments: Vec<PathSeg> = path.segments().collect();
    if segments.is_empty() {
        return None;
    }

    let lengths: Vec<f64> = segments.iter().map(|segment| segment.arclen(1e-3)).collect();
    let total: f64 = lengths.iter().sum();
    if total <= 0.0 {
        return None;
    }

    let target = progress.clamp(0.0, 1.0) * total;
    let mut walked = 0.0;
    let mut chosen = segments.len() - 1;
    let mut local = 1.0;
    for (index, length) in lengths.iter().enumerate() {
        if target <= walked + *length || index == segments.len() - 1 {
            local = if *length > 0.0 { (target - walked) / *length } else { 0.0 };
            chosen = index;
            break;
        }
        walked += *length;
    }

    let segment = segments[chosen];
    let point = segment.eval(local.clamp(0.0, 1.0));
    let mut transform = Transform::from_translate(point.x as f32, point.y as f32);

    let angle = match animation.motion_rotate {
        MotionRotate::None => None,
        MotionRotate::Angle(degrees) => Some(degrees as f32),
        MotionRotate::Auto | MotionRotate::AutoReverse => {
            let tangent = tangent_of(segment, local.clamp(0.0, 1.0));
            let mut degrees = tangent.atan2_degrees();
            if animation.motion_rotate == MotionRotate::AutoReverse {
                degrees += 180.0;
            }
            Some(degrees)
        }
    };
    if let Some(degrees) = angle {
        transform = transform.pre_concat(Transform::from_rotate(degrees));
    }
    Some(transform)
}

struct Tangent {
    x: f64,
    y: f64,
}

impl Tangent {
    fn atan2_degrees(&self) -> f32 {
        self.y.atan2(self.x).to_degrees() as f32
    }
}

fn tangent_of(segment: PathSeg, local: f64) -> Tangent {
    let derivative = match segment {
        PathSeg::Line(line) => line.deriv().eval(local),
        PathSeg::Quad(quad) => quad.deriv().eval(local),
        PathSeg::Cubic(cubic) => cubic.deriv().eval(local),
    };
    Tangent { x: derivative.x, y: derivative.y }
}
```

Also make `ParsedAnimation`, `MotionRotate` reachable: ensure `element.rs` declares them `pub(crate)` (they are) and that `animation/mod.rs` keeps `mod element;` before `mod motion;`. Make `element` module items used by `motion` visible by referencing `crate::parser::animation::element::...` (already done).

> Verify kurbo 0.13 exposes `BezPath::from_svg`, `PathSeg::{Line,Quad,Cubic}`, `arclen`, `deriv`, and `eval`. This mirrors `crates/usvg/src/text/layout.rs`, which already imports `ParamCurve, ParamCurveArclen, ParamCurveDeriv` — copy its segment-handling idioms if a name differs.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p usvg motion::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/usvg/src/parser/animation/motion.rs
git commit -m "feat(usvg): evaluate animateMotion along a path via kurbo arc-length"
```

---

## Phase 3 — bake pass & AnimatedSvg

### Task 8: Bake animations into the Document and wire into conversion

**Files:**
- Create: `crates/usvg/src/parser/animation/bake.rs`
- Modify: `crates/usvg/src/parser/animation/mod.rs` (export `apply_animations`, `timeline_duration`)
- Modify: `crates/usvg/src/parser/mod.rs:159-162` (`from_xmltree`)

- [ ] **Step 1: Write a failing integration test** at the bottom of `crates/usvg/src/parser/animation/bake.rs`:

```rust
// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#[cfg(test)]
mod tests {
    use crate::{Node, Options, Tree};

    fn group_transform_at(svg: &str, time: f64) -> tiny_skia_path::Transform {
        let mut options = Options::default();
        options.animation_time = Some(time);
        let tree = Tree::from_str(svg, &options).unwrap();
        for node in tree.root().children() {
            if let Node::Group(group) = node {
                return group.transform();
            }
        }
        panic!("expected a group child");
    }

    #[test]
    fn bakes_rotation_into_group_transform() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">
            <g>
                <rect x="0" y="0" width="4" height="4"/>
                <animateTransform attributeName="transform" type="rotate"
                    from="0" to="90" dur="1s"/>
            </g>
        </svg>"#;
        let transform = group_transform_at(svg, 0.5);
        // rotate(45deg): sx = cos45 ≈ 0.7071, ky = sin45 ≈ 0.7071.
        assert!((transform.sx - 0.70710677).abs() < 1e-3);
        assert!((transform.ky - 0.70710677).abs() < 1e-3);
    }

    #[test]
    fn no_animation_time_leaves_tree_static() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">
            <g>
                <rect width="4" height="4"/>
                <animateTransform attributeName="transform" type="rotate" from="0" to="90" dur="1s"/>
            </g>
        </svg>"#;
        let tree = Tree::from_str(svg, &Options::default()).unwrap();
        for node in tree.root().children() {
            if let Node::Group(group) = node {
                assert_eq!(group.transform(), tiny_skia_path::Transform::identity());
            }
        }
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p usvg bake::tests`
Expected: FAIL (`apply_animations` not wired; rotation not baked).

- [ ] **Step 3: Implement the bake pass** above the test module in `bake.rs`:

```rust
use tiny_skia_path::Transform;

use crate::parser::svgtree::{AId, Document, EId, NodeId, SvgNode};
use super::element::{Contribution, ParsedAnimation};

const ANIMATION_TAGS: [EId; 4] = [EId::Animate, EId::Set, EId::AnimateTransform, EId::AnimateMotion];

fn is_animation(node: SvgNode) -> bool {
    node.tag_name().map(|tag| ANIMATION_TAGS.contains(&tag)).unwrap_or(false)
}

/// Evaluates every animation at `time` and writes the resulting attribute and
/// transform overrides into the document's override map.
pub(crate) fn apply_animations(doc: &mut Document, time: f64) {
    let mut overrides: Vec<(NodeId, AId, String)> = Vec::new();

    for node in doc.descendants() {
        if !node.is_element() || is_animation(node) {
            continue;
        }

        let mut transform_contributions: Vec<Transform> = Vec::new();
        let mut replaces_base_transform = false;

        for child in node.children().filter(|child| is_animation(*child)) {
            let animation = match ParsedAnimation::parse(child) {
                Some(animation) => animation,
                None => continue,
            };
            match animation.evaluate(time) {
                Some(Contribution::Attribute { name, value }) => {
                    overrides.push((node.id(), name, value));
                }
                Some(Contribution::Transform(matrix)) => {
                    if !animation.additive {
                        replaces_base_transform = true;
                    }
                    transform_contributions.push(matrix);
                }
                None => {}
            }
        }

        if !transform_contributions.is_empty() {
            let base = if replaces_base_transform {
                Transform::identity()
            } else {
                node.attribute::<Transform>(AId::Transform).unwrap_or_default()
            };
            let mut result = base;
            for matrix in transform_contributions {
                result = result.pre_concat(matrix);
            }
            overrides.push((node.id(), AId::Transform, format_matrix(result)));
        }
    }

    for (node_id, name, value) in overrides {
        doc.insert_animation_override(node_id, name, value);
    }
}

/// Maximum timeline length across all animations, in seconds.
pub(crate) fn timeline_duration(doc: &Document) -> f64 {
    let mut maximum = 0.0_f64;
    for node in doc.descendants() {
        if is_animation(node) {
            if let Some(animation) = ParsedAnimation::parse(node) {
                maximum = maximum.max(animation.timing.timeline_end());
            }
        }
    }
    maximum
}

/// True if the document contains any supported animation element.
pub(crate) fn has_animation(doc: &Document) -> bool {
    doc.descendants().any(is_animation)
}

fn format_matrix(transform: Transform) -> String {
    // SVG matrix(a b c d e f) == from_row(sx, ky, kx, sy, tx, ty).
    format!(
        "matrix({} {} {} {} {} {})",
        transform.sx, transform.ky, transform.kx, transform.sy, transform.tx, transform.ty
    )
}
```

> Verify `tiny_skia_path::Transform` exposes public fields `sx, ky, kx, sy, tx, ty` (it does in 0.12) and `pre_concat`. The `timing` field of `ParsedAnimation` must be `pub` (it is).

- [ ] **Step 4: Export the bake entry points.** In `crates/usvg/src/parser/animation/mod.rs`, add:

```rust
pub(crate) use bake::{apply_animations, has_animation, timeline_duration};
```

- [ ] **Step 5: Wire the bake pass into conversion.** In `crates/usvg/src/parser/mod.rs`, change `from_xmltree` (lines ~159-162):

```rust
    pub fn from_xmltree(doc: &roxmltree::Document, opt: &Options) -> Result<Self, Error> {
        let mut doc = svgtree::Document::parse_tree(doc, opt.style_sheet.as_deref())?;
        if let Some(time) = opt.animation_time {
            self::animation::apply_animations(&mut doc, time);
        }
        self::converter::convert_doc(&doc, opt)
    }
```

> `Document::parse_tree` already returns an owned `Document`, so binding it `mut` is sufficient; no signature change is needed.

- [ ] **Step 6: Run to verify pass**

Run: `cargo test -p usvg bake::tests`
Expected: PASS (rotation baked at t=0.5; static tree unchanged with no `animation_time`).

- [ ] **Step 7: Run the full usvg suite for regressions**

Run: `cargo test -p usvg`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/usvg/src/parser/animation/bake.rs crates/usvg/src/parser/animation/mod.rs \
        crates/usvg/src/parser/mod.rs
git commit -m "feat(usvg): bake animations into the Document during conversion"
```

---

### Task 9: Public `AnimatedSvg`

**Files:**
- Create: `crates/usvg/src/parser/animation/animated_svg.rs`
- Modify: `crates/usvg/src/parser/mod.rs` (re-export)

- [ ] **Step 1: Write failing tests.** Create `crates/usvg/src/parser/animation/animated_svg.rs`:

```rust
// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Options;

    const SPINNER: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">
        <g>
            <rect width="4" height="4"/>
            <animateTransform attributeName="transform" type="rotate" from="0" to="360" dur="2s"/>
        </g>
    </svg>"#;

    #[test]
    fn reports_animation_and_duration() {
        let animation = AnimatedSvg::parse(SPINNER.as_bytes(), &Options::default()).unwrap();
        assert!(animation.is_animated());
        assert!((animation.duration() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn tree_at_differs_across_time() {
        let options = Options::default();
        let animation = AnimatedSvg::parse(SPINNER.as_bytes(), &options).unwrap();
        let start = animation.tree_at(0.0, &options).unwrap();
        let quarter = animation.tree_at(0.5, &options).unwrap();
        let start_transform = first_group_transform(&start);
        let quarter_transform = first_group_transform(&quarter);
        assert!(start_transform != quarter_transform);
    }

    #[test]
    fn static_svg_reports_not_animated() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="4" height="4"/></svg>"#;
        let animation = AnimatedSvg::parse(svg.as_bytes(), &Options::default()).unwrap();
        assert!(!animation.is_animated());
        assert_eq!(animation.duration(), 0.0);
    }

    fn first_group_transform(tree: &crate::Tree) -> tiny_skia_path::Transform {
        for node in tree.root().children() {
            if let crate::Node::Group(group) = node {
                return group.transform();
            }
        }
        tiny_skia_path::Transform::identity()
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p usvg animated_svg::tests`
Expected: FAIL (`AnimatedSvg` undefined).

- [ ] **Step 3: Implement** above the test module in `animated_svg.rs`:

```rust
use crate::parser::{Error, Options};
use crate::parser::svgtree::Document;
use crate::Tree;
use super::bake::{has_animation, timeline_duration};

/// A reusable handle over an animated SVG: parses the source and its SMIL
/// timing once, then produces a static [`Tree`] frozen at any instant.
pub struct AnimatedSvg {
    source: String,
    duration: f64,
    is_animated: bool,
}

impl AnimatedSvg {
    /// Parses an SVG (plain or gzip-compressed) and extracts its animation timeline.
    pub fn parse(data: &[u8], options: &Options) -> Result<AnimatedSvg, Error> {
        let source = if data.starts_with(&[0x1f, 0x8b]) {
            let decoded = crate::parser::decompress_svgz(data)?;
            String::from_utf8(decoded).map_err(|_| Error::NotAnUtf8Str)?
        } else {
            std::str::from_utf8(data).map_err(|_| Error::NotAnUtf8Str)?.to_string()
        };

        let xml_options = roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() };
        let xml = roxmltree::Document::parse_with_options(&source, xml_options)
            .map_err(Error::ParsingFailed)?;
        let document = Document::parse_tree(&xml, options.style_sheet.as_deref())?;

        let is_animated = has_animation(&document);
        let duration = if is_animated { timeline_duration(&document) } else { 0.0 };

        Ok(AnimatedSvg { source, duration, is_animated })
    }

    /// Whether the SVG contains any supported animation.
    pub fn is_animated(&self) -> bool {
        self.is_animated
    }

    /// Natural timeline length in seconds (one loop for indefinite repeats).
    pub fn duration(&self) -> f64 {
        self.duration
    }

    /// Produces the static tree corresponding to the animation frozen at `time`.
    pub fn tree_at(&self, time: f64, options: &Options) -> Result<Tree, Error> {
        Tree::from_str_at_time(&self.source, options, time)
    }
}
```

> **Why not clone `Options`?** It holds non-`Clone` resolver closures, so cloning is awkward and lossy. We avoid it entirely: `tree_at` calls a small internal `Tree::from_str_at_time` that reuses the caller's `&Options` and threads the time directly into the bake pass.

- [ ] **Step 3b: Add the internal time-aware entry point** on `Tree`, in `crates/usvg/src/parser/mod.rs`:

```rust
    /// Parses a `Tree` from a string, freezing animations at `time`.
    pub fn from_str_at_time(text: &str, opt: &Options, time: f64) -> Result<Self, Error> {
        let xml_opt = roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() };
        let doc = roxmltree::Document::parse_with_options(text, xml_opt).map_err(Error::ParsingFailed)?;
        let mut tree_doc = svgtree::Document::parse_tree(&doc, opt.style_sheet.as_deref())?;
        self::animation::apply_animations(&mut tree_doc, time);
        self::converter::convert_doc(&tree_doc, opt)
    }
```

`tree_at` (Step 3) already calls this entry point, so `animated_svg.rs` needs no further change. Note `from_str_at_time` duplicates the small `from_str` preamble (gzip is already handled by `AnimatedSvg::parse`, which stores decompressed text in `self.source`).

- [ ] **Step 4: Re-export `AnimatedSvg`.** In `crates/usvg/src/parser/mod.rs`, after `mod animation;`, add:

```rust
pub use animation::AnimatedSvg;
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p usvg animated_svg::tests`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/usvg/src/parser/animation/animated_svg.rs crates/usvg/src/parser/animation/mod.rs \
        crates/usvg/src/parser/mod.rs
git commit -m "feat(usvg): add public AnimatedSvg (parse, duration, tree_at)"
```

---

## Phase 4 — resvg frames & sprite sheet

### Task 10: `render_frames`

**Files:**
- Create: `crates/resvg/src/animation.rs`
- Modify: `crates/resvg/src/lib.rs`

- [ ] **Step 1: Declare and re-export the module.** In `crates/resvg/src/lib.rs`, after `mod render;` add:

```rust
mod animation;
```

and after `pub use usvg;` add:

```rust
pub use animation::{render_frames, render_sprite_sheet, pack_sprite_sheet, FrameOptions, SheetOptions, SpriteSheet, TimeSpan};
```

- [ ] **Step 2: Write failing tests.** Create `crates/resvg/src/animation.rs`:

```rust
// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#[cfg(test)]
mod tests {
    use super::*;

    const SPINNER: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20">
        <g>
            <rect x="8" y="2" width="4" height="4" fill="#ff0000"/>
            <animateTransform attributeName="transform" type="rotate" from="0 10 10" to="360 10 10" dur="1s"/>
        </g>
    </svg>"#;

    #[test]
    fn renders_requested_frame_count() {
        let options = usvg::Options::default();
        let animation = usvg::AnimatedSvg::parse(SPINNER.as_bytes(), &options).unwrap();
        let frames = render_frames(&animation, &options, &FrameOptions { frame_count: 12, ..Default::default() }).unwrap();
        assert_eq!(frames.len(), 12);
        assert_eq!(frames[0].width(), 20);
        assert_eq!(frames[0].height(), 20);
    }

    #[test]
    fn frames_differ_across_rotation() {
        let options = usvg::Options::default();
        let animation = usvg::AnimatedSvg::parse(SPINNER.as_bytes(), &options).unwrap();
        let frames = render_frames(&animation, &options, &FrameOptions { frame_count: 4, ..Default::default() }).unwrap();
        assert_ne!(frames[0].data(), frames[1].data());
    }

    #[test]
    fn zero_frames_is_empty() {
        let options = usvg::Options::default();
        let animation = usvg::AnimatedSvg::parse(SPINNER.as_bytes(), &options).unwrap();
        let frames = render_frames(&animation, &options, &FrameOptions { frame_count: 0, ..Default::default() }).unwrap();
        assert!(frames.is_empty());
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p resvg animation::tests::renders_requested_frame_count`
Expected: FAIL (undefined `render_frames`/`FrameOptions`).

- [ ] **Step 4: Implement** above the test module in `crates/resvg/src/animation.rs`:

```rust
use usvg::AnimatedSvg;

/// A sampling window over the animation timeline, in seconds.
#[derive(Clone, Copy, Debug)]
pub struct TimeSpan {
    pub start: f64,
    pub end: f64,
}

/// Controls how an animation is sampled into frames.
pub struct FrameOptions {
    /// Number of frames to render (e.g. 12 chunky, 60 smooth).
    pub frame_count: usize,
    /// Sampling window; `None` means `0..duration`.
    pub time_span: Option<TimeSpan>,
    /// Output size per frame; `None` means the tree's own size.
    pub size: Option<tiny_skia::IntSize>,
    /// `false` (default) omits the loop endpoint so frame N does not duplicate frame 0.
    pub endpoint_inclusive: bool,
    /// Root transform applied to every frame.
    pub transform: tiny_skia::Transform,
}

impl Default for FrameOptions {
    fn default() -> FrameOptions {
        FrameOptions {
            frame_count: 1,
            time_span: None,
            size: None,
            endpoint_inclusive: false,
            transform: tiny_skia::Transform::identity(),
        }
    }
}

/// Renders `frame_count` frames sampled across the animation timeline.
pub fn render_frames(
    animation: &AnimatedSvg,
    options: &usvg::Options,
    frame_options: &FrameOptions,
) -> Result<Vec<tiny_skia::Pixmap>, usvg::Error> {
    if frame_options.frame_count == 0 {
        return Ok(Vec::new());
    }

    let span = frame_options.time_span.unwrap_or(TimeSpan {
        start: 0.0,
        end: animation.duration(),
    });

    let times = sample_times(span, frame_options.frame_count, frame_options.endpoint_inclusive);
    let mut frames = Vec::with_capacity(times.len());

    for time in times {
        let tree = animation.tree_at(time, options)?;
        let size = frame_options
            .size
            .unwrap_or_else(|| tree.size().to_int_size());
        let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())
            .ok_or(usvg::Error::InvalidSize)?;

        let scale_x = size.width() as f32 / tree.size().width();
        let scale_y = size.height() as f32 / tree.size().height();
        let transform = frame_options.transform.pre_scale(scale_x, scale_y);

        crate::render(&tree, transform, &mut pixmap.as_mut());
        frames.push(pixmap);
    }

    Ok(frames)
}

fn sample_times(span: TimeSpan, frame_count: usize, endpoint_inclusive: bool) -> Vec<f64> {
    let length = span.end - span.start;
    if frame_count == 1 {
        return vec![span.start];
    }
    (0..frame_count)
        .map(|index| {
            let divisor = if endpoint_inclusive {
                (frame_count - 1) as f64
            } else {
                frame_count as f64
            };
            span.start + length * (index as f64) / divisor
        })
        .collect()
}
```

> Verify `tiny_skia::Pixmap::data()` and `IntSize::{width,height}` and `Size::to_int_size()` names; `crates/resvg/examples/minimal.rs` uses `tree.size().to_int_size()`.

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p resvg animation::tests`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/resvg/src/lib.rs crates/resvg/src/animation.rs
git commit -m "feat(resvg): render an animation into a frame sequence"
```

---

### Task 11: Sprite-sheet packing

**Files:**
- Modify: `crates/resvg/src/animation.rs`

- [ ] **Step 1: Write failing tests.** Append to the `tests` module in `crates/resvg/src/animation.rs`:

```rust
    #[test]
    fn packs_uniform_grid() {
        let mut frames = Vec::new();
        for _ in 0..7 {
            frames.push(tiny_skia::Pixmap::new(10, 8).unwrap());
        }
        let sheet = pack_sprite_sheet(&frames, &SheetOptions { columns: Some(3), padding: 0 }).unwrap();
        assert_eq!(sheet.columns, 3);
        assert_eq!(sheet.rows, 3);
        assert_eq!(sheet.frame_width, 10);
        assert_eq!(sheet.frame_height, 8);
        assert_eq!(sheet.pixmap.width(), 30);
        assert_eq!(sheet.pixmap.height(), 24);
        assert_eq!(sheet.frame_count, 7);
    }

    #[test]
    fn default_columns_are_near_square() {
        let frames: Vec<_> = (0..9).map(|_| tiny_skia::Pixmap::new(4, 4).unwrap()).collect();
        let sheet = pack_sprite_sheet(&frames, &SheetOptions { columns: None, padding: 0 }).unwrap();
        assert_eq!(sheet.columns, 3);
        assert_eq!(sheet.rows, 3);
    }

    #[test]
    fn empty_frames_yield_none() {
        assert!(pack_sprite_sheet(&[], &SheetOptions { columns: None, padding: 0 }).is_none());
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p resvg animation::tests::packs_uniform_grid`
Expected: FAIL (undefined `pack_sprite_sheet`/`SpriteSheet`/`SheetOptions`).

- [ ] **Step 3: Implement** in `crates/resvg/src/animation.rs` (add below `render_frames`):

```rust
/// A packed sprite sheet: a single pixmap arranged as a uniform grid.
pub struct SpriteSheet {
    pub pixmap: tiny_skia::Pixmap,
    pub columns: u32,
    pub rows: u32,
    pub frame_width: u32,
    pub frame_height: u32,
    pub frame_count: usize,
}

/// Controls sprite-sheet grid layout.
pub struct SheetOptions {
    /// Columns in the grid; `None` uses `ceil(sqrt(frame_count))`.
    pub columns: Option<u32>,
    /// Inter-cell spacing in pixels.
    pub padding: u32,
}

impl Default for SheetOptions {
    fn default() -> SheetOptions {
        SheetOptions { columns: None, padding: 0 }
    }
}

/// Packs uniform frames into a single grid pixmap. Returns `None` if `frames`
/// is empty. Frames are assumed equal-sized; the first frame's size is used.
pub fn pack_sprite_sheet(
    frames: &[tiny_skia::Pixmap],
    sheet_options: &SheetOptions,
) -> Option<SpriteSheet> {
    let first = frames.first()?;
    let frame_width = first.width();
    let frame_height = first.height();
    let frame_count = frames.len();

    let columns = sheet_options
        .columns
        .unwrap_or_else(|| (frame_count as f64).sqrt().ceil() as u32)
        .max(1);
    let rows = ((frame_count as u32) + columns - 1) / columns;

    let padding = sheet_options.padding;
    let sheet_width = columns * frame_width + padding * columns.saturating_sub(1);
    let sheet_height = rows * frame_height + padding * rows.saturating_sub(1);

    let mut pixmap = tiny_skia::Pixmap::new(sheet_width, sheet_height)?;
    let paint = tiny_skia::PixmapPaint::default();

    for (index, frame) in frames.iter().enumerate() {
        let column = (index as u32) % columns;
        let row = (index as u32) / columns;
        let x = (column * (frame_width + padding)) as i32;
        let y = (row * (frame_height + padding)) as i32;
        pixmap.draw_pixmap(
            x,
            y,
            frame.as_ref(),
            &paint,
            tiny_skia::Transform::identity(),
            None,
        );
    }

    Some(SpriteSheet {
        pixmap,
        columns,
        rows,
        frame_width,
        frame_height,
        frame_count,
    })
}

/// Renders an animation directly into a packed sprite sheet.
pub fn render_sprite_sheet(
    animation: &AnimatedSvg,
    options: &usvg::Options,
    frame_options: &FrameOptions,
    sheet_options: &SheetOptions,
) -> Result<SpriteSheet, usvg::Error> {
    let frames = render_frames(animation, options, frame_options)?;
    pack_sprite_sheet(&frames, sheet_options).ok_or(usvg::Error::InvalidSize)
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p resvg animation::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/resvg/src/animation.rs
git commit -m "feat(resvg): pack animation frames into a uniform-grid sprite sheet"
```

---

### Task 12: Worked example, fixture, and full regression

**Files:**
- Create: `crates/resvg/tests/fixtures/spinner.svg`
- Create: `crates/resvg/examples/animation.rs`

- [ ] **Step 1: Create the fixture** `crates/resvg/tests/fixtures/spinner.svg`:

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
  <rect x="28" y="8" width="8" height="20" rx="4" fill="#e74c3c">
    <animateTransform attributeName="transform" type="rotate"
        from="0 32 32" to="360 32 32" dur="1s" repeatCount="indefinite"/>
    <animate attributeName="opacity" values="1;0.4;1" dur="1s"
        calcMode="spline" keyTimes="0;0.5;1" keySplines=".4 0 .6 1; .4 0 .6 1"/>
  </rect>
</svg>
```

- [ ] **Step 2: Create the example** `crates/resvg/examples/animation.rs`:

```rust
// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

fn main() {
    let options = usvg::Options::default();
    let data = std::fs::read("crates/resvg/tests/fixtures/spinner.svg")
        .expect("read spinner.svg from the workspace root");

    let animation = usvg::AnimatedSvg::parse(&data, &options).expect("parse animated SVG");
    println!("animated: {}, duration: {}s", animation.is_animated(), animation.duration());

    let sheet = resvg::render_sprite_sheet(
        &animation,
        &options,
        &resvg::FrameOptions { frame_count: 12, ..Default::default() },
        &resvg::SheetOptions { columns: Some(4), padding: 0 },
    )
    .expect("render sprite sheet");

    println!(
        "sheet: {}x{} grid, {}x{} per frame",
        sheet.columns, sheet.rows, sheet.frame_width, sheet.frame_height
    );
    sheet.pixmap.save_png("spinner_sheet.png").expect("save PNG");
    println!("wrote spinner_sheet.png");
}
```

- [ ] **Step 3: Run the example** (smoke test)

Run: `cargo run -p resvg --example animation`
Expected: prints `animated: true, duration: 1s`, writes `spinner_sheet.png` (a 256×192 image: a 4×3 grid of 64×64 frames). Inspecting it shows a rotating, pulsing red bar.

- [ ] **Step 4: Add an end-to-end test** for the fixture. Append to the `tests` module in `crates/resvg/src/animation.rs`:

```rust
    #[test]
    fn renders_fixture_sprite_sheet() {
        let options = usvg::Options::default();
        let data = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/spinner.svg")).unwrap();
        let animation = usvg::AnimatedSvg::parse(&data, &options).unwrap();
        assert!(animation.is_animated());
        let sheet = render_sprite_sheet(
            &animation,
            &options,
            &FrameOptions { frame_count: 12, ..Default::default() },
            &SheetOptions { columns: Some(4), padding: 0 },
        )
        .unwrap();
        assert_eq!(sheet.pixmap.width(), 256);
        assert_eq!(sheet.pixmap.height(), 192);
        // The sheet is not entirely transparent.
        assert!(sheet.pixmap.data().iter().any(|byte| *byte != 0));
    }
```

- [ ] **Step 5: Run the new test**

Run: `cargo test -p resvg renders_fixture_sprite_sheet`
Expected: PASS. (256 = 4×64; 192 = 3×64.)

- [ ] **Step 6: Full regression across the workspace**

Run: `cargo test`
Expected: PASS (all existing usvg/resvg tests plus the new ones). If golden-image tests exist under `crates/resvg/tests`, they must be unaffected because no `animation_time` is set for them.

- [ ] **Step 7: Lint**

Run: `cargo clippy --workspace --all-targets`
Expected: no new warnings in the animation modules.

- [ ] **Step 8: Commit**

```bash
git add crates/resvg/tests/fixtures/spinner.svg crates/resvg/examples/animation.rs crates/resvg/src/animation.rs
git commit -m "feat(resvg): add animation example, fixture and end-to-end test"
```

---

## Self-review

**Spec coverage:**
- SMIL subset (`animate`/`animateTransform`/`animateMotion`/`set`/`mpath`, child-of-target) → Tasks 1, 5.
- Scope: transforms (Task 6), opacity & color (Tasks 5–6), discrete show/hide (`ValueFormat::DiscreteString`, Task 6), geometry (works via the choke point, Task 2/8), motion-path (Task 7).
- Full `calcMode` (discrete/linear/paced/spline) + keyTimes + keySplines → Task 4.
- Output: `Vec<Pixmap>` (Task 10) + uniform-grid sprite sheet (Task 11).
- kurbo for curve/spline math; no Bevy dep → Tasks 4, 7.
- Freeze-convert via `animation_time` + override map; `resolve_transform` untouched → Tasks 2, 8.
- `AnimatedSvg` API (parse/is_animated/duration/tree_at) → Task 9; resvg `FrameOptions`/`SpriteSheet` → Tasks 10–11.
- `animation_time = None` byte-for-byte unchanged → Task 2 Step 8, Task 8 Step 7, Task 12 Step 6.
- Library-only (no CLI) → no CLI tasks, by design.

**Deviations from spec (intentional, noted):**
- §8.2/§8.3: pre-bake into an override map instead of scanning children per attribute read (resolves the `&'a str` lifetime; same behavior). `resolve_transform` needs no change.
- §10.1: `frame_count == 0` returns `Ok(empty)` rather than an error (no new error type introduced; reuses `usvg::Error`).

**Placeholder scan:** No "TBD"/"add error handling"/"similar to" placeholders; every code step has real code. A few steps include explicit "verify this constructor/field name in 0.12/0.13" notes — these are confirmations against already-used APIs (mirrored from `text/layout.rs` and `examples/minimal.rs`), not missing logic.

**Type consistency:** `ParsedAnimation`/`Contribution`/`AnimationTarget`/`TransformType`/`ValueFormat`/`MotionRotate` are defined in Task 5/6 and used consistently in Tasks 7–8. `Timing`/`Phase`/`Repeat`/`AnimationFill` (Task 3) used in Tasks 5–8. `interpolate_components`/`CalcMode`/`Easing` (Task 4) used in Task 6. `AnimatedSvg`/`FrameOptions`/`TimeSpan`/`SpriteSheet`/`SheetOptions` consistent across Tasks 9–12. `apply_animations`/`timeline_duration`/`has_animation` defined Task 8, used Task 9.

**Risk notes for the executor:**
- Confirm `tiny_skia_path::Transform` constructor/field names (`from_rotate`, `from_rotate_at`, `from_scale`, `from_translate`, `pre_concat`, `pre_scale`, public `sx/ky/kx/sy/tx/ty`) in 0.12; build via `from_row` if any differ.
- Confirm kurbo 0.13 `BezPath::from_svg`, `PathSeg` variants, `arclen`, `deriv`, `eval` (mirrors `text/layout.rs`).
- Confirm `Group::transform()` getter (used in tests); other getters (`opacity()`) only if you add the optional opacity assertions.
- `Options` cloning is avoided entirely via `Tree::from_str_at_time` (Task 9 Step 3b) — do not attempt to `Clone` the resolver closures.
