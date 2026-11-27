use nandemo_core as ir;

use crate::{MorphKind, Pmx, Weight};

/// MMD の 1 単位をメートルにした値（一般的な換算 1 単位 = 8 cm）
pub const MMD_UNIT_METERS: f32 = 0.08;

/// PMX を IR に変換します。
///
/// MMD は左手系なので Z を反転し、三角形の向きを逆にします。
pub fn to_model(pmx: &Pmx) -> ir::Model {
    let bone = |i: i32| usize::try_from(i).ok().filter(|&i| i < pmx.bones.len());

    let mut joints = Vec::with_capacity(pmx.vertices.len());
    let mut weights = Vec::with_capacity(pmx.vertices.len());
    for v in &pmx.vertices {
        let (j, w) = skin(&v.weight);
        joints.push(j.map(|b| bone(b).unwrap_or(0) as u32));
        let w: [f32; 4] = std::array::from_fn(|k| {
            if bone(j[k]).is_some() {
                w[k].max(0.0)
            } else {
                0.0
            }
        });
        let sum: f32 = w.iter().sum();
        weights.push(if sum > 0.0 { w.map(|x| x / sum) } else { w });
    }

    let mut parts = Vec::with_capacity(pmx.materials.len());
    let mut start = 0;
    for (material, m) in pmx.materials.iter().enumerate() {
        let count = m.index_count.min(pmx.indices.len().saturating_sub(start));
        parts.push(ir::MeshPart {
            material,
            start,
            count,
        });
        start += count;
    }

    let morphs = pmx
        .morphs
        .iter()
        .filter_map(|m| match &m.kind {
            MorphKind::Vertex(offsets) => Some(ir::Morph {
                name: m.name.clone(),
                offsets: offsets
                    .iter()
                    .map(|o| ir::MorphOffset {
                        vertex: o.vertex,
                        position: position(o.offset),
                    })
                    .collect(),
            }),
            _ => None,
        })
        .collect();

    let mesh = ir::Mesh {
        name: pmx.name.clone(),
        positions: pmx.vertices.iter().map(|v| position(v.position)).collect(),
        normals: pmx.vertices.iter().map(|v| direction(v.normal)).collect(),
        uvs: pmx.vertices.iter().map(|v| v.uv).collect(),
        joints,
        weights,
        indices: pmx
            .indices
            .chunks_exact(3)
            .flat_map(|t| [t[2], t[1], t[0]])
            .collect(),
        parts,
        morphs,
    };

    let materials = pmx
        .materials
        .iter()
        .map(|m| ir::Material {
            name: m.name.clone(),
            base_color: m.diffuse,
            base_color_texture: usize::try_from(m.texture)
                .ok()
                .filter(|&i| i < pmx.textures.len()),
        })
        .collect();

    let textures = pmx
        .textures
        .iter()
        .map(|t| ir::Texture {
            path: t.replace('\\', "/"),
        })
        .collect();

    let bones = pmx
        .bones
        .iter()
        .map(|b| {
            let parent = bone(b.parent);
            let origin = parent.map_or([0.0; 3], |p| pmx.bones[p].position);
            let local = std::array::from_fn(|k| b.position[k] - origin[k]);
            ir::Bone {
                name: b.name.clone(),
                parent,
                translation: position(local),
                rotation: ir::QUAT_IDENTITY,
            }
        })
        .collect();

    ir::Model {
        name: pmx.name.clone(),
        meshes: vec![mesh],
        materials,
        textures,
        bones,
    }
}

fn position(v: ir::Vec3) -> ir::Vec3 {
    let [x, y, z] = direction(v);
    [
        x * MMD_UNIT_METERS,
        y * MMD_UNIT_METERS,
        z * MMD_UNIT_METERS,
    ]
}

fn direction([x, y, z]: ir::Vec3) -> ir::Vec3 {
    [x, y, -z]
}

/// SDEF は BDEF2、QDEF は BDEF4 として扱います。
fn skin(weight: &Weight) -> ([i32; 4], [f32; 4]) {
    match *weight {
        Weight::Bdef1(b) => ([b, -1, -1, -1], [1.0, 0.0, 0.0, 0.0]),
        Weight::Bdef2 { bones, weight } | Weight::Sdef { bones, weight, .. } => (
            [bones[0], bones[1], -1, -1],
            [weight, 1.0 - weight, 0.0, 0.0],
        ),
        Weight::Bdef4 { bones, weights } | Weight::Qdef { bones, weights } => (bones, weights),
    }
}
