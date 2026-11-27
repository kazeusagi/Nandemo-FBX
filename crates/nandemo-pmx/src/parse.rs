use nandemo_core::bin::{BinReader, ReadError, Result};

use crate::*;

/// PMX ファイルを読みます。
pub fn parse(data: &[u8]) -> Result<Pmx> {
    let mut r = BinReader::new(data);
    let header = read_header(&mut r)?;
    let mut p = PmxReader { r, header };

    let name = p.text()?;
    let name_en = p.text()?;
    let comment = p.text()?;
    let comment_en = p.text()?;

    let n = p.r.count(38)?;
    let vertices = (0..n).map(|_| p.vertex()).collect::<Result<_>>()?;

    let n = p.r.count(p.header.vertex_index_size as usize)?;
    if n % 3 != 0 {
        return Err(p
            .r
            .error(format!("インデックス数 {n} が 3 の倍数ではありません")));
    }
    let indices = (0..n).map(|_| p.vertex_index()).collect::<Result<_>>()?;

    let n = p.r.count(4)?;
    let textures = (0..n).map(|_| p.text()).collect::<Result<_>>()?;

    let n = p.r.count(4)?;
    let materials = (0..n).map(|_| p.material()).collect::<Result<_>>()?;

    let n = p.r.count(4)?;
    let bones = (0..n).map(|_| p.bone()).collect::<Result<_>>()?;

    let n = p.r.count(4)?;
    let morphs = (0..n).map(|_| p.morph()).collect::<Result<_>>()?;

    Ok(Pmx {
        header: p.header,
        name,
        name_en,
        comment,
        comment_en,
        vertices,
        indices,
        textures,
        materials,
        bones,
        morphs,
    })
}

fn read_header(r: &mut BinReader) -> Result<Header> {
    if r.bytes(4)? != b"PMX " {
        return Err(ReadError {
            offset: 0,
            message: "PMX ファイルではありません".into(),
        });
    }
    let version = r.f32()?;
    let globals_len = r.u8()? as usize;
    if globals_len < 8 {
        return Err(r.error(format!("ヘッダーの設定数 {globals_len} が足りません")));
    }
    let g = r.bytes(globals_len)?;

    for (i, &size) in g[2..8].iter().enumerate() {
        if ![1, 2, 4].contains(&size) {
            return Err(r.error(format!(
                "インデックスサイズ {size}（設定 {}）が不正です",
                i + 2
            )));
        }
    }
    if g[1] > 4 {
        return Err(r.error(format!("追加 UV 数 {} が不正です", g[1])));
    }

    Ok(Header {
        version,
        utf8: g[0] == 1,
        additional_uvs: g[1],
        vertex_index_size: g[2],
        texture_index_size: g[3],
        material_index_size: g[4],
        bone_index_size: g[5],
        morph_index_size: g[6],
        rigid_body_index_size: g[7],
    })
}

struct PmxReader<'a> {
    r: BinReader<'a>,
    header: Header,
}

impl PmxReader<'_> {
    fn text(&mut self) -> Result<String> {
        let len = self.r.count(1)?;
        let bytes = self.r.bytes(len)?;
        if self.header.utf8 {
            return Ok(String::from_utf8_lossy(bytes).into_owned());
        }
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        Ok(String::from_utf16_lossy(&units))
    }

    /// ボーン・テクスチャなどの番号（符号付き、`-1` でなし）
    fn index(&mut self, size: u8) -> Result<i32> {
        Ok(match size {
            1 => self.r.i8()? as i32,
            2 => self.r.i16()? as i32,
            _ => self.r.i32()?,
        })
    }

    /// 頂点番号（1・2 バイトのときは符号なし）
    fn vertex_index(&mut self) -> Result<u32> {
        Ok(match self.header.vertex_index_size {
            1 => self.r.u8()? as u32,
            2 => self.r.u16()? as u32,
            _ => self.r.i32()? as u32,
        })
    }

    fn bone_index(&mut self) -> Result<i32> {
        self.index(self.header.bone_index_size)
    }

    fn texture_index(&mut self) -> Result<i32> {
        self.index(self.header.texture_index_size)
    }

    fn vertex(&mut self) -> Result<Vertex> {
        let position = self.r.f32s()?;
        let normal = self.r.f32s()?;
        let uv = self.r.f32s()?;
        let additional_uvs = (0..self.header.additional_uvs)
            .map(|_| self.r.f32s())
            .collect::<Result<_>>()?;

        let weight = match self.r.u8()? {
            0 => Weight::Bdef1(self.bone_index()?),
            1 => Weight::Bdef2 {
                bones: [self.bone_index()?, self.bone_index()?],
                weight: self.r.f32()?,
            },
            2 => Weight::Bdef4 {
                bones: self.bone_indices4()?,
                weights: self.r.f32s()?,
            },
            3 => Weight::Sdef {
                bones: [self.bone_index()?, self.bone_index()?],
                weight: self.r.f32()?,
                c: self.r.f32s()?,
                r0: self.r.f32s()?,
                r1: self.r.f32s()?,
            },
            4 => Weight::Qdef {
                bones: self.bone_indices4()?,
                weights: self.r.f32s()?,
            },
            t => return Err(self.r.error(format!("ウェイト種別 {t} は不明です"))),
        };

        Ok(Vertex {
            position,
            normal,
            uv,
            additional_uvs,
            weight,
            edge_scale: self.r.f32()?,
        })
    }

    fn bone_indices4(&mut self) -> Result<[i32; 4]> {
        Ok([
            self.bone_index()?,
            self.bone_index()?,
            self.bone_index()?,
            self.bone_index()?,
        ])
    }

    fn material(&mut self) -> Result<Material> {
        let name = self.text()?;
        let name_en = self.text()?;
        let diffuse = self.r.f32s()?;
        let specular = self.r.f32s()?;
        let specular_power = self.r.f32()?;
        let ambient = self.r.f32s()?;
        let flags = self.r.u8()?;
        let edge_color = self.r.f32s()?;
        let edge_size = self.r.f32()?;
        let texture = self.texture_index()?;
        let sphere_texture = self.texture_index()?;
        let sphere_mode = self.r.u8()?;
        let toon = match self.r.u8()? {
            0 => Toon::Texture(self.texture_index()?),
            _ => Toon::Shared(self.r.u8()?),
        };
        let memo = self.text()?;
        let index_count = self.r.count(0)?;

        Ok(Material {
            name,
            name_en,
            diffuse,
            specular,
            specular_power,
            ambient,
            flags,
            edge_color,
            edge_size,
            texture,
            sphere_texture,
            sphere_mode,
            toon,
            memo,
            index_count,
        })
    }

    fn bone(&mut self) -> Result<Bone> {
        let name = self.text()?;
        let name_en = self.text()?;
        let position = self.r.f32s()?;
        let parent = self.bone_index()?;
        let layer = self.r.i32()?;
        let flags = self.r.u16()?;
        let has = |flag: u16| flags & flag != 0;

        let tail = if has(Bone::TAIL_IS_BONE) {
            BoneTail::Bone(self.bone_index()?)
        } else {
            BoneTail::Offset(self.r.f32s()?)
        };
        let inherit = if has(Bone::INHERIT_ROTATION) || has(Bone::INHERIT_TRANSLATION) {
            Some((self.bone_index()?, self.r.f32()?))
        } else {
            None
        };
        let fixed_axis = if has(Bone::FIXED_AXIS) {
            Some(self.r.f32s()?)
        } else {
            None
        };
        let local_axes = if has(Bone::LOCAL_AXES) {
            Some((self.r.f32s()?, self.r.f32s()?))
        } else {
            None
        };
        let external_parent = if has(Bone::EXTERNAL_PARENT) {
            Some(self.r.i32()?)
        } else {
            None
        };
        let ik = if has(Bone::IK) {
            Some(self.ik()?)
        } else {
            None
        };

        Ok(Bone {
            name,
            name_en,
            position,
            parent,
            layer,
            flags,
            tail,
            inherit,
            fixed_axis,
            local_axes,
            external_parent,
            ik,
        })
    }

    fn ik(&mut self) -> Result<Ik> {
        let target = self.bone_index()?;
        let loops = self.r.i32()?;
        let limit_angle = self.r.f32()?;
        let n = self.r.count(2)?;
        let mut links = Vec::with_capacity(n);
        for _ in 0..n {
            let bone = self.bone_index()?;
            let limits = if self.r.u8()? != 0 {
                Some((self.r.f32s()?, self.r.f32s()?))
            } else {
                None
            };
            links.push(IkLink { bone, limits });
        }
        Ok(Ik {
            target,
            loops,
            limit_angle,
            links,
        })
    }

    fn morph(&mut self) -> Result<Morph> {
        let name = self.text()?;
        let name_en = self.text()?;
        let panel = self.r.u8()?;
        let kind_id = self.r.u8()?;
        let n = self.r.count(1)?;
        let morph_size = self.header.morph_index_size;

        let kind = match kind_id {
            0 => MorphKind::Group(
                (0..n)
                    .map(|_| Ok((self.index(morph_size)?, self.r.f32()?)))
                    .collect::<Result<_>>()?,
            ),
            1 => MorphKind::Vertex(
                (0..n)
                    .map(|_| {
                        Ok(VertexOffset {
                            vertex: self.vertex_index()?,
                            offset: self.r.f32s()?,
                        })
                    })
                    .collect::<Result<_>>()?,
            ),
            2 => MorphKind::Bone(
                (0..n)
                    .map(|_| {
                        Ok(BoneOffset {
                            bone: self.bone_index()?,
                            translation: self.r.f32s()?,
                            rotation: self.r.f32s()?,
                        })
                    })
                    .collect::<Result<_>>()?,
            ),
            _ => {
                // 1 オフセットあたりのバイト数
                let h = &self.header;
                let size = match kind_id {
                    3..=7 => h.vertex_index_size as usize + 16,  // UV
                    8 => h.material_index_size as usize + 113,   // 材質
                    9 => morph_size as usize + 4,                // フリップ
                    10 => h.rigid_body_index_size as usize + 25, // インパルス
                    t => return Err(self.r.error(format!("モーフ種別 {t} は不明です"))),
                };
                self.r.skip(n * size)?;
                MorphKind::Unsupported(kind_id)
            }
        };

        Ok(Morph {
            name,
            name_en,
            panel,
            kind,
        })
    }
}
