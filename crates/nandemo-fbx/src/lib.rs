//! IR から FBX（バイナリ 7.4）を書き出します。
//!
//! 出力の座標系は Y-up・正面 +Z・右手系、単位はセンチメートル（UnitScaleFactor = 1）です。
//! ボーン・スキン・メッシュ（位置・法線・UV）・マテリアルの色・ボーンアニメーションを書き出します。

mod anim;
pub mod binary;
mod matrix;
mod mesh;
mod skeleton;

use binary::{CREATION_TIME, FILE_ID, Node, Prop, VERSION, leaf};
use nandemo_core::{Model, Motion};

const CREATOR: &str = concat!("Nandemo-FBX ", env!("CARGO_PKG_VERSION"));
const CM_PER_METER: f64 = 100.0;

#[derive(Debug, Clone)]
pub struct Options {
    /// メッシュ・マテリアル・スキンを書き出すか。
    /// `false` にするとボーンとアニメーションだけの FBX になります
    pub mesh: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { mesh: true }
    }
}

/// モデル（とモーション）を FBX バイナリにします。
pub fn write(model: &Model, motion: Option<&Motion>, options: &Options) -> Vec<u8> {
    let mut doc = Document::default();
    let skeleton = skeleton::write(&mut doc, &model.bones);
    if options.mesh {
        mesh::write(&mut doc, model, &skeleton);
    }
    if let Some(motion) = motion {
        anim::write(&mut doc, model, motion, &skeleton);
    }
    doc.encode()
}

/// 書き出し中のオブジェクトと接続
#[derive(Default)]
struct Document {
    last_id: i64,
    objects: Vec<Node>,
    connections: Vec<Node>,
}

impl Document {
    fn id(&mut self) -> i64 {
        self.last_id += 1;
        1_000_000 + self.last_id
    }

    fn add(&mut self, node: Node) {
        self.objects.push(node);
    }

    /// オブジェクト同士の接続（`parent` が 0 ならシーンのルート）
    fn connect(&mut self, child: i64, parent: i64) {
        self.connections
            .push(Node::new("C").prop("OO").prop(child).prop(parent));
    }

    /// オブジェクトからプロパティへの接続（アニメーションカーブなど）
    fn connect_prop(&mut self, child: i64, parent: i64, prop: &str) {
        self.connections.push(
            Node::new("C")
                .prop("OP")
                .prop(child)
                .prop(parent)
                .prop(prop),
        );
    }

    fn encode(self) -> Vec<u8> {
        // 種類ごとの数（出てきた順）
        let mut types: Vec<(&str, i32)> = vec![("GlobalSettings", 1)];
        for o in &self.objects {
            match types.iter_mut().find(|(name, _)| *name == o.name) {
                Some((_, n)) => *n += 1,
                None => types.push((&o.name, 1)),
            }
        }
        let definitions = Node::new("Definitions")
            .child(leaf("Version", 100))
            .child(leaf("Count", 1 + self.objects.len() as i32))
            .children(types.iter().map(|&(name, count)| {
                Node::new("ObjectType")
                    .prop(name)
                    .child(leaf("Count", count))
            }));

        binary::encode(&[
            header_extension(),
            leaf("FileId", Prop::Raw(FILE_ID.to_vec())),
            leaf("CreationTime", CREATION_TIME),
            leaf("Creator", CREATOR),
            global_settings(),
            Node::new("Documents").child(leaf("Count", 0)),
            Node::new("References"),
            definitions,
            Node::new("Objects").children(self.objects),
            Node::new("Connections").children(self.connections),
        ])
    }
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

/// オブジェクトのノード。バイナリ FBX の名前は「名前 \0\x01 クラス名」です
fn object(kind: &str, class: &str, id: i64, name: &str, subclass: &str) -> Node {
    Node::new(kind)
        .prop(id)
        .prop(format!("{name}\0\u{1}{class}"))
        .prop(subclass)
}

fn props(children: impl IntoIterator<Item = Node>) -> Node {
    Node::new("Properties70").children(children)
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

fn p_number(name: &str, v: f64) -> Node {
    p(name, "Number", "", "A").prop(v)
}

fn p_time(name: &str, v: i64) -> Node {
    p(name, "KTime", "Time", "").prop(v)
}

fn p_color(name: &str, [r, g, b]: [f32; 3]) -> Node {
    p(name, "Color", "", "A")
        .prop(r as f64)
        .prop(g as f64)
        .prop(b as f64)
}

/// `Lcl Translation` など、型名とプロパティ名が同じ 3 成分のプロパティ
fn p_vec3(name: &str, [x, y, z]: [f64; 3]) -> Node {
    p(name, name, "", "A").prop(x).prop(y).prop(z)
}
