use nandemo_core::math::{add, normalize, quat_mul};
use nandemo_core::{Model, Motion};

use crate::binary::leaf;
use crate::mesh::Channel;
use crate::reduce::reduce;
use crate::skeleton::Skeleton;
use crate::{CM_PER_METER, Document, Options, matrix, object, p_number, p_time, props};

/// FBX の時間単位（1 秒あたり）
const KTIME_PER_SECOND: f64 = 46_186_158_000.0;

/// 線形補間（eInterpolationLinear | eTangentAuto）
const KEY_FLAGS_LINEAR: i32 = 0x0104;

/// ボーンとモーフのアニメーションを書き出します。
///
/// ボーンはベジェ補間をそのまま FBX に移すのが難しいので、毎フレームの値を焼き込みます。
/// 回転はオイラー角にするため、前のフレームに近い角度を選んで飛びを防ぎます。
/// 焼き込んだキーは、許容誤差の範囲で減らします（[`reduce`]）。
/// モーフは VMD でも線形補間なので、キーをそのまま書き出します。
pub(crate) fn write(
    doc: &mut Document,
    model: &Model,
    motion: &Motion,
    skeleton: &Skeleton,
    channels: &[Channel],
    options: &Options,
) {
    let fps = if motion.fps > 0.0 { motion.fps } else { 30.0 } as f64;
    let time = |frame: u32| (frame as f64 * KTIME_PER_SECOND / fps).round() as i64;
    let last = motion.last_frame();
    let times: Vec<i64> = (0..=last).map(time).collect();

    let (stack, layer) = (doc.id(), doc.id());
    let name = if motion.name.is_empty() {
        "Take 001"
    } else {
        &motion.name
    };
    doc.add(
        object("AnimationStack", "AnimStack", stack, name, "").child(props([
            p_time("LocalStart", 0),
            p_time("LocalStop", time(last)),
            p_time("ReferenceStart", 0),
            p_time("ReferenceStop", time(last)),
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
        let [tx, ty, tz] = translation;
        let [rx, ry, rz] = rotation;
        let curves = |x, y, z| vec![("d|X", x), ("d|Y", y), ("d|Z", z)];
        let t_tol = (options.translation_tolerance as f64 * CM_PER_METER) as f32;
        let r_tol = options.rotation_tolerance;
        let (t, r) = (curves(tx, ty, tz), curves(rx, ry, rz));
        curve_node(doc, layer, target, "T", "Lcl Translation", &times, t, t_tol);
        curve_node(doc, layer, target, "R", "Lcl Rotation", &times, r, r_tol);
    }

    for channel in channels {
        let Some(track) = motion.morphs.iter().find(|t| t.morph == channel.morph) else {
            continue;
        };
        if track.keys.is_empty() {
            continue;
        }
        // FBX のブレンドシェイプの重みは 0〜100
        let times: Vec<i64> = track.keys.iter().map(|k| time(k.frame)).collect();
        let values = track.keys.iter().map(|k| k.weight * 100.0).collect();
        curve_node(
            doc,
            layer,
            channel.id,
            "DeformPercent",
            "DeformPercent",
            &times,
            vec![("d|DeformPercent", values)],
            0.0,
        );
    }
}

/// カーブノードと、成分ごとのカーブを書き出します。キーは `tolerance` の範囲で減らします。
#[allow(clippy::too_many_arguments)]
fn curve_node(
    doc: &mut Document,
    layer: i64,
    target: i64,
    name: &str,
    prop: &str,
    times: &[i64],
    curves: Vec<(&str, Vec<f32>)>,
    tolerance: f32,
) {
    let node = doc.id();
    doc.add(
        object("AnimationCurveNode", "AnimCurveNode", node, name, "").child(props(
            curves
                .iter()
                .map(|(axis, values)| p_number(axis, values[0] as f64)),
        )),
    );
    doc.connect(node, layer);
    doc.connect_prop(node, target, prop);

    for (axis, values) in curves {
        // 値が変化しないカーブはキー 1 つにします
        let (times, values) = if values.iter().all(|&v| v == values[0]) {
            (vec![times[0]], vec![values[0]])
        } else {
            reduce(times, &values, tolerance)
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
