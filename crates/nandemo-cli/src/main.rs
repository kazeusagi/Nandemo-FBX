use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use nandemo_core::{Model, Motion, bake};

/// MMD / VRM のモデル・モーションを FBX に変換します
#[derive(Parser)]
#[command(version)]
struct Args {
    /// 入力モデル（.pmx）。アニメーションだけを書き出すときもボーン構造に使います
    input: PathBuf,
    /// 入力モーション（.vmd）
    #[arg(short, long)]
    motion: Option<PathBuf>,
    /// 出力先（省略時は入力と同じ場所に .fbx）
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// 書き出す内容
    #[arg(short, long, value_enum, default_value_t = Export::All)]
    export: Export,
    /// ボーンアニメーションのキーを減らすときの、移動の許容誤差（mm）
    #[arg(long, default_value_t = 0.1)]
    tolerance_mm: f32,
    /// ボーンアニメーションのキーを減らすときの、回転の許容誤差（度）
    #[arg(long, default_value_t = 0.01)]
    tolerance_deg: f32,
    /// キーを減らさず、全フレームのキーを書き出す
    #[arg(long)]
    no_reduce: bool,
}

#[derive(Clone, Copy, PartialEq, ValueEnum)]
enum Export {
    /// メッシュ・マテリアル・ボーン・アニメーション（モーション指定時）
    All,
    /// メッシュ・マテリアル・ボーン（モーションは書き出さない）
    Model,
    /// ボーンとボーンアニメーションだけ（--motion が必要。モーフのアニメーションはメッシュがないので含まれません）
    Anim,
}

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("エラー: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Args) -> Result<()> {
    if args.export == Export::Anim && args.motion.is_none() {
        return Err("--export anim には --motion でモーションを指定してください".into());
    }

    let model = load_model(&args.input)?;
    let motion = match &args.motion {
        Some(_) if args.export == Export::Model => {
            eprintln!("警告: --export model なのでモーションは書き出しません");
            None
        }
        Some(path) => Some(load_motion(path)?),
        None => None,
    };
    if let Some(motion) = &motion {
        report_unmatched(&model, motion, args.export);
    }
    // FBX には IK・付与親がないので、計算したポーズを焼き込みます
    let motion = motion.map(|m| {
        if bake::needs_bake(&model) {
            bake::bake(&model, &m)
        } else {
            m
        }
    });

    let reduce = if args.no_reduce { 0.0 } else { 1.0 };
    let options = nandemo_fbx::Options {
        mesh: args.export != Export::Anim,
        translation_tolerance: args.tolerance_mm / 1000.0 * reduce,
        rotation_tolerance: args.tolerance_deg * reduce,
    };
    let output = args
        .output
        .unwrap_or_else(|| args.input.with_extension("fbx"));
    fs::write(
        &output,
        nandemo_fbx::write(&model, motion.as_ref(), &options),
    )?;
    println!("{} を書き出しました", output.display());
    Ok(())
}

/// モデルにないボーンのトラックなど、書き出されないものを知らせます
fn report_unmatched(model: &Model, motion: &Motion, export: Export) {
    let missing: Vec<&str> = motion
        .bones
        .iter()
        .map(|t| t.bone.as_str())
        .filter(|name| !model.bones.iter().any(|b| b.name == *name))
        .collect();
    if !missing.is_empty() {
        eprintln!(
            "警告: モデルにないボーン {} 本のモーションは無視しました: {}",
            missing.len(),
            missing.join(", ")
        );
    }

    let has_morph = |name: &str| {
        model
            .meshes
            .iter()
            .flat_map(|m| &m.morphs)
            .any(|m| m.name == name && !m.offsets.is_empty())
    };
    let missing: Vec<&str> = motion
        .morphs
        .iter()
        .map(|t| t.morph.as_str())
        .filter(|name| !has_morph(name))
        .collect();
    if !missing.is_empty() {
        eprintln!(
            "警告: モデルにない（またはボーン・UV・材質だけの）モーフ {} 個のモーションは無視しました: {}",
            missing.len(),
            missing.join(", ")
        );
    }
    if export == Export::Anim && !motion.morphs.is_empty() {
        eprintln!(
            "警告: --export anim ではメッシュがないので、モーフのアニメーションは書き出しません"
        );
    }
}

fn load_model(path: &Path) -> Result<Model> {
    match extension(path).as_str() {
        "pmx" => {
            let pmx = nandemo_pmx::parse(&read(path)?)?;
            Ok(nandemo_pmx::to_model(&pmx))
        }
        "vrm" => Err("VRM はまだ未対応です".into()),
        ext => Err(format!("モデル形式 .{ext} には対応していません").into()),
    }
}

fn load_motion(path: &Path) -> Result<Motion> {
    match extension(path).as_str() {
        "vmd" => {
            let vmd = nandemo_vmd::parse(&read(path)?)?;
            Ok(nandemo_vmd::to_motion(&vmd))
        }
        "vrma" => Err("VRMA はまだ未対応です".into()),
        ext => Err(format!("モーション形式 .{ext} には対応していません").into()),
    }
}

fn read(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).map_err(|e| format!("{} を読めません: {e}", path.display()).into())
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}
