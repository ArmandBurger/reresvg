// Validates the benchmark generators: every generated SVG must parse and must
// produce a non-trivial tree. This guards against the "unreferenced defs" trap,
// where the converter silently drops content and a benchmark measures nothing.

#[path = "../benches/generators.rs"]
mod generators;

use generators::{
    css_heavy, flat_rects, gradient_heavy, nested_groups, path_segments, single_kind, ELEMENT_KINDS,
};

fn child_count(svg: &str) -> usize {
    let options = usvg::Options::default();
    let tree = usvg::Tree::from_str(svg, &options)
        .unwrap_or_else(|error| panic!("generated SVG failed to parse: {error:?}"));
    fn count(group: &usvg::Group) -> usize {
        group.children().iter().fold(0, |total, node| {
            total + 1 + if let usvg::Node::Group(inner) = node { count(inner) } else { 0 }
        })
    }
    count(tree.root())
}

#[test]
fn scaled_generators_produce_nontrivial_trees() {
    assert!(child_count(&flat_rects(50)) >= 50);
    assert!(child_count(&nested_groups(50)) >= 50);
    assert!(child_count(&gradient_heavy(50)) >= 50);
    assert!(child_count(&css_heavy(50)) >= 50);
    assert!(child_count(&path_segments(50)) >= 1);
}

#[test]
fn every_element_kind_is_reachable() {
    for kind in ELEMENT_KINDS {
        let svg = single_kind(kind, 20);
        let count = child_count(&svg);
        assert!(count >= 1, "kind `{kind}` produced an empty tree (likely dropped as unreferenced)");
    }
}
