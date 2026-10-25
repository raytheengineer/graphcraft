//! Nice-number axis tick computation.
//!
//! The classical Heckbert (1990) algorithm — pick a tick step
//! from {1, 2, 5} × 10^k that gives ~`target` ticks across the
//! range, then expand the min/max out to the nearest tick.
//!
//! Reference (behaviour): Heckbert, P. S. (1990) "Nice numbers for
//! graph labels", in *Graphics Gems* (A. S. Glassner, ed.),
//! Academic Press, pp. 61–63. Classical algorithm; no third-party
//! or GraphPad material.

/// Result of nice-tick computation.
#[derive(Debug, Clone, PartialEq)]
pub struct NiceAxis {
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub ticks: Vec<f64>,
}

/// Round `v` to a "nice" number — 1, 2, 5, or 10 times a power of
/// ten. `round` picks the nearest; `!round` picks the ceiling (used
/// to pick the tick step).
fn nicenum(v: f64, round: bool) -> f64 {
    if v <= 0.0 {
        return 0.0;
    }
    let exp = v.log10().floor();
    let f = v / 10f64.powf(exp);
    let nf = if round {
        if f < 1.5 {
            1.0
        } else if f < 3.0 {
            2.0
        } else if f < 7.0 {
            5.0
        } else {
            10.0
        }
    } else if f <= 1.0 {
        1.0
    } else if f <= 2.0 {
        2.0
    } else if f <= 5.0 {
        5.0
    } else {
        10.0
    };
    nf * 10f64.powf(exp)
}

/// Compute a nice linear axis `[min, max]` that covers the data
/// range `[lo, hi]` with roughly `target` ticks.
pub fn nice_linear(lo: f64, hi: f64, target: usize) -> NiceAxis {
    assert!(target >= 2, "target must be >= 2");
    let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
    if !(lo.is_finite() && hi.is_finite()) {
        return NiceAxis {
            min: 0.0,
            max: 1.0,
            step: 0.5,
            ticks: vec![0.0, 0.5, 1.0],
        };
    }
    if (hi - lo).abs() < f64::EPSILON * (1.0 + hi.abs().max(lo.abs())) {
        // Degenerate zero-range input: centre a unit window.
        let c = 0.5 * (lo + hi);
        return nice_linear(c - 0.5, c + 0.5, target);
    }
    let range = nicenum(hi - lo, false);
    let step = nicenum(range / (target as f64 - 1.0), true);
    let min = (lo / step).floor() * step;
    let max = (hi / step).ceil() * step;
    let mut ticks = Vec::new();
    // Pre-size the vector; iterate on an integer counter so float
    // accumulation does not drop the last tick.
    let n = ((max - min) / step).round() as i64 + 1;
    for i in 0..n {
        ticks.push(min + step * i as f64);
    }
    NiceAxis {
        min,
        max,
        step,
        ticks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers_classic_textbook_range() {
        // Heckbert's own worked example: [-0.5, 8.5], target 5.
        // Expected: step 2, ticks 0, 2, 4, 6, 8, min=-2, max=10.
        // (Different convention: Heckbert uses the "loose" variant
        // which expands outside the data; "tight" shrinks inside.
        // We implement the loose variant.)
        let a = nice_linear(-0.5, 8.5, 5);
        assert_eq!(a.step, 2.0);
        assert_eq!(a.ticks, vec![-2.0, 0.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
        assert_eq!(a.min, -2.0);
        assert_eq!(a.max, 10.0);
    }

    #[test]
    fn covers_power_of_ten_range() {
        let a = nice_linear(0.0, 100.0, 6);
        assert!(a.ticks.contains(&0.0));
        assert!(a.ticks.contains(&100.0));
        for w in a.ticks.windows(2) {
            assert!((w[1] - w[0] - a.step).abs() < 1e-13);
        }
    }

    #[test]
    fn degenerate_range_still_has_ticks() {
        let a = nice_linear(5.0, 5.0, 5);
        assert!(a.ticks.len() >= 2);
        assert!(a.min < a.max);
    }

    #[test]
    fn reversed_input_is_normalised() {
        let a = nice_linear(10.0, -0.5, 5);
        let b = nice_linear(-0.5, 10.0, 5);
        assert_eq!(a, b);
    }
}
