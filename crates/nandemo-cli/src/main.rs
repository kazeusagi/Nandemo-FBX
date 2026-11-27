use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use nandemo_core::{Model, Motion};

/// MMD / VRM のモデル・モーションを FBX に変換します
#[derive(Parser)]
#[command(version)]
struct Args {
    /// 入力モデル（.pmx）
    input: PathBuf,
    /// 入力モーション（.vmd）
    #[arg(short, long)]
    motion: Option<PathBuf>,
    /// 出力先（省略時は入力と同じ場所に .fbx）
    #[arg(short, long)]
    output: Option<PathBuf>,
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
    let model = load_model(&args.input)?;

    if let Some(path) = &args.motion {
        let motion = load_motion(path)?;
        // TODO: アニメーションの書き出し
        eprintln!(
            "警告: モーションの書き出しはまだ未対応です（ボーン {} 本・モーフ {} 個分を読み込みました）",
            motion.bones.len(),
            motion.morphs.len()
        );
    }

    let output = args
        .output
        .unwrap_or_else(|| args.input.with_extension("fbx"));
    fs::write(&output, nandemo_fbx::write_model(&model))?;
    println!("{} を書き出しました", output.display());
    Ok(())
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
