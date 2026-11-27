//! Nandemo-FBX の中間表現（IR）と共通ユーティリティ。
//!
//! 各フォーマットのリーダーは入力を IR に変換し、FBX ライターは IR だけを見て書き出します。
//! 座標系や単位の変換はリーダー側で一度だけ行います。
//!
//! IR の規約（glTF / VRM 1.0 と同じ）:
//! - 右手系・Y-up・モデルの正面は +Z
//! - 単位はメートル
//! - UV は左上原点
//! - 三角形は反時計回りが表面

pub mod bin;
mod model;
mod motion;

pub use model::*;
pub use motion::*;

pub type Vec2 = [f32; 2];
pub type Vec3 = [f32; 3];
pub type Vec4 = [f32; 4];
/// クォータニオン（x, y, z, w）
pub type Quat = [f32; 4];

pub const QUAT_IDENTITY: Quat = [0.0, 0.0, 0.0, 1.0];
