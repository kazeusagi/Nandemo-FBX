use crate::{Quat, Vec2, Vec3, Vec4};

#[derive(Debug, Clone, Default)]
pub struct Model {
    pub name: String,
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
    pub textures: Vec<Texture>,
    pub bones: Vec<Bone>,
}

#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub name: String,
    pub positions: Vec<Vec3>,
    /// 空の場合は法線なし
    pub normals: Vec<Vec3>,
    /// 空の場合は UV なし
    pub uvs: Vec<Vec2>,
    /// 頂点ごとのボーン番号（最大 4 本）。空の場合はスキンなし
    pub joints: Vec<[u32; 4]>,
    /// `joints` に対応するウェイト（合計 1）
    pub weights: Vec<[f32; 4]>,
    /// 三角形リスト
    pub indices: Vec<u32>,
    pub parts: Vec<MeshPart>,
    pub morphs: Vec<Morph>,
}

/// `indices` のうち 1 つのマテリアルを使う範囲
#[derive(Debug, Clone, PartialEq)]
pub struct MeshPart {
    pub material: usize,
    pub start: usize,
    pub count: usize,
}

#[derive(Debug, Clone, Default)]
pub struct Morph {
    pub name: String,
    pub offsets: Vec<MorphOffset>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MorphOffset {
    pub vertex: u32,
    pub position: Vec3,
}

#[derive(Debug, Clone)]
pub struct Material {
    pub name: String,
    pub base_color: Vec4,
    pub base_color_texture: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct Texture {
    /// モデルファイルからの相対パス（区切りは `/`）
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct Bone {
    pub name: String,
    pub parent: Option<usize>,
    /// 親ボーンからの相対位置
    pub translation: Vec3,
    /// 親ボーンからの相対回転
    pub rotation: Quat,
    /// 変形階層。小さいものから順に計算します（MMD 以外は 0）
    pub layer: i32,
    /// 付与親（MMD）
    pub inherit: Option<Inherit>,
    /// IK（MMD）
    pub ik: Option<Ik>,
}

impl Bone {
    /// IK・付与親のない、変形階層 0 のボーン
    pub fn new(name: &str, parent: Option<usize>, translation: Vec3, rotation: Quat) -> Self {
        Self {
            name: name.into(),
            parent,
            translation,
            rotation,
            layer: 0,
            inherit: None,
            ik: None,
        }
    }
}

/// ボーンごとの親の番号。範囲外や循環している親は `None` にします
pub fn safe_parents(bones: &[Bone]) -> Vec<Option<usize>> {
    let parent = |i: usize| bones[i].parent.filter(|&p| p < bones.len());
    (0..bones.len())
        .map(|i| {
            let mut cur = parent(i);
            for _ in 0..bones.len() {
                match cur {
                    Some(c) if c == i => return None,
                    Some(c) => cur = parent(c),
                    None => break,
                }
            }
            parent(i)
        })
        .collect()
}

/// 付与親のアニメーションを `weight` の割合で自分にも加えます
#[derive(Debug, Clone, PartialEq)]
pub struct Inherit {
    pub bone: usize,
    pub weight: f32,
    pub rotation: bool,
    pub translation: bool,
}

/// このボーンの位置に `target` ボーンが来るよう、`links` のボーンを回します
#[derive(Debug, Clone, PartialEq)]
pub struct Ik {
    pub target: usize,
    pub iterations: u32,
    /// 1 回の計算で回せる最大角度（ラジアン）
    pub limit_angle: f32,
    /// `target` に近いものから順
    pub links: Vec<IkLink>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IkLink {
    pub bone: usize,
    /// 回転の制限。オイラー角 XYZ（ラジアン）の（下限, 上限）
    pub limits: Option<(Vec3, Vec3)>,
}
