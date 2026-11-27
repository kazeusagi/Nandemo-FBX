//! FBX バイナリ（7.4）のノードツリーとエンコーダ。
//!
//! 形式は Blender の FBX 書き出し（io_scene_fbx/encode_bin.py）と同じ方針です。

pub const VERSION: u32 = 7400;

/// FileId・CreationTime と、それに対応するフッター。
/// フッターは作成日時から計算される値なので、Blender と同じ固定値を使います。
pub const FILE_ID: [u8; 16] = *b"\x28\xb3\x2a\xeb\xb6\x24\xcc\xc2\xbf\xc8\xb0\x2a\xa9\x2b\xfc\xf1";
pub const CREATION_TIME: &str = "1970-01-01 10:00:19:463";
const FOOTER_ID: [u8; 16] = *b"\xfa\xbc\xab\x09\xd0\xc8\xd4\x66\xb1\x76\xfb\x83\x1c\xf7\x26\x7e";
const FOOTER_MAGIC: [u8; 16] = *b"\xf8\x5a\x8c\x6a\xde\xf5\xd9\x7e\xec\xe9\x0c\xe3\x75\x8f\x29\x0b";

const HEADER_MAGIC: &[u8] = b"Kaydara FBX Binary  \x00\x1a\x00";
const NULL_RECORD: [u8; 13] = [0; 13];

#[derive(Debug, Clone, PartialEq)]
pub enum Prop {
    Bool(bool),
    I16(i16),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    Str(String),
    Raw(Vec<u8>),
    I32s(Vec<i32>),
    I64s(Vec<i64>),
    F32s(Vec<f32>),
    F64s(Vec<f64>),
}

impl From<bool> for Prop {
    fn from(v: bool) -> Self {
        Prop::Bool(v)
    }
}
impl From<i32> for Prop {
    fn from(v: i32) -> Self {
        Prop::I32(v)
    }
}
impl From<i64> for Prop {
    fn from(v: i64) -> Self {
        Prop::I64(v)
    }
}
impl From<f64> for Prop {
    fn from(v: f64) -> Self {
        Prop::F64(v)
    }
}
impl From<&str> for Prop {
    fn from(v: &str) -> Self {
        Prop::Str(v.into())
    }
}
impl From<String> for Prop {
    fn from(v: String) -> Self {
        Prop::Str(v)
    }
}
impl From<Vec<i32>> for Prop {
    fn from(v: Vec<i32>) -> Self {
        Prop::I32s(v)
    }
}
impl From<Vec<f64>> for Prop {
    fn from(v: Vec<f64>) -> Self {
        Prop::F64s(v)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub name: String,
    pub props: Vec<Prop>,
    pub children: Vec<Node>,
}

impl Node {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            props: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn prop(mut self, p: impl Into<Prop>) -> Self {
        self.props.push(p.into());
        self
    }

    pub fn child(mut self, c: Node) -> Self {
        self.children.push(c);
        self
    }

    pub fn children(mut self, c: impl IntoIterator<Item = Node>) -> Self {
        self.children.extend(c);
        self
    }
}

/// プロパティを 1 つだけ持つノード（`Version: 100` など）
pub fn leaf(name: &str, p: impl Into<Prop>) -> Node {
    Node::new(name).prop(p)
}

/// トップレベルのノード列を FBX バイナリにします。
pub fn encode(nodes: &[Node]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(HEADER_MAGIC);
    out.extend(VERSION.to_le_bytes());
    for n in nodes {
        write_node(&mut out, n);
    }
    out.extend(NULL_RECORD);
    write_footer(&mut out);
    out
}

fn write_node(out: &mut Vec<u8>, node: &Node) {
    let start = out.len();
    out.extend([0; 12]); // EndOffset, NumProperties, PropertyListLen は後で書きます
    out.push(node.name.len() as u8);
    out.extend(node.name.as_bytes());

    let props_start = out.len();
    for p in &node.props {
        write_prop(out, p);
    }
    let props_len = out.len() - props_start;

    for c in &node.children {
        write_node(out, c);
    }
    // 子を持つノードと、プロパティも子もないノードの末尾には空レコードが必要です
    let always = matches!(node.name.as_str(), "AnimationStack" | "AnimationLayer");
    if !node.children.is_empty() || node.props.is_empty() || always {
        out.extend(NULL_RECORD);
    }

    let header = [out.len() as u32, node.props.len() as u32, props_len as u32];
    for (i, v) in header.iter().enumerate() {
        out[start + i * 4..start + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
}

fn write_prop(out: &mut Vec<u8>, p: &Prop) {
    match p {
        Prop::Bool(v) => {
            out.push(b'C');
            out.push(if *v { b'T' } else { b'F' });
        }
        Prop::I16(v) => scalar(out, b'Y', &v.to_le_bytes()),
        Prop::I32(v) => scalar(out, b'I', &v.to_le_bytes()),
        Prop::I64(v) => scalar(out, b'L', &v.to_le_bytes()),
        Prop::F32(v) => scalar(out, b'F', &v.to_le_bytes()),
        Prop::F64(v) => scalar(out, b'D', &v.to_le_bytes()),
        Prop::Str(v) => bytes(out, b'S', v.as_bytes()),
        Prop::Raw(v) => bytes(out, b'R', v),
        Prop::I32s(v) => array(out, b'i', v.len(), v.iter().flat_map(|x| x.to_le_bytes())),
        Prop::I64s(v) => array(out, b'l', v.len(), v.iter().flat_map(|x| x.to_le_bytes())),
        Prop::F32s(v) => array(out, b'f', v.len(), v.iter().flat_map(|x| x.to_le_bytes())),
        Prop::F64s(v) => array(out, b'd', v.len(), v.iter().flat_map(|x| x.to_le_bytes())),
    }
}

fn scalar(out: &mut Vec<u8>, code: u8, data: &[u8]) {
    out.push(code);
    out.extend(data);
}

fn bytes(out: &mut Vec<u8>, code: u8, data: &[u8]) {
    out.push(code);
    out.extend((data.len() as u32).to_le_bytes());
    out.extend(data);
}

// TODO: 大きな配列は zlib 圧縮する（encoding = 1）
fn array(out: &mut Vec<u8>, code: u8, len: usize, data: impl Iterator<Item = u8>) {
    out.push(code);
    out.extend((len as u32).to_le_bytes());
    out.extend(0u32.to_le_bytes()); // encoding: 非圧縮
    let len_pos = out.len();
    out.extend(0u32.to_le_bytes());
    out.extend(data);
    let data_len = (out.len() - len_pos - 4) as u32;
    out[len_pos..len_pos + 4].copy_from_slice(&data_len.to_le_bytes());
}

fn write_footer(out: &mut Vec<u8>) {
    out.extend(FOOTER_ID);
    out.extend([0; 4]);
    let pad = match out.len() % 16 {
        0 => 16,
        r => 16 - r,
    };
    out.extend(std::iter::repeat_n(0, pad));
    out.extend(VERSION.to_le_bytes());
    out.extend([0; 120]);
    out.extend(FOOTER_MAGIC);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_node_layout() {
        let data = encode(&[Node::new("A").prop(1i32).child(leaf("B", "x"))]);
        assert!(data.starts_with(b"Kaydara FBX Binary  \x00\x1a\x00"));
        assert_eq!(&data[23..27], &7400u32.to_le_bytes());

        // A: EndOffset, NumProperties=1, PropertyListLen=5, 名前
        let a = &data[27..];
        let end = u32::from_le_bytes(a[0..4].try_into().unwrap()) as usize;
        assert_eq!(&a[4..8], &1u32.to_le_bytes());
        assert_eq!(&a[8..12], &5u32.to_le_bytes());
        assert_eq!(&a[12..14], b"\x01A");
        assert_eq!(&a[14..19], b"I\x01\x00\x00\x00");
        // A は子を持つので空レコードで終わる
        assert_eq!(&data[end - 13..end], &[0; 13]);
        // トップレベルの終わりの空レコードの後にフッター
        assert_eq!(&data[end + 13..end + 29], &FOOTER_ID);
        assert!(data.ends_with(&FOOTER_MAGIC));
    }

    #[test]
    fn encodes_arrays() {
        let mut out = Vec::new();
        write_prop(&mut out, &Prop::I32s(vec![1, 2]));
        assert_eq!(out[0], b'i');
        assert_eq!(&out[1..13], &[2, 0, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0]);
        assert_eq!(out.len(), 13 + 8);
    }
}
