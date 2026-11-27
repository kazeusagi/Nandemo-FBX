//! FBX 用の行列（列優先 4x4、平行移動はセンチメートル）とオイラー角。

use nandemo_core::{Quat, Vec3};

use crate::CM_PER_METER;

pub type Mat4 = [f64; 16];

pub const IDENTITY: Mat4 = [
    1.0, 0.0, 0.0, 0.0, //
    0.0, 1.0, 0.0, 0.0, //
    0.0, 0.0, 1.0, 0.0, //
    0.0, 0.0, 0.0, 1.0,
];

/// 平行移動（メートル）と回転から行列を作ります。
pub fn from_rt(t: Vec3, q: Quat) -> Mat4 {
    let [x, y, z, w] = q.map(|v| v as f64);
    let [tx, ty, tz] = t.map(|v| v as f64 * CM_PER_METER);
    [
        1.0 - 2.0 * (y * y + z * z),
        2.0 * (x * y + z * w),
        2.0 * (x * z - y * w),
        0.0,
        2.0 * (x * y - z * w),
        1.0 - 2.0 * (x * x + z * z),
        2.0 * (y * z + x * w),
        0.0,
        2.0 * (x * z + y * w),
        2.0 * (y * z - x * w),
        1.0 - 2.0 * (x * x + y * y),
        0.0,
        tx,
        ty,
        tz,
        1.0,
    ]
}

/// 回転と平行移動だけの行列の逆行列
pub fn rigid_inverse(m: &Mat4) -> Mat4 {
    let mut inv = IDENTITY;
    for col in 0..3 {
        for row in 0..3 {
            inv[col * 4 + row] = m[row * 4 + col];
        }
        inv[12 + col] = -(0..3).map(|j| m[col * 4 + j] * m[12 + j]).sum::<f64>();
    }
    inv
}

/// クォータニオンを FBX の既定の回転順（XYZ、X を先に適用）のオイラー角（度）にします。
/// `prev` を渡すと、それに一番近い同じ回転の角度を選びます（アニメーションの飛び防止）。
pub fn euler_xyz(q: Quat, prev: Option<[f64; 3]>) -> [f64; 3] {
    let m = from_rt([0.0; 3], q);
    // 行列の要素 (行, 列)
    let r = |row: usize, col: usize| m[col * 4 + row];

    let y = (-r(2, 0)).clamp(-1.0, 1.0).asin();
    let (x, z) = if r(2, 0).abs() < 0.999_999 {
        (r(2, 1).atan2(r(2, 2)), r(1, 0).atan2(r(0, 0)))
    } else {
        // ジンバルロック: Z を 0 とみなします
        ((-r(1, 2)).atan2(r(1, 1)), 0.0)
    };
    let a = [x, y, z].map(f64::to_degrees);

    let Some(prev) = prev else {
        return a;
    };
    // 同じ回転を表すもう 1 つの解
    let b = [a[0] + 180.0, 180.0 - a[1], a[2] + 180.0];
    let near = |v: [f64; 3]| -> [f64; 3] {
        std::array::from_fn(|i| v[i] - ((v[i] - prev[i]) / 360.0).round() * 360.0)
    };
    let dist = |v: &[f64; 3]| (0..3).map(|i| (v[i] - prev[i]).abs()).sum::<f64>();
    let (a, b) = (near(a), near(b));
    if dist(&a) <= dist(&b) { a } else { b }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nandemo_core::math::quat_mul;

    fn axis_angle(axis: Vec3, deg: f32) -> Quat {
        let (s, c) = (deg.to_radians() / 2.0).sin_cos();
        [axis[0] * s, axis[1] * s, axis[2] * s, c]
    }

    #[test]
    fn euler_matches_xyz_order() {
        // X を先に適用 → q = Rz * Ry * Rx
        let q = quat_mul(
            axis_angle([0.0, 0.0, 1.0], 30.0),
            quat_mul(
                axis_angle([0.0, 1.0, 0.0], 20.0),
                axis_angle([1.0, 0.0, 0.0], 10.0),
            ),
        );
        let e = euler_xyz(q, None);
        for (v, want) in e.iter().zip([10.0, 20.0, 30.0]) {
            assert!((v - want).abs() < 1e-3, "{e:?}");
        }
    }

    #[test]
    fn euler_stays_continuous() {
        let e = euler_xyz(axis_angle([0.0, 0.0, 1.0], -170.0), Some([0.0, 0.0, 175.0]));
        assert!((e[2] - 190.0).abs() < 1e-3, "{e:?}");
    }

    #[test]
    fn inverse_undoes_transform() {
        let m = from_rt([1.0, 2.0, 3.0], axis_angle([0.0, 1.0, 0.0], 90.0));
        let inv = rigid_inverse(&m);
        // inv * (m の平行移動) = 原点
        let t = [m[12], m[13], m[14]];
        for row in 0..3 {
            let v: f64 = (0..3).map(|j| inv[j * 4 + row] * t[j]).sum::<f64>() + inv[12 + row];
            assert!(v.abs() < 1e-9);
        }
    }
}
