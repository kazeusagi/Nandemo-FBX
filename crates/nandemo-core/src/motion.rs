use crate::{Quat, Vec3};

/// ボーン名・モーフ名で対象を指定するモーション。
/// 値はすべてレストポーズからの相対値です。
#[derive(Debug, Clone, Default)]
pub struct Motion {
    pub name: String,
    pub fps: f32,
    pub bones: Vec<BoneTrack>,
    pub morphs: Vec<MorphTrack>,
}

#[derive(Debug, Clone, Default)]
pub struct BoneTrack {
    pub bone: String,
    /// フレーム順に並んでいます
    pub keys: Vec<BoneKey>,
}

// TODO: VMD のベジェ補間を保持する
#[derive(Debug, Clone, PartialEq)]
pub struct BoneKey {
    pub frame: u32,
    pub translation: Vec3,
    pub rotation: Quat,
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
