// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tiny_skia_path::Transform;
use super::element::ParsedAnimation;

pub(crate) fn evaluate_motion(_animation: &ParsedAnimation, _progress: f64) -> Option<Transform> {
    None
}
