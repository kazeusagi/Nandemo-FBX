//! IK と付与親を解いて、毎フレームのボーンのキーに焼き込みます。
//!
//! FBX には MMD の IK・付与親にあたる仕組みがないので、計算済みのポーズを書き出します。
//! 計算の順序は three.js の MMD 実装と同じです。
//!
//! 1. 各ボーンにアニメーションの値を設定する
//! 2. IK を変形順に CCD 法で解く
//! 3. 付与親の回転・移動を変形順に加える

use crate::math::{
    add, axis_angle, conj, cross, dot, from_euler_xyz, length, normalize, normalize3, quat_mul,
    rotate, scale, slerp, sub, to_euler_xyz,
};
use crate::{
    Bezier, Bone, BoneKey, BoneTrack, Ik, Inherit, Model, Motion, QUAT_IDENTITY, Quat, Vec3,
    safe_parents,
};

/// IK か付与親を持つボーンがあるか
pub fn needs_bake(model: &Model) -> bool {
    model
        .bones
        .iter()
        .any(|b| b.ik.is_some() || b.inherit.is_some())
}

/// IK・付与親を解いたモーションを返します。
///
/// 結果は全フレームにキーがあり、補間は線形です。
/// レストポーズから動かないボーンのトラックは含めません。IK の切り替えは焼き込み済みなので空になります。
pub fn bake(model: &Model, motion: &Motion) -> Motion {
    let bones = &model.bones;
    let find = |name: &str| motion.bones.iter().find(|t| t.bone == name);
    let tracks: Vec<Option<&BoneTrack>> = bones.iter().map(|b| find(&b.name)).collect();
    let ik_tracks: Vec<_> = bones
        .iter()
        .map(|b| motion.ik.iter().find(|t| t.bone == b.name))
        .collect();

    let mut pose = Pose::new(bones);
    let mut keys: Vec<Vec<BoneKey>> = vec![Vec::new(); bones.len()];

    for frame in 0..=motion.last_frame() {
        for (i, b) in bones.iter().enumerate() {
            let (dt, dr) = tracks[i].map_or(([0.0; 3], QUAT_IDENTITY), |t| t.sample(frame as f32));
            pose.local_t[i] = add(b.translation, dt);
            pose.local_r[i] = normalize(quat_mul(b.rotation, dr));
        }
        pose.update_all();

        for &i in &pose.order.clone() {
            let enabled = ik_tracks[i].is_none_or(|t| t.enabled(frame));
            if let (Some(ik), true) = (&bones[i].ik, enabled) {
                pose.solve_ik(i, ik);
                pose.update_all();
            }
        }

        for &i in &pose.order.clone() {
            if let Some(inherit) = &bones[i].inherit {
                pose.apply_inherit(i, inherit);
            }
        }

        for (i, b) in bones.iter().enumerate() {
            keys[i].push(BoneKey {
                frame,
                translation: sub(pose.local_t[i], b.translation),
                rotation: quat_mul(conj(b.rotation), pose.local_r[i]),
                interpolation: [Bezier::LINEAR; 4],
            });
        }
    }

    let baked = bones
        .iter()
        .zip(keys)
        .enumerate()
        .filter(|(i, (_, keys))| tracks[*i].is_some() || keys.iter().any(moved))
        .map(|(_, (b, keys))| BoneTrack {
            bone: b.name.clone(),
            keys,
        })
        .collect();

    Motion {
        name: motion.name.clone(),
        fps: motion.fps,
        bones: baked,
        morphs: motion.morphs.clone(),
        ik: Vec::new(),
    }
}

/// レストポーズから動いているか
fn moved(k: &BoneKey) -> bool {
    let [x, y, z, _] = k.rotation;
    k.translation.iter().any(|v| v.abs() > 1e-6) || x.abs().max(y.abs()).max(z.abs()) > 1e-6
}

/// 1 フレーム分のポーズ
struct Pose<'a> {
    bones: &'a [Bone],
    parents: Vec<Option<usize>>,
    children: Vec<Vec<usize>>,
    /// 親が子より先に来る順
    hierarchy: Vec<usize>,
    /// 変形順（変形階層 → ボーン番号）
    order: Vec<usize>,
    local_t: Vec<Vec3>,
    local_r: Vec<Quat>,
    global_t: Vec<Vec3>,
    global_r: Vec<Quat>,
}

impl<'a> Pose<'a> {
    fn new(bones: &'a [Bone]) -> Self {
        let n = bones.len();
        let parents = safe_parents(bones);
        let mut children = vec![Vec::new(); n];
        for (i, p) in parents.iter().enumerate() {
            if let Some(p) = *p {
                children[p].push(i);
            }
        }
        let depth = |mut i: usize| {
            let mut d = 0;
            while let Some(p) = parents[i] {
                (i, d) = (p, d + 1);
            }
            d
        };
        let mut hierarchy: Vec<usize> = (0..n).collect();
        hierarchy.sort_by_key(|&i| depth(i));
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by_key(|&i| bones[i].layer);

        Self {
            bones,
            parents,
            children,
            hierarchy,
            order,
            local_t: vec![[0.0; 3]; n],
            local_r: vec![QUAT_IDENTITY; n],
            global_t: vec![[0.0; 3]; n],
            global_r: vec![QUAT_IDENTITY; n],
        }
    }

    fn update(&mut self, i: usize) {
        (self.global_t[i], self.global_r[i]) = match self.parents[i] {
            Some(p) => (
                add(self.global_t[p], rotate(self.global_r[p], self.local_t[i])),
                quat_mul(self.global_r[p], self.local_r[i]),
            ),
            None => (self.local_t[i], self.local_r[i]),
        };
    }

    fn update_all(&mut self) {
        for k in 0..self.hierarchy.len() {
            self.update(self.hierarchy[k]);
        }
    }

    /// `i` とその子孫のグローバル値を更新します
    fn update_subtree(&mut self, i: usize) {
        let mut stack = vec![i];
        while let Some(j) = stack.pop() {
            self.update(j);
            stack.extend_from_slice(&self.children[j]);
        }
    }

    /// CCD 法: 先端に近いリンクから順に、先端がゴールの方を向くよう回すのを繰り返します。
    ///
    /// ひざのように 1 軸だけ動けるリンクは、その軸まわりの角度を積み上げて解きます（MMD 互換の実装でよく使われる方式）。
    /// 回す向きはゴールに近づく方を選び、1 回目で範囲外なら逆向きにするので、
    /// まっすぐな脚でも曲がる向きが決まります。
    fn solve_ik(&mut self, ik_bone: usize, ik: &Ik) {
        let goal = self.global_t[ik_bone];
        let effector = ik.target;
        let mut plane_angles = vec![0.0f32; ik.links.len()];
        let mut best = (self.distance(effector, goal), self.link_rotations(ik));

        for iteration in 0..ik.iterations {
            for (n, link) in ik.links.iter().enumerate() {
                let l = link.bone;
                if l == effector {
                    continue;
                }
                // リンクのローカル空間で、先端とゴールへの向き
                let inv = conj(self.global_r[l]);
                let dir = |p: Vec3| normalize3(rotate(inv, sub(p, self.global_t[l])));
                let (Some(from), Some(to)) = (dir(self.global_t[effector]), dir(goal)) else {
                    continue;
                };
                let mut angle = dot(from, to).clamp(-1.0, 1.0).acos();
                if ik.limit_angle > 0.0 {
                    angle = angle.min(ik.limit_angle);
                }
                let rest = self.bones[l].rotation;

                let single = link
                    .limits
                    .and_then(|(min, max)| Some((min, max, single_axis(min, max)?)));
                let r = match (single, link.limits) {
                    (Some((min, max, k)), _) => {
                        let mut axis = [0.0; 3];
                        axis[k] = 1.0;
                        // ± どちらに回すと先端がゴールの向きに近づくか
                        let gain = |a: f32| dot(rotate(axis_angle(axis, a), from), to);
                        let step = if gain(angle) >= gain(-angle) {
                            angle
                        } else {
                            -angle
                        };
                        let mut a = plane_angles[n] + step;
                        let range = min[k]..=max[k];
                        if iteration == 0 && !range.contains(&a) {
                            let mid = (min[k] + max[k]) / 2.0;
                            if range.contains(&-a) || (mid - a).abs() > (mid + a).abs() {
                                a = -a;
                            }
                        }
                        a = a.clamp(min[k], max[k]);
                        plane_angles[n] = a;
                        quat_mul(rest, axis_angle(axis, a))
                    }
                    (None, limits) => {
                        let Some(axis) = normalize3(cross(from, to)).filter(|_| angle > 1e-5)
                        else {
                            continue;
                        };
                        let r = normalize(quat_mul(self.local_r[l], axis_angle(axis, angle)));
                        match limits {
                            Some((min, max)) => {
                                quat_mul(rest, clamp_euler(quat_mul(conj(rest), r), min, max))
                            }
                            None => r,
                        }
                    }
                };
                self.local_r[l] = r;
                self.update_subtree(l);
            }

            // 良くならなくなったら、一番良かった状態に戻して終わります
            let d = self.distance(effector, goal);
            if d < best.0 {
                best = (d, self.link_rotations(ik));
            } else {
                for (link, r) in ik.links.iter().zip(&best.1) {
                    self.local_r[link.bone] = *r;
                }
                self.update_all();
                break;
            }
            if d < 1e-6 {
                break;
            }
        }

        if self.distance(effector, goal) > 1e-4 {
            self.refine_two_link(ik, goal);
        }
    }

    /// CCD 法は、脚がほぼまっすぐでゴールが少しだけ近いとき、ひざがほとんど曲がりません。
    /// ひざ（1 軸）＋太ももの 2 リンクなら、ひざの角度を距離から求めて補正します。
    fn refine_two_link(&mut self, ik: &Ik, goal: Vec3) {
        let [knee, thigh] = &ik.links[..] else {
            return;
        };
        let Some((min, max)) = knee.limits else {
            return;
        };
        let Some(k) = single_axis(min, max) else {
            return;
        };
        // 範囲が 0 をまたぐと曲げる向きが決まらないので補正しません
        if min[k] < 0.0 && max[k] > 0.0 {
            return;
        }
        let (effector, rest) = (ik.target, self.bones[knee.bone].rotation);
        let mut axis = [0.0; 3];
        axis[k] = 1.0;
        let want = length(sub(goal, self.global_t[thigh.bone]));

        // まっすぐに近い側（0 に近い端）から折りたたむ側へ、太ももから先端までの距離は単調に縮みます
        let (mut straight, mut folded) = if min[k] >= 0.0 {
            (min[k], max[k])
        } else {
            (max[k], min[k])
        };
        for _ in 0..32 {
            let mid = (straight + folded) / 2.0;
            self.local_r[knee.bone] = quat_mul(rest, axis_angle(axis, mid));
            self.update_subtree(knee.bone);
            if length(sub(self.global_t[effector], self.global_t[thigh.bone])) > want {
                straight = mid;
            } else {
                folded = mid;
            }
        }
        self.local_r[knee.bone] = quat_mul(rest, axis_angle(axis, (straight + folded) / 2.0));
        self.update_subtree(knee.bone);

        // 太ももを最小の回転でゴールに向けます
        let l = thigh.bone;
        let inv = conj(self.global_r[l]);
        let dir = |p: Vec3| normalize3(rotate(inv, sub(p, self.global_t[l])));
        let (Some(from), Some(to)) = (dir(self.global_t[effector]), dir(goal)) else {
            return;
        };
        if let Some(axis) = normalize3(cross(from, to)) {
            let angle = dot(from, to).clamp(-1.0, 1.0).acos();
            let mut r = normalize(quat_mul(self.local_r[l], axis_angle(axis, angle)));
            if let Some((min, max)) = thigh.limits {
                let rest = self.bones[l].rotation;
                r = quat_mul(rest, clamp_euler(quat_mul(conj(rest), r), min, max));
            }
            self.local_r[l] = r;
            self.update_subtree(l);
        }
    }

    fn distance(&self, bone: usize, goal: Vec3) -> f32 {
        length(sub(self.global_t[bone], goal))
    }

    fn link_rotations(&self, ik: &Ik) -> Vec<Quat> {
        ik.links.iter().map(|l| self.local_r[l.bone]).collect()
    }

    /// 付与親の（レストポーズからの）回転・移動を `weight` の割合で加えます
    fn apply_inherit(&mut self, i: usize, inherit: &Inherit) {
        let p = inherit.bone;
        let parent = &self.bones[p];
        if inherit.rotation {
            let q = quat_mul(conj(parent.rotation), self.local_r[p]);
            let q = slerp(QUAT_IDENTITY, q, inherit.weight);
            self.local_r[i] = normalize(quat_mul(self.local_r[i], q));
        }
        if inherit.translation {
            let t = sub(self.local_t[p], parent.translation);
            self.local_t[i] = add(self.local_t[i], scale(t, inherit.weight));
        }
    }
}

/// 1 軸だけ動ける制限（ひざなど）なら、その軸の番号
fn single_axis(min: Vec3, max: Vec3) -> Option<usize> {
    let free: Vec<usize> = (0..3)
        .filter(|&k| min[k].abs() > 1e-6 || max[k].abs() > 1e-6)
        .collect();
    match free[..] {
        [k] => Some(k),
        _ => None,
    }
}

/// 回転をオイラー角 XYZ の範囲に収めます
fn clamp_euler(q: Quat, min: Vec3, max: Vec3) -> Quat {
    let e = to_euler_xyz(q);
    from_euler_xyz(std::array::from_fn(|k| e[k].clamp(min[k], max[k])))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IkKey, IkLink, IkTrack};

    fn close(a: &[f32], b: &[f32], eps: f32) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < eps)
    }

    fn key(translation: Vec3, rotation: Quat) -> BoneKey {
        BoneKey {
            frame: 0,
            translation,
            rotation,
            interpolation: [Bezier::LINEAR; 4],
        }
    }

    fn track(bone: &str, translation: Vec3, rotation: Quat) -> BoneTrack {
        BoneTrack {
            bone: bone.into(),
            keys: vec![key(translation, rotation)],
        }
    }

    fn motion(bones: Vec<BoneTrack>) -> Motion {
        Motion {
            fps: 30.0,
            bones,
            ..Default::default()
        }
    }

    /// 焼き込んだモーションのフレーム 0 でのグローバル位置
    fn positions(model: &Model, baked: &Motion) -> Vec<Vec3> {
        let mut pose = Pose::new(&model.bones);
        for (i, b) in model.bones.iter().enumerate() {
            let (dt, dr) = baked
                .bones
                .iter()
                .find(|t| t.bone == b.name)
                .map_or(([0.0; 3], QUAT_IDENTITY), |t| t.sample(0.0));
            pose.local_t[i] = add(b.translation, dt);
            pose.local_r[i] = quat_mul(b.rotation, dr);
        }
        pose.update_all();
        pose.global_t
    }

    #[test]
    fn inherits_rotation_and_translation() {
        let mut b = Bone::new("b", None, [0.0; 3], QUAT_IDENTITY);
        b.inherit = Some(Inherit {
            bone: 0,
            weight: 0.5,
            rotation: true,
            translation: true,
        });
        let model = Model {
            bones: vec![Bone::new("a", None, [0.0; 3], QUAT_IDENTITY), b],
            ..Default::default()
        };
        let z90 = axis_angle([0.0, 0.0, 1.0], std::f32::consts::FRAC_PI_2);
        let baked = bake(&model, &motion(vec![track("a", [2.0, 0.0, 0.0], z90)]));

        let b = &baked.bones.iter().find(|t| t.bone == "b").unwrap().keys[0];
        let z45 = axis_angle([0.0, 0.0, 1.0], std::f32::consts::FRAC_PI_4);
        assert!(close(&b.rotation, &z45, 1e-5), "{:?}", b.rotation);
        assert!(close(&b.translation, &[1.0, 0.0, 0.0], 1e-5));
    }

    /// 太もも（y = 2）→ ひざ（y = 1）→ 足首（y = 0）と、足首を動かす IK ボーン
    fn leg(knee_limit: bool) -> Model {
        let knee = Bone::new("knee", Some(1), [0.0, -1.0, 0.0], QUAT_IDENTITY);
        let mut ik = Bone::new("ik", Some(0), [0.0; 3], QUAT_IDENTITY);
        ik.ik = Some(Ik {
            target: 3,
            iterations: 40,
            limit_angle: 2.0,
            links: vec![
                IkLink {
                    bone: 2,
                    limits: knee_limit.then_some((
                        [0.5f32.to_radians(), 0.0, 0.0],
                        [180f32.to_radians(), 0.0, 0.0],
                    )),
                },
                IkLink {
                    bone: 1,
                    limits: None,
                },
            ],
        });
        Model {
            bones: vec![
                Bone::new("root", None, [0.0; 3], QUAT_IDENTITY),
                Bone::new("thigh", Some(0), [0.0, 2.0, 0.0], QUAT_IDENTITY),
                knee,
                Bone::new("ankle", Some(2), [0.0, -1.0, 0.0], QUAT_IDENTITY),
                ik,
            ],
            ..Default::default()
        }
    }

    #[test]
    fn ik_moves_effector_to_goal() {
        let model = leg(true);
        let goal = [0.0, 0.5, 0.3];
        let baked = bake(&model, &motion(vec![track("ik", goal, QUAT_IDENTITY)]));

        let p = positions(&model, &baked);
        assert!(close(&p[3], &goal, 1e-3), "足首 {:?}", p[3]);

        // ひざは X 軸まわりにだけ、制限の向き（正）に曲がる
        let knee = baked.bones.iter().find(|t| t.bone == "knee").unwrap().keys[0].rotation;
        assert!(
            knee[0] > 0.1 && knee[1].abs() < 1e-5 && knee[2].abs() < 1e-5,
            "{knee:?}"
        );
    }

    #[test]
    fn ik_without_limits_reaches_goal() {
        let model = leg(false);
        let goal = [0.4, 0.8, -0.2];
        let baked = bake(&model, &motion(vec![track("ik", goal, QUAT_IDENTITY)]));
        assert!(close(&positions(&model, &baked)[3], &goal, 1e-3));
    }

    #[test]
    fn disabled_ik_is_skipped() {
        let model = leg(true);
        let mut m = motion(vec![track("ik", [0.0, 0.5, 0.3], QUAT_IDENTITY)]);
        m.ik = vec![IkTrack {
            bone: "ik".into(),
            keys: vec![IkKey {
                frame: 0,
                enabled: false,
            }],
        }];
        let baked = bake(&model, &m);
        assert!(!baked.bones.iter().any(|t| t.bone == "knee"));
        assert!(baked.ik.is_empty());
    }

    /// まっすぐな脚からゴールを少しずつ前上方へ動かしても、毎フレーム届く
    #[test]
    fn ik_bends_straight_leg() {
        let model = leg(true);
        for i in 1..=30 {
            let t = i as f32 / 30.0;
            let goal = [0.0, 0.6 * t, 0.4 * t];
            let baked = bake(&model, &motion(vec![track("ik", goal, QUAT_IDENTITY)]));
            let p = positions(&model, &baked);
            assert!(close(&p[3], &goal, 1e-3), "t = {t}: 足首 {:?}", p[3]);
        }
    }

    /// 長さ 1 でない回転のキーがあっても、脚の長さは変わらない
    #[test]
    fn normalizes_rotations() {
        let model = leg(true);
        let goal = [0.0, 0.5, 0.3];
        let scaled = [0.0, 0.0, 0.0, 0.5];
        let baked = bake(
            &model,
            &motion(vec![
                track("ik", goal, QUAT_IDENTITY),
                track("thigh", [0.0; 3], scaled),
            ]),
        );
        assert!(close(&positions(&model, &baked)[3], &goal, 1e-3));
    }

    #[test]
    fn clamps_multi_axis_limits() {
        let q = from_euler_xyz([0.5, -0.5, 0.2]);
        let e = to_euler_xyz(clamp_euler(q, [0.0, -0.1, -1.0], [0.3, 0.1, 1.0]));
        assert!(close(&e, &[0.3, -0.1, 0.2], 1e-4), "{e:?}");
    }
}
