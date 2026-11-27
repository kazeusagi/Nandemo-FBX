use crate::math::{lerp, slerp};
use crate::{QUAT_IDENTITY, Quat, Vec3};

/// ボーン名・モーフ名で対象を指定するモーション。
/// 値はすべてレストポーズからの相対値です。
#[derive(Debug, Clone, Default)]
pub struct Motion {
    pub name: String,
    pub fps: f32,
    pub bones: Vec<BoneTrack>,
    pub morphs: Vec<MorphTrack>,
    /// IK の有効・無効の切り替え（MMD）
    pub ik: Vec<IkTrack>,
}

#[derive(Debug, Clone, Default)]
pub struct IkTrack {
    /// IK ボーンの名前
    pub bone: String,
    /// フレーム順に並んでいます
    pub keys: Vec<IkKey>,
}

impl IkTrack {
    /// 指定フレームで IK が有効か（最初のキーより前は有効）
    pub fn enabled(&self, frame: u32) -> bool {
        let i = self.keys.partition_point(|k| k.frame <= frame);
        i == 0 || self.keys[i - 1].enabled
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct IkKey {
    pub frame: u32,
    pub enabled: bool,
}

impl Motion {
    /// 最後のキーのフレーム番号
    pub fn last_frame(&self) -> u32 {
        let bones = self
            .bones
            .iter()
            .filter_map(|t| t.keys.last())
            .map(|k| k.frame);
        let morphs = self
            .morphs
            .iter()
            .filter_map(|t| t.keys.last())
            .map(|k| k.frame);
        bones.chain(morphs).max().unwrap_or(0)
    }
}

#[derive(Debug, Clone, Default)]
pub struct BoneTrack {
    pub bone: String,
    /// フレーム順に並んでいます
    pub keys: Vec<BoneKey>,
}

impl BoneTrack {
    /// 指定フレームの（移動, 回転）を補間して返します。
    /// 最初のキーより前は最初のキー、最後のキーより後は最後のキーの値です。
    pub fn sample(&self, frame: f32) -> (Vec3, Quat) {
        let keys = &self.keys;
        let i = keys.partition_point(|k| k.frame as f32 <= frame);
        let (a, b) = match (i.checked_sub(1).map(|i| &keys[i]), keys.get(i)) {
            (Some(a), Some(b)) => (a, b),
            (Some(k), None) | (None, Some(k)) => return (k.translation, k.rotation),
            (None, None) => return ([0.0; 3], QUAT_IDENTITY),
        };

        let t = (frame - a.frame as f32) / (b.frame - a.frame) as f32;
        let curve = &b.interpolation;
        let translation =
            std::array::from_fn(|c| lerp(a.translation[c], b.translation[c], curve[c].eval(t)));
        let rotation = slerp(a.rotation, b.rotation, curve[3].eval(t));
        (translation, rotation)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoneKey {
    pub frame: u32,
    pub translation: Vec3,
    pub rotation: Quat,
    /// 前のキーからこのキーまでの補間（移動 X, Y, Z, 回転）
    pub interpolation: [Bezier; 4],
}

/// (0, 0)・(x1, y1)・(x2, y2)・(1, 1) を制御点とする 3 次ベジェ曲線。
/// 横軸がキー間の経過割合、縦軸が値の変化割合です。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bezier {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl Bezier {
    pub const LINEAR: Self = Self {
        x1: 1.0 / 3.0,
        y1: 1.0 / 3.0,
        x2: 2.0 / 3.0,
        y2: 2.0 / 3.0,
    };

    /// 経過割合 `x`（0〜1）に対する値の変化割合を返します。
    pub fn eval(&self, x: f32) -> f32 {
        if x <= 0.0 || x >= 1.0 {
            return x.clamp(0.0, 1.0);
        }
        let curve = |t: f32, p1: f32, p2: f32| {
            let s = 1.0 - t;
            3.0 * s * s * t * p1 + 3.0 * s * t * t * p2 + t * t * t
        };
        // x は t について単調増加なので二分法で t を求めます
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        for _ in 0..24 {
            let mid = (lo + hi) / 2.0;
            if curve(mid, self.x1, self.x2) < x {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        curve((lo + hi) / 2.0, self.y1, self.y2)
    }
}

#[derive(Debug, Clone, Default)]
pub struct MorphTrack {
    pub morph: String,
    /// フレーム順に並んでいます
    pub keys: Vec<MorphKey>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MorphKey {
    pub frame: u32,
    pub weight: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bezier_eval() {
        assert!((Bezier::LINEAR.eval(0.3) - 0.3).abs() < 1e-4);
        let ease_in = Bezier {
            x1: 1.0,
            y1: 0.0,
            x2: 1.0,
            y2: 0.0,
        };
        assert!(ease_in.eval(0.5) < 0.2);
        assert_eq!(ease_in.eval(0.0), 0.0);
        assert!((ease_in.eval(1.0) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn sample_holds_and_interpolates() {
        let key = |frame, x| BoneKey {
            frame,
            translation: [x, 0.0, 0.0],
            rotation: QUAT_IDENTITY,
            interpolation: [Bezier::LINEAR; 4],
        };
        let track = BoneTrack {
            bone: "b".into(),
            keys: vec![key(10, 1.0), key(20, 3.0)],
        };
        assert_eq!(track.sample(0.0).0, [1.0, 0.0, 0.0]);
        assert_eq!(track.sample(30.0).0, [3.0, 0.0, 0.0]);
        assert!((track.sample(15.0).0[0] - 2.0).abs() < 1e-4);
    }
}
