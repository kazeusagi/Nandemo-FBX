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
    /// `joints` に対応するウェイト
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
}
