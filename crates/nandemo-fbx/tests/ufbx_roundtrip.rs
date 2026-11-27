//! 書き出した FBX を ufbx（厳密モード）で読み戻して確かめます。

use nandemo_core::{
    Bezier, Bone, BoneKey, BoneTrack, Material, Mesh, MeshPart, Model, Motion, QUAT_IDENTITY,
};
use nandemo_fbx::Options;

/// 2 つの三角形からなる四角形。三角形ごとに別のマテリアルを使います。
/// 下の 2 頂点は root、上の 2 頂点は arm（1 つは root と半々）に追従します
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
        joints: vec![[0, 0, 0, 0], [0, 0, 0, 0], [1, 0, 0, 0], [1, 0, 0, 0]],
        weights: vec![
            [1.0, 0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [0.5, 0.5, 0.0, 0.0],
        ],
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
    let bone = |name: &str, parent, translation| Bone {
        name: name.into(),
        parent,
        translation,
        rotation: QUAT_IDENTITY,
    };
    Model {
        name: "quad".into(),
        meshes: vec![mesh],
        materials: vec![
            material("赤", [1.0, 0.0, 0.0, 1.0]),
            material("青", [0.0, 0.0, 1.0, 1.0]),
        ],
        textures: vec![],
        bones: vec![
            bone("root", None, [0.0; 3]),
            bone("arm", Some(0), [0.0, 1.0, 0.0]),
        ],
    }
}

/// 30 フレーム（1 秒）で root が X に 1 m 動き、arm が Z 軸まわりに 90° 回るモーション
fn motion() -> Motion {
    let h = std::f32::consts::FRAC_1_SQRT_2;
    let key = |frame, translation, rotation| BoneKey {
        frame,
        translation,
        rotation,
        interpolation: [Bezier::LINEAR; 4],
    };
    let track = |bone: &str, keys| BoneTrack {
        bone: bone.into(),
        keys,
    };
    Motion {
        name: "test".into(),
        fps: 30.0,
        bones: vec![
            track(
                "root",
                vec![
                    key(0, [0.0; 3], QUAT_IDENTITY),
                    key(30, [1.0, 0.0, 0.0], QUAT_IDENTITY),
                ],
            ),
            track(
                "arm",
                vec![
                    key(0, [0.0; 3], QUAT_IDENTITY),
                    key(30, [0.0; 3], [0.0, 0.0, h, h]),
                ],
            ),
            track("not_in_model", vec![key(0, [0.0; 3], QUAT_IDENTITY)]),
        ],
        morphs: vec![],
    }
}

fn load(data: &[u8]) -> ufbx::SceneRoot {
    let opts = ufbx::LoadOpts {
        strict: true,
        ..Default::default()
    };
    ufbx::load_memory(data, opts).expect("ufbx で読み込めること")
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn mesh_roundtrip() {
    let scene = load(&nandemo_fbx::write(&quad(), None, &Options::default()));

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
fn skeleton_and_skin() {
    let scene = load(&nandemo_fbx::write(&quad(), None, &Options::default()));

    let arm = scene.find_node("arm").expect("arm ボーン");
    assert!(arm.bone.is_some());
    assert_eq!(&*arm.parent.as_ref().unwrap().element.name, "root");
    assert!(close(arm.node_to_world.m13, 100.0));

    assert_eq!(scene.skin_deformers.len(), 1);
    let skin = &scene.skin_deformers[0];
    assert_eq!(skin.clusters.len(), 2);
    let cluster = skin
        .clusters
        .iter()
        .find(|c| {
            c.bone_node
                .as_ref()
                .is_some_and(|n| &*n.element.name == "arm")
        })
        .expect("arm のクラスター");
    let vertices: Vec<u32> = cluster.vertices.iter().copied().collect();
    let weights: Vec<f64> = cluster.weights.iter().copied().collect();
    assert_eq!(vertices, [2, 3]);
    assert_eq!(weights, [1.0, 0.5]);
    // メッシュの原点からボーン空間へ: arm は y = 100 cm にあるので -100
    assert!(close(cluster.geometry_to_bone.m13, -100.0));

    assert_eq!(scene.poses.len(), 1);
    assert!(scene.poses[0].is_bind_pose);
    assert_eq!(scene.poses[0].bone_poses.len(), 3);
}

#[test]
fn bone_animation() {
    let scene = load(&nandemo_fbx::write(
        &quad(),
        Some(&motion()),
        &Options::default(),
    ));

    assert_eq!(scene.anim_stacks.len(), 1);
    let stack = &scene.anim_stacks[0];
    assert_eq!(&*stack.element.name, "test");
    assert!(close(stack.time_end, 1.0));

    // 0.5 秒（15 フレーム）で root は X に 50 cm、arm は Z 軸まわりに 45°
    let root = scene.find_node("root").unwrap();
    let t = ufbx::evaluate_transform(&stack.anim, root, 0.5);
    assert!(close(t.translation.x, 50.0), "{:?}", t.translation);

    let arm = scene.find_node("arm").unwrap();
    let t = ufbx::evaluate_transform(&stack.anim, arm, 0.5);
    let (s, c) = (std::f64::consts::PI / 8.0).sin_cos();
    let q = t.rotation;
    assert!(close(q.z.abs(), s) && close(q.w.abs(), c), "{q:?}");
    assert!(close(t.translation.y, 100.0));

    let t = ufbx::evaluate_transform(&stack.anim, arm, 1.0);
    assert!(close(t.rotation.z.abs(), std::f64::consts::FRAC_1_SQRT_2));
}

#[test]
fn animation_only() {
    let options = Options { mesh: false };
    let scene = load(&nandemo_fbx::write(&quad(), Some(&motion()), &options));

    assert_eq!(scene.meshes.len(), 0);
    assert_eq!(scene.materials.len(), 0);
    assert_eq!(scene.skin_deformers.len(), 0);
    assert!(scene.find_node("root").is_some());
    assert!(scene.find_node("arm").is_some());
    assert_eq!(scene.anim_stacks.len(), 1);
}

#[test]
fn empty_model_is_valid() {
    let scene = load(&nandemo_fbx::write(
        &Model::default(),
        None,
        &Options::default(),
    ));
    assert_eq!(scene.meshes.len(), 0);
}
