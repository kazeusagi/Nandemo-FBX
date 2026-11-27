//! VMD リーダー。
//!
//! [`parse`] でボーン・モーフのキーフレームを読み、[`to_motion`] で IR に変換します。
//! カメラ・照明・セルフ影・IK 表示のキーは今のところ読みません。

use std::collections::HashMap;

use nandemo_core::bin::{BinReader, ReadError, Result};
use nandemo_core::math::normalize;
use nandemo_core::{self as ir, Quat, Vec3};

/// MMD の 1 単位をメートルにした値（PMX リーダーと同じ）
pub const MMD_UNIT_METERS: f32 = 0.08;
pub const FPS: f32 = 30.0;

#[derive(Debug, Clone)]
pub struct Vmd {
    pub model_name: String,
    pub bone_keys: Vec<BoneKeyframe>,
    pub morph_keys: Vec<MorphKeyframe>,
    pub ik_keys: Vec<IkKeyframe>,
}

/// 表示・IK の有効/無効のキーフレーム
#[derive(Debug, Clone, PartialEq)]
pub struct IkKeyframe {
    pub frame: u32,
    pub visible: bool,
    /// （IK ボーン名, 有効か）
    pub states: Vec<(String, bool)>,
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

    // 古いファイルでは、ここから後ろのセクションがないことがあります
    let n = section(&mut r, 23)?;
    let mut morph_keys = Vec::with_capacity(n);
    for _ in 0..n {
        morph_keys.push(MorphKeyframe {
            morph: sjis(r.bytes(15)?),
            frame: r.u32()?,
            weight: r.f32()?,
        });
    }

    // カメラ・照明・セルフ影は読み飛ばします
    for size in [61, 28, 9] {
        let n = section(&mut r, size)?;
        r.skip(n * size)?;
    }

    let n = section(&mut r, 9)?;
    let mut ik_keys = Vec::with_capacity(n);
    for _ in 0..n {
        let frame = r.u32()?;
        let visible = r.u8()? != 0;
        let m = count(&mut r, 21)?;
        let mut states = Vec::with_capacity(m);
        for _ in 0..m {
            states.push((sjis(r.bytes(20)?), r.u8()? != 0));
        }
        ik_keys.push(IkKeyframe {
            frame,
            visible,
            states,
        });
    }

    Ok(Vmd {
        model_name,
        bone_keys,
        morph_keys,
        ik_keys,
    })
}

/// セクションの要素数。ファイルがここで終わっていれば 0
fn section(r: &mut BinReader, size: usize) -> Result<usize> {
    if r.remaining() == 0 {
        Ok(0)
    } else {
        count(r, size)
    }
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
            rotation: normalize([-qx, -qy, qz, qw]),
            interpolation: std::array::from_fn(|c| bezier(&k.interpolation, c)),
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

    let mut ik: Vec<ir::IkTrack> = Vec::new();
    for key in &vmd.ik_keys {
        for (bone, enabled) in &key.states {
            let k = ir::IkKey {
                frame: key.frame,
                enabled: *enabled,
            };
            match ik.iter_mut().find(|t| t.bone == *bone) {
                Some(t) => t.keys.push(k),
                None => ik.push(ir::IkTrack {
                    bone: bone.clone(),
                    keys: vec![k],
                }),
            }
        }
    }
    for t in &mut ik {
        t.keys.sort_by_key(|k| k.frame);
    }

    ir::Motion {
        name: vmd.model_name.clone(),
        fps: FPS,
        bones,
        morphs,
        ik,
    }
}

/// 補間パラメーターからベジェ曲線を取り出します。
/// `c` は 0〜2 が移動 X・Y・Z、3 が回転で、x1・y1・x2・y2 が 4 バイトおきに並んでいます（各 0〜127）。
fn bezier(data: &[u8; 64], c: usize) -> ir::Bezier {
    let v = |i: usize| data[c + i * 4].min(127) as f32 / 127.0;
    ir::Bezier {
        x1: v(0),
        y1: v(1),
        x2: v(2),
        y2: v(3),
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

        // カメラ 1 つ（読み飛ばし）、照明・セルフ影なし
        b.extend(1u32.to_le_bytes());
        b.extend([0u8; 61]);
        b.extend([0u8; 8]);

        // フレーム 20 で左足ＩＫを無効に
        b.extend(1u32.to_le_bytes());
        b.extend(20u32.to_le_bytes());
        b.push(1);
        b.extend(1u32.to_le_bytes());
        b.extend(fixed("左足ＩＫ", 20));
        b.push(0);
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
        assert_eq!(vmd.ik_keys[0].frame, 20);
        assert_eq!(vmd.ik_keys[0].states, [("左足ＩＫ".to_string(), false)]);

        let motion = to_motion(&vmd);
        assert!(motion.ik[0].enabled(19));
        assert!(!motion.ik[0].enabled(20));
    }

    #[test]
    fn parses_file_without_trailing_sections() {
        let data = sample_vmd();
        // モーフのセクションの直後で終わるファイル
        let vmd = parse(&data[..30 + 20 + 4 + 111 * 2 + 4 + 23]).unwrap();
        assert!(vmd.ik_keys.is_empty());
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
        // X・Y を反転し、長さ 1 に正規化
        assert_eq!(keys[0].rotation, normalize([-0.1, -0.2, 0.3, 0.9]));
        assert_eq!(keys[0].interpolation[3].x1, 20.0 / 127.0);
        assert_eq!(motion.morphs[0].keys[0].frame, 5);
    }
}
