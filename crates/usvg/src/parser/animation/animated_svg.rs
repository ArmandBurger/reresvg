// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::parser::{Error, Options};
use crate::parser::svgtree::Document;
use crate::Tree;
use super::bake::{has_animation, timeline_duration};

/// A reusable handle over an animated SVG: parses the source and its SMIL
/// timing once, then produces a static [`Tree`] frozen at any instant.
#[derive(Debug)]
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
