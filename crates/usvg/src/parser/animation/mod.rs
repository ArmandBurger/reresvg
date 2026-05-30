// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A pragmatic SMIL animation subset for sampling animated SVGs into frames.

mod timing;
mod interpolate;
mod element;
mod motion;
mod bake;
pub(crate) use bake::apply_animations;

mod animated_svg;
pub use animated_svg::AnimatedSvg;
