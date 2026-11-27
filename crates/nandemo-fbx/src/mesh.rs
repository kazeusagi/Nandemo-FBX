use nandemo_core::{Mesh, Model};

use crate::binary::{Node, leaf};
use crate::matrix::{self, Mat4};
use crate::skeleton::Skeleton;
use crate::{CM_PER_METER, Document, object, p_color, p_double, props};

/// メッシュ・マテリアル・スキン・バインドポーズを書き出します。
pub(crate) fn write(doc: &mut Document, model: &Model, skeleton: &Skeleton) {
    let material_ids: Vec<i64> = model
        .materials
        .iter()
        .map(|m| {
            let id = doc.id();
            doc.add(material(id, &m.name, m.base_color));
            id
        })
        .collect();

    // バインドポーズに入れる（Model の ID, 行列）
    let mut pose: Vec<(i64, Mat4)> = Vec::new();

    for mesh in &model.meshes {
        let (model_id, geometry_id) = (doc.id(), doc.id());

        // このメッシュで使うマテリアル（接続順がマテリアル番号になります）
        let mut used: Vec<usize> = Vec::new();
        for p in &mesh.parts {
            if p.material < material_ids.len() && !used.contains(&p.material) {
                used.push(p.material);
            }
        }

        doc.add(mesh_model(model_id, &mesh.name));
        doc.add(geometry(geometry_id, mesh, &used));
        doc.connect(model_id, 0);
        doc.connect(geometry_id, model_id);
        for &m in &used {
            doc.connect(material_ids[m], model_id);
        }

        if write_skin(doc, mesh, geometry_id, skeleton, &model.bones) {
            pose.push((model_id, matrix::IDENTITY));
        }
    }

    if !pose.is_empty() {
        pose.extend(
            skeleton
                .ids
                .iter()
                .copied()
                .zip(skeleton.globals.iter().copied()),
        );
        let id = doc.id();
        doc.add(
            object("Pose", "Pose", id, &model.name, "BindPose")
                .child(leaf("Type", "BindPose"))
                .child(leaf("Version", 100))
                .child(leaf("NbPoseNodes", pose.len() as i32))
                .children(pose.into_iter().map(|(node, m)| {
                    Node::new("PoseNode")
                        .child(leaf("Node", node))
                        .child(leaf("Matrix", m.to_vec()))
                })),
        );
    }
}

/// スキンとボーンごとのクラスターを書き出します。書き出したら `true` を返します。
fn write_skin(
    doc: &mut Document,
    mesh: &Mesh,
    geometry_id: i64,
    skeleton: &Skeleton,
    bones: &[nandemo_core::Bone],
) -> bool {
    // ボーンごとの（頂点番号, ウェイト）
    let mut clusters: Vec<(Vec<i32>, Vec<f64>)> = vec![Default::default(); bones.len()];
    for (v, (joints, weights)) in mesh.joints.iter().zip(&mesh.weights).enumerate() {
        for (&b, &w) in joints.iter().zip(weights) {
            let Some((indexes, ws)) = clusters.get_mut(b as usize).filter(|_| w > 0.0) else {
                continue;
            };
            // 同じ頂点に同じボーンが重複していればまとめます
            if indexes.last() == Some(&(v as i32)) {
                *ws.last_mut().unwrap() += w as f64;
            } else {
                indexes.push(v as i32);
                ws.push(w as f64);
            }
        }
    }
    if clusters.iter().all(|(indexes, _)| indexes.is_empty()) {
        return false;
    }

    let skin = doc.id();
    doc.add(
        object("Deformer", "Deformer", skin, &mesh.name, "Skin")
            .child(leaf("Version", 101))
            .child(leaf("Link_DeformAcuracy", 50.0)),
    );
    doc.connect(skin, geometry_id);

    for (b, (indexes, weights)) in clusters.into_iter().enumerate() {
        if indexes.is_empty() {
            continue;
        }
        // Transform はメッシュ（原点）からボーン空間への変換、TransformLink はボーンのグローバル行列
        let link = skeleton.globals[b];
        let id = doc.id();
        doc.add(
            object("Deformer", "SubDeformer", id, &bones[b].name, "Cluster")
                .child(leaf("Version", 100))
                .child(Node::new("UserData").prop("").prop(""))
                .child(leaf("Indexes", indexes))
                .child(leaf("Weights", weights))
                .child(leaf("Transform", matrix::rigid_inverse(&link).to_vec()))
                .child(leaf("TransformLink", link.to_vec())),
        );
        doc.connect(id, skin);
        doc.connect(skeleton.ids[b], id);
    }
    true
}

fn mesh_model(id: i64, name: &str) -> Node {
    object("Model", "Model", id, name, "Mesh")
        .child(leaf("Version", 232))
        .child(props([]))
        .child(leaf("Shading", true))
        .child(leaf("Culling", "CullingOff"))
}

fn geometry(id: i64, mesh: &Mesh, materials: &[usize]) -> Node {
    let vertices = mesh
        .positions
        .iter()
        .flatten()
        .map(|&v| v as f64 * CM_PER_METER)
        .collect::<Vec<_>>();

    // 多角形の最後の頂点番号はビット反転して区切りを表します
    let polygon_indices = mesh
        .indices
        .chunks_exact(3)
        .flat_map(|t| [t[0] as i32, t[1] as i32, !(t[2] as i32)])
        .collect::<Vec<_>>();

    let mut node = object("Geometry", "Geometry", id, &mesh.name, "Mesh")
        .child(leaf("Vertices", vertices))
        .child(leaf("PolygonVertexIndex", polygon_indices))
        .child(leaf("GeometryVersion", 124));
    let mut layer = Node::new("Layer").prop(0).child(leaf("Version", 100));

    if !mesh.normals.is_empty() {
        let normals = mesh
            .normals
            .iter()
            .flatten()
            .map(|&v| v as f64)
            .collect::<Vec<_>>();
        node = node.child(
            layer_element("LayerElementNormal", "", "ByVertice", "Direct")
                .child(leaf("Normals", normals)),
        );
        layer = layer.child(layer_ref("LayerElementNormal"));
    }

    if !mesh.uvs.is_empty() {
        // FBX の UV は左下原点
        let uvs = mesh
            .uvs
            .iter()
            .flat_map(|&[u, v]| [u as f64, 1.0 - v as f64])
            .collect::<Vec<_>>();
        let uv_index = mesh.indices.iter().map(|&i| i as i32).collect::<Vec<_>>();
        node = node.child(
            layer_element(
                "LayerElementUV",
                "UVMap",
                "ByPolygonVertex",
                "IndexToDirect",
            )
            .child(leaf("UV", uvs))
            .child(leaf("UVIndex", uv_index)),
        );
        layer = layer.child(layer_ref("LayerElementUV"));
    }

    if !materials.is_empty() {
        let mut per_face = vec![0; mesh.indices.len() / 3];
        for p in &mesh.parts {
            let Some(local) = materials.iter().position(|&m| m == p.material) else {
                continue;
            };
            let faces = p.start / 3..((p.start + p.count) / 3).min(per_face.len());
            per_face[faces].fill(local as i32);
        }
        node = node.child(
            layer_element("LayerElementMaterial", "", "ByPolygon", "IndexToDirect")
                .child(leaf("Materials", per_face)),
        );
        layer = layer.child(layer_ref("LayerElementMaterial"));
    }

    node.child(layer)
}

fn layer_element(kind: &str, name: &str, mapping: &str, reference: &str) -> Node {
    Node::new(kind)
        .prop(0)
        .child(leaf("Version", 101))
        .child(leaf("Name", name))
        .child(leaf("MappingInformationType", mapping))
        .child(leaf("ReferenceInformationType", reference))
}

fn layer_ref(kind: &str) -> Node {
    Node::new("LayerElement")
        .child(leaf("Type", kind))
        .child(leaf("TypedIndex", 0))
}

fn material(id: i64, name: &str, [r, g, b, a]: [f32; 4]) -> Node {
    object("Material", "Material", id, name, "")
        .child(leaf("Version", 102))
        .child(leaf("ShadingModel", "phong"))
        .child(leaf("MultiLayer", 0))
        .child(props([
            p_color("DiffuseColor", [r, g, b]),
            p_double("DiffuseFactor", 1.0),
            p_double("TransparencyFactor", 1.0 - a as f64),
        ]))
}
