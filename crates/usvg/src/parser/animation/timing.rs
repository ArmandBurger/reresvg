// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

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
