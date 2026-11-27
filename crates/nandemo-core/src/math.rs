//! ベクトル・クォータニオンの最小限の計算。

use crate::{Quat, Vec3};

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// `a * b`（`b` の回転を先に適用）
pub fn quat_mul(a: Quat, b: Quat) -> Quat {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

pub fn normalize(q: Quat) -> Quat {
    let len = q.iter().map(|v| v * v).sum::<f32>().sqrt();
    if len == 0.0 {
        return crate::QUAT_IDENTITY;
    }
    q.map(|v| v / len)
}

/// ベクトルを回転します。
pub fn rotate(q: Quat, v: Vec3) -> Vec3 {
    let p = quat_mul(
        quat_mul(q, [v[0], v[1], v[2], 0.0]),
        [-q[0], -q[1], -q[2], q[3]],
    );
    [p[0], p[1], p[2]]
}

pub fn slerp(a: Quat, b: Quat, t: f32) -> Quat {
    let mut dot: f32 = (0..4).map(|i| a[i] * b[i]).sum();
    let mut b = b;
    if dot < 0.0 {
        b = b.map(|v| -v);
        dot = -dot;
    }
    if dot > 0.9995 {
        return normalize(std::array::from_fn(|i| lerp(a[i], b[i], t)));
    }
    let theta = dot.acos();
    let s = theta.sin();
    let (wa, wb) = (((1.0 - t) * theta).sin() / s, (t * theta).sin() / s);
    std::array::from_fn(|i| a[i] * wa + b[i] * wb)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: &[f32], b: &[f32]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5)
    }

    #[test]
    fn rotates_and_slerps() {
        let h = std::f32::consts::FRAC_1_SQRT_2;
        let z90 = [0.0, 0.0, h, h];
        assert!(close(&rotate(z90, [1.0, 0.0, 0.0]), &[0.0, 1.0, 0.0]));
        assert!(close(&quat_mul(z90, z90), &[0.0, 0.0, 1.0, 0.0]));

        let s = (std::f32::consts::PI / 8.0).sin();
        let c = (std::f32::consts::PI / 8.0).cos();
        assert!(close(
            &slerp(crate::QUAT_IDENTITY, z90, 0.5),
            &[0.0, 0.0, s, c]
        ));
    }
}
