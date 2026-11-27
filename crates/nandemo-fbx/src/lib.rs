//! IR から FBX（バイナリ 7.4）を書き出します。
//!
//! 出力の座標系は Y-up・正面 +Z・右手系、単位はセンチメートル（UnitScaleFactor = 1）です。
//! 今のところメッシュ（位置・法線・UV）とマテリアルの色だけを書き出します。

pub mod binary;

use binary::{CREATION_TIME, FILE_ID, Node, Prop, VERSION, leaf};
use nandemo_core::{Mesh, Model};

const CREATOR: &str = concat!("Nandemo-FBX ", env!("CARGO_PKG_VERSION"));
const CM_PER_METER: f64 = 100.0;

/// モデルを FBX バイナリにします。
pub fn write_model(model: &Model) -> Vec<u8> {
    let mut ids = 1_000_000i64..;
    let mut objects = Vec::new();
    let mut connections = Vec::new();

    let material_ids: Vec<i64> = model
        .materials
        .iter()
        .map(|m| {
            let id = ids.next().unwrap();
            objects.push(material(id, &m.name, m.base_color));
            id
        })
        .collect();

    for mesh in &model.meshes {
        let (model_id, geometry_id) = (ids.next().unwrap(), ids.next().unwrap());

        // このメッシュで使うマテリアル（接続順がマテリアル番号になります）
        let mut used: Vec<usize> = Vec::new();
        for p in &mesh.parts {
            if p.material < material_ids.len() && !used.contains(&p.material) {
                used.push(p.material);
            }
        }

        objects.push(mesh_model(model_id, &mesh.name));
        objects.push(geometry(geometry_id, mesh, &used));
        connections.push(connect(model_id, 0));
        connections.push(connect(geometry_id, model_id));
        for &m in &used {
            connections.push(connect(material_ids[m], model_id));
        }
    }

    let count = |name: &str| objects.iter().filter(|n| n.name == name).count() as i32;
    let definitions = Node::new("Definitions")
        .child(leaf("Version", 100))
        .child(leaf("Count", 1 + objects.len() as i32))
        .child(object_type("GlobalSettings", 1))
        .child(object_type("Model", count("Model")))
        .child(object_type("Geometry", count("Geometry")))
        .child(object_type("Material", count("Material")));

    binary::encode(&[
        header_extension(),
        leaf("FileId", Prop::Raw(FILE_ID.to_vec())),
        leaf("CreationTime", CREATION_TIME),
        leaf("Creator", CREATOR),
        global_settings(),
        Node::new("Documents").child(leaf("Count", 0)),
        Node::new("References"),
        definitions,
        Node::new("Objects").children(objects),
        Node::new("Connections").children(connections),
    ])
}

fn header_extension() -> Node {
    let time = Node::new("CreationTimeStamp").children(
        [
            ("Version", 1000),
            ("Year", 1970),
            ("Month", 1),
            ("Day", 1),
            ("Hour", 10),
            ("Minute", 0),
            ("Second", 19),
            ("Millisecond", 463),
        ]
        .map(|(k, v)| leaf(k, v)),
    );
    Node::new("FBXHeaderExtension")
        .child(leaf("FBXHeaderVersion", 1003))
        .child(leaf("FBXVersion", VERSION as i32))
        .child(time)
        .child(leaf("Creator", CREATOR))
}

fn global_settings() -> Node {
    let props = Node::new("Properties70").children([
        p_int("UpAxis", 1),
        p_int("UpAxisSign", 1),
        p_int("FrontAxis", 2),
        p_int("FrontAxisSign", 1),
        p_int("CoordAxis", 0),
        p_int("CoordAxisSign", 1),
        p_int("OriginalUpAxis", 1),
        p_int("OriginalUpAxisSign", 1),
        p_double("UnitScaleFactor", 1.0),
        p_double("OriginalUnitScaleFactor", 1.0),
        p_enum("TimeMode", 6), // 30 fps
    ]);
    Node::new("GlobalSettings")
        .child(leaf("Version", 1000))
        .child(props)
}

fn object_type(name: &str, count: i32) -> Node {
    Node::new("ObjectType")
        .prop(name)
        .child(leaf("Count", count))
}

fn mesh_model(id: i64, name: &str) -> Node {
    Node::new("Model")
        .prop(id)
        .prop(class_name(name, "Model"))
        .prop("Mesh")
        .child(leaf("Version", 232))
        .child(Node::new("Properties70"))
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

    let mut node = Node::new("Geometry")
        .prop(id)
        .prop(class_name(&mesh.name, "Geometry"))
        .prop("Mesh")
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
    let props = Node::new("Properties70").children([
        p_color("DiffuseColor", [r, g, b]),
        p_double("DiffuseFactor", 1.0),
        p_double("TransparencyFactor", 1.0 - a as f64),
    ]);
    Node::new("Material")
        .prop(id)
        .prop(class_name(name, "Material"))
        .prop("")
        .child(leaf("Version", 102))
        .child(leaf("ShadingModel", "phong"))
        .child(leaf("MultiLayer", 0))
        .child(props)
}

fn connect(child: i64, parent: i64) -> Node {
    Node::new("C").prop("OO").prop(child).prop(parent)
}

/// バイナリ FBX のオブジェクト名は「名前 \0\x01 クラス名」
fn class_name(name: &str, class: &str) -> String {
    format!("{name}\0\u{1}{class}")
}

fn p(name: &str, kind: &str, label: &str, flags: &str) -> Node {
    Node::new("P").prop(name).prop(kind).prop(label).prop(flags)
}

fn p_int(name: &str, v: i32) -> Node {
    p(name, "int", "Integer", "").prop(v)
}

fn p_enum(name: &str, v: i32) -> Node {
    p(name, "enum", "", "").prop(v)
}

fn p_double(name: &str, v: f64) -> Node {
    p(name, "double", "Number", "").prop(v)
}

fn p_color(name: &str, [r, g, b]: [f32; 3]) -> Node {
    p(name, "Color", "", "A")
        .prop(r as f64)
        .prop(g as f64)
        .prop(b as f64)
}
