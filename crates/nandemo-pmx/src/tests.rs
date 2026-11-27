use crate::*;

/// テスト用の最小 PMX（UTF-16、インデックスはすべて 1 バイト）
fn sample_pmx() -> Vec<u8> {
    let mut b = Bytes::default();
    b.raw(b"PMX ");
    b.f32(2.0);
    b.raw(&[8, 0, 0, 1, 1, 1, 1, 1, 1]);
    b.text("テスト");
    b.text("test");
    b.text("");
    b.text("");

    // 頂点 3 つ（BDEF1、ボーン 0）
    b.i32(3);
    for pos in [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 2.0]] {
        b.f32s(&pos);
        b.f32s(&[0.0, 0.0, -1.0]);
        b.f32s(&[0.5, 0.25]);
        b.raw(&[0, 0]);
        b.f32(1.0);
    }

    // 三角形 1 つ
    b.i32(3);
    b.raw(&[0, 1, 2]);

    // テクスチャ
    b.i32(1);
    b.text("tex\\body.png");

    // マテリアル
    b.i32(1);
    b.text("体");
    b.text("body");
    b.f32s(&[1.0, 0.5, 0.25, 1.0]);
    b.f32s(&[0.0; 3]);
    b.f32(5.0);
    b.f32s(&[0.0; 3]);
    b.raw(&[0]);
    b.f32s(&[0.0; 4]);
    b.f32(1.0);
    b.raw(&[0, 0xff, 0, 1, 3]); // テクスチャ 0、スフィアなし、共有トゥーン 3
    b.text("");
    b.i32(3);

    // ボーン 2 本（センター → 頭、頭は IK 付き）
    b.i32(2);
    b.text("センター");
    b.text("center");
    b.f32s(&[0.0, 1.0, 0.0]);
    b.raw(&[0xff]);
    b.i32(0);
    b.raw(&0x0001u16.to_le_bytes());
    b.raw(&[1]);

    b.text("頭");
    b.text("head");
    b.f32s(&[0.0, 2.0, 1.0]);
    b.raw(&[0]);
    b.i32(0);
    b.raw(&(Bone::IK).to_le_bytes());
    b.f32s(&[0.0, 1.0, 0.0]);
    b.raw(&[0]);
    b.i32(40);
    b.f32(2.0);
    b.i32(1);
    b.raw(&[0, 1]);
    b.f32s(&[-1.0, 0.0, 0.0]);
    b.f32s(&[1.0, 0.0, 0.0]);

    // モーフ 2 つ（頂点モーフ、UV モーフ）
    b.i32(2);
    b.text("あ");
    b.text("a");
    b.raw(&[3, 1]);
    b.i32(1);
    b.raw(&[2]);
    b.f32s(&[0.0, 0.0, 1.0]);

    b.text("uv");
    b.text("uv");
    b.raw(&[4, 3]);
    b.i32(1);
    b.raw(&[0]);
    b.f32s(&[0.0; 4]);

    b.0
}

#[derive(Default)]
struct Bytes(Vec<u8>);

impl Bytes {
    fn raw(&mut self, v: &[u8]) {
        self.0.extend_from_slice(v);
    }
    fn i32(&mut self, v: i32) {
        self.raw(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.raw(&v.to_le_bytes());
    }
    fn f32s(&mut self, v: &[f32]) {
        v.iter().for_each(|&x| self.f32(x));
    }
    fn text(&mut self, s: &str) {
        let bytes: Vec<u8> = s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        self.i32(bytes.len() as i32);
        self.raw(&bytes);
    }
}

#[test]
fn parses_sample() {
    let pmx = parse(&sample_pmx()).unwrap();
    assert_eq!(pmx.name, "テスト");
    assert_eq!(pmx.vertices.len(), 3);
    assert_eq!(pmx.vertices[2].position, [0.0, 1.0, 2.0]);
    assert_eq!(pmx.vertices[0].weight, Weight::Bdef1(0));
    assert_eq!(pmx.indices, [0, 1, 2]);
    assert_eq!(pmx.textures, ["tex\\body.png"]);

    let m = &pmx.materials[0];
    assert_eq!(m.name, "体");
    assert_eq!((m.texture, m.sphere_texture), (0, -1));
    assert_eq!(m.toon, Toon::Shared(3));
    assert_eq!(m.index_count, 3);

    assert_eq!(pmx.bones[0].parent, -1);
    assert_eq!(pmx.bones[0].tail, BoneTail::Bone(1));
    let ik = pmx.bones[1].ik.as_ref().unwrap();
    assert_eq!((ik.target, ik.loops, ik.links.len()), (0, 40, 1));
    assert!(ik.links[0].limits.is_some());

    assert!(matches!(&pmx.morphs[0].kind, MorphKind::Vertex(o) if o[0].vertex == 2));
    assert!(matches!(pmx.morphs[1].kind, MorphKind::Unsupported(3)));
}

#[test]
fn rejects_truncated_file() {
    let data = sample_pmx();
    assert!(parse(&data[..data.len() - 1]).is_err());
    assert!(parse(b"PMD ").is_err());
}

#[test]
fn converts_to_right_handed_meters() {
    let model = to_model(&parse(&sample_pmx()).unwrap());
    let mesh = &model.meshes[0];
    let s = MMD_UNIT_METERS;

    assert_eq!(mesh.positions[2], [0.0, s, -2.0 * s]);
    assert_eq!(mesh.normals[0], [0.0, 0.0, 1.0]);
    assert_eq!(mesh.indices, [2, 1, 0]);
    assert_eq!(mesh.joints[0], [0, 0, 0, 0]);
    assert_eq!(mesh.weights[0], [1.0, 0.0, 0.0, 0.0]);
    assert_eq!(
        mesh.parts,
        [nandemo_core::MeshPart {
            material: 0,
            start: 0,
            count: 3
        }]
    );
    assert_eq!(mesh.morphs.len(), 1);
    assert_eq!(mesh.morphs[0].offsets[0].position, [0.0, 0.0, -s]);

    assert_eq!(model.materials[0].base_color_texture, Some(0));
    assert_eq!(model.textures[0].path, "tex/body.png");

    assert_eq!(model.bones[0].parent, None);
    assert_eq!(model.bones[1].parent, Some(0));
    assert_eq!(model.bones[1].translation, [0.0, s, -s]);
}
