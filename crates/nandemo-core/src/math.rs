//! ベクトル・クォータニオンの最小限の計算。

use crate::{Quat, Vec3};

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn scale(a: Vec3, s: f32) -> Vec3 {
    a.map(|v| v * s)
}

pub fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn length(a: Vec3) -> f32 {
    dot(a, a).sqrt()
}

/// 長さ 1 にします。長さ 0 のときは `None`
pub fn normalize3(a: Vec3) -> Option<Vec3> {
    let len = length(a);
    (len > 1e-8).then(|| scale(a, 1.0 / len))
}

/// 共役（単位クォータニオンなら逆回転）
pub fn conj(q: Quat) -> Quat {
    [-q[0], -q[1], -q[2], q[3]]
}

/// `axis`（長さ 1）まわりに `angle` ラジアン回す回転
pub fn axis_angle(axis: Vec3, angle: f32) -> Quat {
    let (s, c) = (angle / 2.0).sin_cos();
    [axis[0] * s, axis[1] * s, axis[2] * s, c]
}

/// オイラー角 XYZ（ラジアン、X を先に適用）にします
pub fn to_euler_xyz(q: Quat) -> Vec3 {
    let [x, y, z, w] = q;
    // 回転行列の要素
    let r20 = 2.0 * (x * z - y * w);
    let ry = (-r20).clamp(-1.0, 1.0).asin();
    if r20.abs() < 0.999_999 {
        let r21 = 2.0 * (y * z + x * w);
        let r22 = 1.0 - 2.0 * (x * x + y * y);
        let r10 = 2.0 * (x * y + z * w);
        let r00 = 1.0 - 2.0 * (y * y + z * z);
        [r21.atan2(r22), ry, r10.atan2(r00)]
    } else {
        // ジンバルロック: Z を 0 とみなします
        let r12 = 2.0 * (y * z - x * w);
        let r11 = 1.0 - 2.0 * (x * x + z * z);
        [(-r12).atan2(r11), ry, 0.0]
    }
}

/// オイラー角 XYZ（ラジアン、X を先に適用）から作ります
pub fn from_euler_xyz(e: Vec3) -> Quat {
    let rx = axis_angle([1.0, 0.0, 0.0], e[0]);
    let ry = axis_angle([0.0, 1.0, 0.0], e[1]);
    let rz = axis_angle([0.0, 0.0, 1.0], e[2]);
    quat_mul(rz, quat_mul(ry, rx))
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

    #[test]
    fn euler_roundtrip() {
        let e = [0.1, -0.4, 1.2];
        let q = from_euler_xyz(e);
        assert!(close(&to_euler_xyz(q), &e));
        assert!(close(
            &axis_angle([0.0, 0.0, 1.0], 1.2),
            &from_euler_xyz([0.0, 0.0, 1.2])
        ));
    }
}
