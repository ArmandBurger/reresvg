// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

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
        .map(|(first, second)| (first - second) * (first - second))
        .sum::<f64>()
        .sqrt()
}

fn lerp_vectors(a: &[f64], b: &[f64], local: f64) -> Vec<f64> {
    a.iter()
        .zip(b.iter())
        .map(|(first, second)| first + (second - first) * local)
        .collect()
}

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
