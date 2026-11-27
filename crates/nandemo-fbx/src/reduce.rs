//! 線形補間のカーブのキーを、誤差が許容値以下になる範囲で減らします。

/// 元のどのキーの時刻でも、線形補間した値との差が `tolerance` 以下になるようにキーを減らします。
///
/// 区間の始点から、全ての点を許容範囲内で通れる傾きの範囲を狭めながら進み、
/// 範囲がなくなったところで区間を終えます。計算量は O(n) です。
/// キーの値は元の値から最大 `tolerance` ずれることがあります。`tolerance` が 0 以下なら何もしません。
pub(crate) fn reduce(times: &[i64], values: &[f32], tolerance: f32) -> (Vec<i64>, Vec<f32>) {
    let n = values.len();
    if n <= 2 || tolerance <= 0.0 {
        return (times.to_vec(), values.to_vec());
    }
    let tol = tolerance as f64;
    let (mut out_t, mut out_v) = (vec![times[0]], vec![values[0]]);

    // 区間の始点と、通れる傾きの範囲
    let (mut t0, mut v0) = (times[0] as f64, values[0] as f64);
    let (mut lo, mut hi) = (f64::NEG_INFINITY, f64::INFINITY);
    let mut last = 0;
    let mut k = 1;
    while k < n {
        let dt = times[k] as f64 - t0;
        let v = values[k] as f64;
        let (new_lo, new_hi) = (lo.max((v - tol - v0) / dt), hi.min((v + tol - v0) / dt));
        if new_lo <= new_hi {
            (lo, hi, last) = (new_lo, new_hi, k);
            k += 1;
            continue;
        }
        // k は今の区間に入らないので、last で区間を終えて、そこから次の区間を始めます
        (t0, v0) = end_segment(times, values, last, t0, v0, lo, hi);
        out_t.push(times[last]);
        out_v.push(v0 as f32);
        (lo, hi) = (f64::NEG_INFINITY, f64::INFINITY);
    }
    let (_, v) = end_segment(times, values, n - 1, t0, v0, lo, hi);
    out_t.push(times[n - 1]);
    out_v.push(v as f32);
    (out_t, out_v)
}

/// 区間の終点の（時刻, 値）。傾きはなるべく元の値を通るものを選びます
fn end_segment(
    times: &[i64],
    values: &[f32],
    end: usize,
    t0: f64,
    v0: f64,
    lo: f64,
    hi: f64,
) -> (f64, f64) {
    let dt = times[end] as f64 - t0;
    let slope = ((values[end] as f64 - v0) / dt).clamp(lo, hi);
    (times[end] as f64, v0 + slope * dt)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 減らしたカーブを元の各時刻で線形補間したときの最大誤差
    fn max_error(times: &[i64], values: &[f32], (rt, rv): &(Vec<i64>, Vec<f32>)) -> f32 {
        times
            .iter()
            .zip(values)
            .map(|(&t, &v)| {
                let i = rt.partition_point(|&x| x <= t).clamp(1, rt.len() - 1);
                let s = (t - rt[i - 1]) as f32 / (rt[i] - rt[i - 1]) as f32;
                let y = rv[i - 1] + (rv[i] - rv[i - 1]) * s;
                (y - v).abs()
            })
            .fold(0.0, f32::max)
    }

    #[test]
    fn keeps_line_endpoints_only() {
        let times: Vec<i64> = (0..100).map(|t| t * 10).collect();
        let values: Vec<f32> = (0..100).map(|t| t as f32 * 0.5).collect();
        let r = reduce(&times, &values, 1e-3);
        assert_eq!(r.0, [0, 990]);
        assert!(max_error(&times, &values, &r) < 1e-3);
    }

    #[test]
    fn stays_within_tolerance() {
        let times: Vec<i64> = (0..600).collect();
        let values: Vec<f32> = (0..600)
            .map(|t| (t as f32 * 0.05).sin() * 30.0 + (t as f32 * 0.31).cos())
            .collect();
        for tol in [0.01, 0.1, 1.0] {
            let r = reduce(&times, &values, tol);
            assert!(r.0.len() < times.len());
            let e = max_error(&times, &values, &r);
            assert!(e <= tol * 1.001, "許容誤差 {tol} に対して {e}");
            assert_eq!((r.0[0], *r.0.last().unwrap()), (0, 599));
        }
    }

    #[test]
    fn keeps_steps() {
        // 急な段差は残る
        let times = [0, 1, 2, 3];
        let values = [0.0, 0.0, 10.0, 10.0];
        let r = reduce(&times, &values, 0.01);
        assert_eq!(r.0, [0, 1, 2, 3]);
    }

    #[test]
    fn zero_tolerance_keeps_everything() {
        let times = [0, 1, 2];
        let values = [0.0, 1.0, 2.0];
        assert_eq!(reduce(&times, &values, 0.0).0.len(), 3);
    }
}
