//! PMX 2.0 / 2.1 リーダー。
//!
//! [`parse`] でファイルをそのままの構造で読み、[`to_model`] で IR に変換します。
//! 剛体・ジョイント・表示枠・ソフトボディは今のところ読みません。

mod convert;
mod parse;
#[cfg(test)]
mod tests;

pub use convert::{MMD_UNIT_METERS, to_model};
pub use parse::parse;

use nandemo_core::{Vec2, Vec3, Vec4};

#[derive(Debug, Clone)]
pub struct Pmx {
    pub header: Header,
    pub name: String,
    pub name_en: String,
    pub comment: String,
    pub comment_en: String,
    pub vertices: Vec<Vertex>,
    /// 三角形リスト（MMD の向きのまま）
    pub indices: Vec<u32>,
    pub textures: Vec<String>,
    pub materials: Vec<Material>,
    pub bones: Vec<Bone>,
    pub morphs: Vec<Morph>,
}

#[derive(Debug, Clone)]
pub struct Header {
    pub version: f32,
    pub utf8: bool,
    pub additional_uvs: u8,
    pub vertex_index_size: u8,
    pub texture_index_size: u8,
    pub material_index_size: u8,
    pub bone_index_size: u8,
    pub morph_index_size: u8,
    pub rigid_body_index_size: u8,
}

#[derive(Debug, Clone)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
    pub additional_uvs: Vec<Vec4>,
    pub weight: Weight,
    pub edge_scale: f32,
}

/// ボーン番号は `-1` で「なし」
#[derive(Debug, Clone, PartialEq)]
pub enum Weight {
    Bdef1(i32),
    Bdef2 {
        bones: [i32; 2],
        weight: f32,
    },
    Bdef4 {
        bones: [i32; 4],
        weights: [f32; 4],
    },
    Sdef {
        bones: [i32; 2],
        weight: f32,
        c: Vec3,
        r0: Vec3,
        r1: Vec3,
    },
    Qdef {
        bones: [i32; 4],
        weights: [f32; 4],
    },
}

#[derive(Debug, Clone)]
pub struct Material {
    pub name: String,
    pub name_en: String,
    pub diffuse: Vec4,
    pub specular: Vec3,
    pub specular_power: f32,
    pub ambient: Vec3,
    pub flags: u8,
    pub edge_color: Vec4,
    pub edge_size: f32,
    /// `-1` でなし
    pub texture: i32,
    /// `-1` でなし
    pub sphere_texture: i32,
    pub sphere_mode: u8,
    pub toon: Toon,
    pub memo: String,
    /// このマテリアルが使うインデックス数（三角形数 × 3）
    pub index_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Toon {
    /// 共有トゥーン（toon01.bmp〜toon10.bmp の番号 0〜9）
    Shared(u8),
    /// テクスチャ番号（`-1` でなし）
    Texture(i32),
}

#[derive(Debug, Clone)]
pub struct Bone {
    pub name: String,
    pub name_en: String,
    pub position: Vec3,
    /// `-1` でなし
    pub parent: i32,
    pub layer: i32,
    pub flags: u16,
    pub tail: BoneTail,
    /// 付与親（ボーン番号, 付与率）
    pub inherit: Option<(i32, f32)>,
    pub fixed_axis: Option<Vec3>,
    /// ローカル軸（X 軸, Z 軸）
    pub local_axes: Option<(Vec3, Vec3)>,
    pub external_parent: Option<i32>,
    pub ik: Option<Ik>,
}

impl Bone {
    pub const TAIL_IS_BONE: u16 = 0x0001;
    pub const IK: u16 = 0x0020;
    pub const INHERIT_ROTATION: u16 = 0x0100;
    pub const INHERIT_TRANSLATION: u16 = 0x0200;
    pub const FIXED_AXIS: u16 = 0x0400;
    pub const LOCAL_AXES: u16 = 0x0800;
    pub const EXTERNAL_PARENT: u16 = 0x2000;
}

#[derive(Debug, Clone, PartialEq)]
pub enum BoneTail {
    Bone(i32),
    Offset(Vec3),
}

#[derive(Debug, Clone)]
pub struct Ik {
    pub target: i32,
    pub loops: i32,
    pub limit_angle: f32,
    pub links: Vec<IkLink>,
}

#[derive(Debug, Clone)]
pub struct IkLink {
    pub bone: i32,
    /// 角度制限（下限, 上限）
    pub limits: Option<(Vec3, Vec3)>,
}

#[derive(Debug, Clone)]
pub struct Morph {
    pub name: String,
    pub name_en: String,
    pub panel: u8,
    pub kind: MorphKind,
}

#[derive(Debug, Clone)]
pub enum MorphKind {
    Group(Vec<(i32, f32)>),
    Vertex(Vec<VertexOffset>),
    Bone(Vec<BoneOffset>),
    /// UV・材質・フリップ・インパルスモーフ。中身は読み飛ばし、種類の番号だけ持ちます
    Unsupported(u8),
}

#[derive(Debug, Clone, PartialEq)]
pub struct VertexOffset {
    pub vertex: u32,
    pub offset: Vec3,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoneOffset {
    pub bone: i32,
    pub translation: Vec3,
    pub rotation: Vec4,
}
