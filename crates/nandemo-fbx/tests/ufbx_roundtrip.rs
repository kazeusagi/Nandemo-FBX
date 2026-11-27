//! 書き出した FBX を ufbx（厳密モード）で読み戻して確かめます。

use nandemo_core::{Material, Mesh, MeshPart, Model};

/// 2 つの三角形からなる四角形。三角形ごとに別のマテリアルを使います
fn quad() -> Model {
    let mesh = Mesh {
        name: "quad".into(),
        positions: vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        uvs: vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
        indices: vec![0, 1, 2, 0, 2, 3],
        parts: vec![
            MeshPart {
                material: 0,
                start: 0,
                count: 3,
            },
            MeshPart {
                material: 1,
                start: 3,
                count: 3,
            },
        ],
        ..Default::default()
    };
    let material = |name: &str, color| Material {
        name: name.into(),
        base_color: color,
        base_color_texture: None,
    };
    Model {
        name: "quad".into(),
        meshes: vec![mesh],
        materials: vec![
            material("赤", [1.0, 0.0, 0.0, 1.0]),
            material("青", [0.0, 0.0, 1.0, 1.0]),
        ],
        ..Default::default()
    }
}

fn load(data: &[u8]) -> ufbx::SceneRoot {
    let opts = ufbx::LoadOpts {
        strict: true,
        ..Default::default()
    };
    ufbx::load_memory(data, opts).expect("ufbx で読み込めること")
}

#[test]
fn mesh_roundtrip() {
    let scene = load(&nandemo_fbx::write_model(&quad()));

    assert_eq!(scene.metadata.version, 7400);
    assert_eq!(scene.settings.unit_meters, 0.01);
    assert_eq!(scene.settings.axes.up, ufbx::CoordinateAxis::PositiveY);
    assert_eq!(scene.settings.axes.front, ufbx::CoordinateAxis::PositiveZ);

    assert_eq!(scene.meshes.len(), 1);
    let mesh = &scene.meshes[0];
    assert_eq!(&*mesh.element.name, "quad");
    assert_eq!(mesh.num_vertices, 4);
    assert_eq!(mesh.num_faces, 2);

    // メートル → センチメートル
    let p = mesh.vertices[2];
    assert_eq!((p.x, p.y, p.z), (100.0, 100.0, 0.0));

    // 頂点 0 の UV は左上原点 (0, 1) → 左下原点 (0, 0)
    assert!(mesh.vertex_uv.exists);
    let uv = mesh.vertex_uv[0];
    assert_eq!((uv.x, uv.y), (0.0, 0.0));
    assert!(mesh.vertex_normal.exists);

    assert_eq!(mesh.materials.len(), 2);
    assert_eq!(&*mesh.materials[1].element.name, "青");
    assert_eq!((mesh.face_material[0], mesh.face_material[1]), (0, 1));
    let blue = mesh.materials[1].fbx.diffuse_color.value_vec4;
    assert_eq!((blue.x, blue.y, blue.z), (0.0, 0.0, 1.0));
}

#[test]
fn empty_model_is_valid() {
    let scene = load(&nandemo_fbx::write_model(&Model::default()));
    assert_eq!(scene.meshes.len(), 0);
}
