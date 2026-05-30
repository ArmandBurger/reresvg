// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::str::FromStr;

use tiny_skia_path::Transform;
use crate::parser::svgtree::{AId, EId, SvgNode};
use super::interpolate::{CalcMode, Easing};
use super::timing::Timing;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TransformType {
    Translate,
    Scale,
    Rotate,
    SkewX,
    SkewY,
}

#[allow(dead_code)]
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
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ValueFormat {
    /// Single scalar, formatted as a plain number ("18.5").
    Scalar,
    /// Three components clamped to 0..255, formatted as "#rrggbb".
    Color,
    /// A discrete string value taken verbatim (for `<set>` / non-numeric).
    DiscreteString,
}

#[allow(dead_code)]
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

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum MotionRotate {
    None,
    Auto,
    AutoReverse,
    Angle(f64),
}

#[derive(Debug)]
pub(crate) enum Contribution {
    Attribute { name: AId, value: String },
    Transform(Transform),
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
                let count = self.values.len().max(self.discrete_values.len());
                let index = discrete_index(progress, count, self.key_times.as_deref());
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
}

fn value_format_for(aid: AId) -> ValueFormat {
    match aid {
        AId::Fill | AId::Stroke | AId::StopColor | AId::FloodColor => ValueFormat::Color,
        AId::Visibility | AId::Display => ValueFormat::DiscreteString,
        _ => ValueFormat::Scalar,
    }
}

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

pub(crate) fn parse_components(text: &str) -> Vec<f64> {
    text.split([' ', ',', '\t', '\n'])
        .filter(|part| !part.is_empty())
        .filter_map(|part| svgtypes::Number::from_str(part).ok().map(|number| number.0))
        .collect()
}

pub(crate) fn format_components(components: &[f64]) -> String {
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
            let radians = (at(0) as f64).to_radians() as f32;
            Transform::from_row(1.0, 0.0, radians.tan(), 1.0, 0.0, 0.0)
        }
        TransformType::SkewY => {
            let radians = (at(0) as f64).to_radians() as f32;
            Transform::from_row(1.0, radians.tan(), 0.0, 1.0, 0.0, 0.0)
        }
    }
}

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
}
