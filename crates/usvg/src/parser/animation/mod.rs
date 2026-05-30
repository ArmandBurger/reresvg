// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A pragmatic SMIL animation subset for sampling animated SVGs into frames.

mod timing;
mod interpolate;
mod element;
mod motion;
mod bake;
pub(crate) use bake::apply_animations;
#[allow(unused_imports)]
pub(crate) use bake::has_animation;
#[allow(unused_imports)]
pub(crate) use bake::timeline_duration;
