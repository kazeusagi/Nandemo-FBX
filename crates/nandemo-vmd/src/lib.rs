//! VMD リーダー。
//!
//! [`parse`] でボーン・モーフのキーフレームを読み、[`to_motion`] で IR に変換します。
//! カメラ・照明・セルフ影・IK 表示のキーは今のところ読みません。

use std::collections::HashMap;

use nandemo_core::bin::{BinReader, ReadError, Result};
use nandemo_core::{self as ir, Quat, Vec3};

/// MMD の 1 単位をメートルにした値（PMX リーダーと同じ）
pub const MMD_UNIT_METERS: f32 = 0.08;
pub const FPS: f32 = 30.0;

#[derive(Debug, Clone)]
pub struct Vmd {
    pub model_name: String,
    pub bone_keys: Vec<BoneKeyframe>,
    pub morph_keys: Vec<MorphKeyframe>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoneKeyframe {
    pub bone: String,
    pub frame: u32,
    pub translation: Vec3,
    pub rotation: Quat,
    /// ベジェ補間パラメーター（生データ）
    pub interpolation: [u8; 64],
}

#[derive(Debug, Clone, PartialEq)]
pub struct MorphKeyframe {
    pub morph: String,
    pub frame: u32,
    pub weight: f32,
}

/// VMD ファイルを読みます。
pub fn parse(data: &[u8]) -> Result<Vmd> {
    let mut r = BinReader::new(data);
    let magic = r.bytes(30)?;
    let name_len = if magic.starts_with(b"Vocaloid Motion Data 0002") {
        20
    } else if magic.starts_with(b"Vocaloid Motion Data file") {
        10
    } else {
        return Err(ReadError {
            offset: 0,
            message: "VMD ファイルではありません".into(),
        });
    };
    let model_name = sjis(r.bytes(name_len)?);

    let n = count(&mut r, 111)?;
    let mut bone_keys = Vec::with_capacity(n);
    for _ in 0..n {
        bone_keys.push(BoneKeyframe {
            bone: sjis(r.bytes(15)?),
            frame: r.u32()?,
            translation: r.f32s()?,
            rotation: r.f32s()?,
            interpolation: r.bytes(64)?.try_into().unwrap(),
        });
    }

    // 古いファイルはモーフのセクション自体がないことがあります
    let n = if r.remaining() == 0 {
        0
    } else {
        count(&mut r, 23)?
    };
    let mut morph_keys = Vec::with_capacity(n);
    for _ in 0..n {
        morph_keys.push(MorphKeyframe {
            morph: sjis(r.bytes(15)?),
            frame: r.u32()?,
            weight: r.f32()?,
        });
    }

    Ok(Vmd {
        model_name,
        bone_keys,
        morph_keys,
    })
}

/// VMD を IR に変換します。
///
/// MMD は左手系なので Z を反転します（回転は X・Y 成分を反転）。
pub fn to_motion(vmd: &Vmd) -> ir::Motion {
    let mut bones: Vec<ir::BoneTrack> = Vec::new();
    let mut lookup = HashMap::new();
    for k in &vmd.bone_keys {
        let i = *lookup.entry(k.bone.as_str()).or_insert_with(|| {
            bones.push(ir::BoneTrack {
                bone: k.bone.clone(),
                keys: Vec::new(),
            });
            bones.len() - 1
        });
        let [x, y, z] = k.translation;
        let [qx, qy, qz, qw] = k.rotation;
        let s = MMD_UNIT_METERS;
        bones[i].keys.push(ir::BoneKey {
            frame: k.frame,
            translation: [x * s, y * s, -z * s],
            rotation: [-qx, -qy, qz, qw],
        });
    }

    let mut morphs: Vec<ir::MorphTrack> = Vec::new();
    let mut lookup = HashMap::new();
    for k in &vmd.morph_keys {
        let i = *lookup.entry(k.morph.as_str()).or_insert_with(|| {
            morphs.push(ir::MorphTrack {
                morph: k.morph.clone(),
                keys: Vec::new(),
            });
            morphs.len() - 1
        });
        morphs[i].keys.push(ir::MorphKey {
            frame: k.frame,
            weight: k.weight,
        });
    }

    for t in &mut bones {
        t.keys.sort_by_key(|k| k.frame);
    }
    for t in &mut morphs {
        t.keys.sort_by_key(|k| k.frame);
    }

    ir::Motion {
        name: vmd.model_name.clone(),
        fps: FPS,
        bones,
        morphs,
    }
}

fn count(r: &mut BinReader, size: usize) -> Result<usize> {
    let n = r.u32()? as usize;
    if n.saturating_mul(size) > r.remaining() {
        return Err(r.error(format!("キーフレーム数 {n} が不正です")));
    }
    Ok(n)
}

/// 固定長の Shift_JIS 文字列（NUL 以降は無視）
fn sjis(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let (text, _) = encoding_rs::SHIFT_JIS.decode_without_bom_handling(&bytes[..end]);
    text.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed(s: &str, len: usize) -> Vec<u8> {
        let (bytes, _, _) = encoding_rs::SHIFT_JIS.encode(s);
        let mut v = bytes.into_owned();
        v.resize(len, 0);
        v
    }

    fn sample_vmd() -> Vec<u8> {
        let mut b = fixed("Vocaloid Motion Data 0002", 30);
        b.extend(fixed("テストモデル", 20));

        b.extend(2u32.to_le_bytes());
        for frame in [10u32, 0] {
            b.extend(fixed("センター", 15));
            b.extend(frame.to_le_bytes());
            for v in [1.0f32, 2.0, 3.0, 0.1, 0.2, 0.3, 0.9] {
                b.extend(v.to_le_bytes());
            }
            b.extend([20u8; 64]);
        }

        b.extend(1u32.to_le_bytes());
        b.extend(fixed("あ", 15));
        b.extend(5u32.to_le_bytes());
        b.extend(0.5f32.to_le_bytes());
        b
    }

    #[test]
    fn parses_sample() {
        let vmd = parse(&sample_vmd()).unwrap();
        assert_eq!(vmd.model_name, "テストモデル");
        assert_eq!(vmd.bone_keys.len(), 2);
        assert_eq!(vmd.bone_keys[0].bone, "センター");
        assert_eq!(vmd.bone_keys[0].frame, 10);
        assert_eq!(vmd.morph_keys[0].morph, "あ");
        assert_eq!(vmd.morph_keys[0].weight, 0.5);
    }

    #[test]
    fn rejects_broken_file() {
        let data = sample_vmd();
        assert!(parse(&data[..100]).is_err());
        assert!(parse(&[0; 50]).is_err());
    }

    #[test]
    fn converts_to_right_handed_meters() {
        let motion = to_motion(&parse(&sample_vmd()).unwrap());
        assert_eq!(motion.bones.len(), 1);
        let keys = &motion.bones[0].keys;
        assert_eq!(keys[0].frame, 0);
        assert_eq!(keys[1].frame, 10);
        let s = MMD_UNIT_METERS;
        assert_eq!(keys[0].translation, [s, 2.0 * s, -3.0 * s]);
        assert_eq!(keys[0].rotation, [-0.1, -0.2, 0.3, 0.9]);
        assert_eq!(motion.morphs[0].keys[0].frame, 5);
    }
}
