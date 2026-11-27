use nandemo_core::math::{add, normalize, quat_mul};
use nandemo_core::{Model, Motion};

use crate::binary::leaf;
use crate::skeleton::Skeleton;
use crate::{CM_PER_METER, Document, matrix, object, p_number, p_time, props};

/// FBX の時間単位（1 秒あたり）
const KTIME_PER_SECOND: f64 = 46_186_158_000.0;

/// 線形補間（eInterpolationLinear | eTangentAuto）
const KEY_FLAGS_LINEAR: i32 = 0x0104;

/// ボーンアニメーションを書き出します。
///
/// ベジェ補間をそのまま FBX に移すのは難しいので、毎フレームの値を焼き込みます。
/// 回転はオイラー角にするため、前のフレームに近い角度を選んで飛びを防ぎます。
// TODO: IK・付与親の焼き込み、モーフアニメーション、キーの間引き
pub(crate) fn write(doc: &mut Document, model: &Model, motion: &Motion, skeleton: &Skeleton) {
    let fps = if motion.fps > 0.0 { motion.fps } else { 30.0 } as f64;
    let last = motion.last_frame();
    let times: Vec<i64> = (0..=last)
        .map(|f| (f as f64 * KTIME_PER_SECOND / fps).round() as i64)
        .collect();

    let (stack, layer) = (doc.id(), doc.id());
    let name = if motion.name.is_empty() {
        "Take 001"
    } else {
        &motion.name
    };
    doc.add(
        object("AnimationStack", "AnimStack", stack, name, "").child(props([
            p_time("LocalStart", 0),
            p_time("LocalStop", times[times.len() - 1]),
            p_time("ReferenceStart", 0),
            p_time("ReferenceStop", times[times.len() - 1]),
        ])),
    );
    doc.add(object(
        "AnimationLayer",
        "AnimLayer",
        layer,
        "BaseLayer",
        "",
    ));
    doc.connect(layer, stack);

    for (i, bone) in model.bones.iter().enumerate() {
        let Some(track) = motion.bones.iter().find(|t| t.bone == bone.name) else {
            continue;
        };

        let mut translation: [Vec<f32>; 3] = Default::default();
        let mut rotation: [Vec<f32>; 3] = Default::default();
        let mut prev = None;
        for f in 0..=last {
            let (dt, dr) = track.sample(f as f32);
            let t = add(bone.translation, dt);
            let e = matrix::euler_xyz(normalize(quat_mul(bone.rotation, dr)), prev);
            prev = Some(e);
            for c in 0..3 {
                translation[c].push((t[c] as f64 * CM_PER_METER) as f32);
                rotation[c].push(e[c] as f32);
            }
        }

        let target = skeleton.ids[i];
        curve_node(
            doc,
            layer,
            target,
            "T",
            "Lcl Translation",
            &times,
            translation,
        );
        curve_node(doc, layer, target, "R", "Lcl Rotation", &times, rotation);
    }
}

/// 3 成分のカーブノードと、成分ごとのカーブを書き出します。
fn curve_node(
    doc: &mut Document,
    layer: i64,
    target: i64,
    name: &str,
    prop: &str,
    times: &[i64],
    values: [Vec<f32>; 3],
) {
    const AXES: [&str; 3] = ["d|X", "d|Y", "d|Z"];

    let node = doc.id();
    doc.add(
        object("AnimationCurveNode", "AnimCurveNode", node, name, "").child(props(
            (0..3).map(|c| p_number(AXES[c], values[c][0] as f64)),
        )),
    );
    doc.connect(node, layer);
    doc.connect_prop(node, target, prop);

    for (axis, values) in AXES.iter().zip(values) {
        // 値が変化しないカーブはキー 1 つにします
        let (times, values) = if values.iter().all(|&v| v == values[0]) {
            (vec![times[0]], vec![values[0]])
        } else {
            (times.to_vec(), values)
        };
        let count = times.len() as i32;

        let curve = doc.id();
        doc.add(
            object("AnimationCurve", "AnimCurve", curve, "", "")
                .child(leaf("Default", values[0] as f64))
                .child(leaf("KeyVer", 4009))
                .child(leaf("KeyTime", times))
                .child(leaf("KeyValueFloat", values))
                .child(leaf("KeyAttrFlags", vec![KEY_FLAGS_LINEAR]))
                // Blender と同じ既定値（線形補間では使われません）
                .child(leaf(
                    "KeyAttrDataFloat",
                    vec![0.0f32, 0.0, 9.419_963e-30, 0.0],
                ))
                .child(leaf("KeyAttrRefCount", vec![count])),
        );
        doc.connect_prop(curve, node, axis);
    }
}
