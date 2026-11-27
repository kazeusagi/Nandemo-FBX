use nandemo_core::Bone;
use nandemo_core::math::{add, quat_mul, rotate};

use crate::binary::leaf;
use crate::matrix::{self, Mat4};
use crate::{CM_PER_METER, Document, object, p_double, p_enum, p_vec3, props};

pub(crate) struct Skeleton {
    /// ボーンごとの Model の ID
    pub ids: Vec<i64>,
    /// ボーンごとのレストポーズのグローバル行列
    pub globals: Vec<Mat4>,
}

/// ボーンを LimbNode として書き出します。
pub(crate) fn write(doc: &mut Document, bones: &[Bone]) -> Skeleton {
    let parents = parents(bones);
    let mut ids = Vec::with_capacity(bones.len());
    for b in bones {
        let attr = doc.id();
        doc.add(
            object("NodeAttribute", "NodeAttribute", attr, &b.name, "LimbNode")
                .child(props([p_double("Size", 10.0)]))
                .child(leaf("TypeFlags", "Skeleton")),
        );

        let id = doc.id();
        let t = b.translation.map(|v| v as f64 * CM_PER_METER);
        doc.add(
            object("Model", "Model", id, &b.name, "LimbNode")
                .child(leaf("Version", 232))
                .child(props([
                    p_vec3("Lcl Translation", t),
                    p_vec3("Lcl Rotation", matrix::euler_xyz(b.rotation, None)),
                    p_enum("InheritType", 1),
                ]))
                .child(leaf("Shading", true))
                .child(leaf("Culling", "CullingOff")),
        );
        doc.connect(attr, id);
        ids.push(id);
    }

    for (i, parent) in parents.iter().enumerate() {
        doc.connect(ids[i], parent.map_or(0, |p| ids[p]));
    }

    let globals = globals(bones, &parents)
        .into_iter()
        .map(|(t, r)| matrix::from_rt(t, r))
        .collect();
    Skeleton { ids, globals }
}

/// 親の番号。範囲外や循環している親は無視します
fn parents(bones: &[Bone]) -> Vec<Option<usize>> {
    (0..bones.len())
        .map(|i| {
            let parent = bones[i].parent.filter(|&p| p < bones.len())?;
            let mut cur = Some(parent);
            for _ in 0..bones.len() {
                match cur {
                    Some(c) if c == i => return None,
                    Some(c) => cur = bones[c].parent.filter(|&p| p < bones.len()),
                    None => break,
                }
            }
            Some(parent)
        })
        .collect()
}

/// ボーンごとのグローバルな（位置, 回転）
fn globals(bones: &[Bone], parents: &[Option<usize>]) -> Vec<([f32; 3], [f32; 4])> {
    fn resolve(
        i: usize,
        bones: &[Bone],
        parents: &[Option<usize>],
        out: &mut Vec<Option<([f32; 3], [f32; 4])>>,
    ) -> ([f32; 3], [f32; 4]) {
        if let Some(g) = out[i] {
            return g;
        }
        let b = &bones[i];
        let g = match parents[i] {
            Some(p) => {
                let (pt, pr) = resolve(p, bones, parents, out);
                (add(pt, rotate(pr, b.translation)), quat_mul(pr, b.rotation))
            }
            None => (b.translation, b.rotation),
        };
        out[i] = Some(g);
        g
    }

    let mut out = vec![None; bones.len()];
    (0..bones.len())
        .map(|i| resolve(i, bones, parents, &mut out))
        .collect()
}
