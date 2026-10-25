//! Gauss–Legendre quadrature nodes and weights.
//!
//! Used by distributions that have no closed form but a convergent
//! integral representation — currently the studentized-range (Tukey)
//! and (in M3.3b.2) multivariate-t (Dunnett) distributions.
//!
//! The 32-point rule integrates polynomials up to degree 63 exactly
//! and gives ~10 significant digits on smooth integrands over a
//! finite interval. Nodes/weights are the published values on
//! `[-1, 1]`; the caller does the affine map to its own interval.
//!
//! Reference (behaviour): Abramowitz & Stegun §25.4, Table 25.4;
//! Stroud & Secrest (1966), *Gaussian Quadrature Formulas*. Public
//! domain tabled values; no third-party code is used.

/// Legendre-Gauss nodes on `[-1, 1]`, n = 32. Reflected pairs are
/// listed as positive-half values; the caller mirrors.
#[allow(clippy::excessive_precision)]
pub const GL32_NODES_POS: [f64; 16] = [
    0.048_307_665_687_738_32,
    0.144_471_961_582_796_49,
    0.239_287_362_252_137_08,
    0.331_868_602_282_127_65,
    0.421_351_276_130_635_35,
    0.506_899_908_932_229_39,
    0.587_715_757_240_762_33,
    0.663_044_266_930_215_20,
    0.732_182_118_740_289_68,
    0.794_483_795_967_942_41,
    0.849_367_613_732_569_97,
    0.896_321_155_766_052_12,
    0.934_906_075_937_739_69,
    0.964_762_255_587_506_43,
    0.985_611_511_545_268_34,
    0.997_263_861_849_481_56,
];

/// Legendre-Gauss weights on `[-1, 1]`, n = 32, matching
/// [`GL32_NODES_POS`]. Weights are the same for ±node.
#[allow(clippy::excessive_precision)]
pub const GL32_WEIGHTS_POS: [f64; 16] = [
    0.096_540_088_514_727_80,
    0.095_638_720_079_274_86,
    0.093_844_399_080_804_57,
    0.091_173_878_695_763_88,
    0.087_652_093_004_403_80,
    0.083_311_924_226_946_75,
    0.078_193_895_787_070_31,
    0.072_345_794_108_848_51,
    0.065_822_222_776_361_85,
    0.058_684_093_478_535_55,
    0.050_998_059_262_376_17,
    0.042_835_898_022_226_68,
    0.034_273_862_913_021_43,
    0.025_392_065_309_262_06,
    0.016_274_394_730_905_67,
    0.007_018_610_009_470_09,
];

/// Integrate `f` over `[a, b]` using 32-point Gauss–Legendre.
pub fn gl32_integrate<F: FnMut(f64) -> f64>(a: f64, b: f64, mut f: F) -> f64 {
    let mid = 0.5 * (a + b);
    let half = 0.5 * (b - a);
    let mut sum = 0.0_f64;
    for i in 0..16 {
        let x = GL32_NODES_POS[i];
        let w = GL32_WEIGHTS_POS[i];
        sum += w * (f(mid + half * x) + f(mid - half * x));
    }
    half * sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrates_constant() {
        let i = gl32_integrate(0.0, 2.0, |_| 3.0);
        assert!((i - 6.0).abs() < 1e-14);
    }

    #[test]
    fn integrates_polynomial() {
        // ∫₀¹ x⁵ dx = 1/6, exact for a 32-point rule.
        let i = gl32_integrate(0.0, 1.0, |x| x.powi(5));
        assert!((i - 1.0 / 6.0).abs() < 1e-14);
    }

    #[test]
    fn integrates_smooth_function() {
        // ∫₀^π sin(x) dx = 2.
        let i = gl32_integrate(0.0, std::f64::consts::PI, f64::sin);
        assert!((i - 2.0).abs() < 1e-13);
    }
}
