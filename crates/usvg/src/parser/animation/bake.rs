// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

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
    format!(
        "matrix({} {} {} {} {} {})",
        transform.sx, transform.ky, transform.kx, transform.sy, transform.tx, transform.ty
    )
}

#[cfg(test)]
mod tests {
    use crate::{Node, Options, Tree};

    fn group_transform_at(svg: &str, time: f64) -> tiny_skia_path::Transform {
        let options = Options {
            animation_time: Some(time),
            ..Default::default()
        };
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
