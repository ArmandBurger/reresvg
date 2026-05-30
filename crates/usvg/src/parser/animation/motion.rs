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
