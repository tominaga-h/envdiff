mod diff;
mod json;
mod parser;
mod render;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// 終了コード（§3）。`diff(1)` の慣習に従う。
///
/// エラーを 1 ではなく 2 に割り当てるのは、「差分を見つけた」と「読めなかった」を
/// 呼び出し側が区別できるようにするため（§3）。
const EXIT_NO_DIFF: u8 = 0;
const EXIT_DIFF: u8 = 1;
const EXIT_ERROR: u8 = 2;

/// `.env` 系ファイル 2 つを環境変数ごとに比較する。
#[derive(Parser, Debug)]
#[command(
    name = "envdiff",
    about = "Compare two .env files variable by variable",
    version
)]
struct Cli {
    /// 比較対象ファイル 1（出力の A 列）
    a: PathBuf,

    /// 比較対象ファイル 2（出力の B 列）
    b: PathBuf,

    /// 差分のない変数（same）も出力に含める
    #[arg(long)]
    all: bool,

    /// JSON 形式で出力する
    #[arg(long)]
    json: bool,
}

fn main() -> ExitCode {
    // 引数エラーは clap が usage を stderr に出して exit 2 で終える（§2）。
    let cli = Cli::parse();

    match run(&cli) {
        Ok(has_differences) => {
            if has_differences {
                ExitCode::from(EXIT_DIFF)
            } else {
                ExitCode::from(EXIT_NO_DIFF)
            }
        }
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(EXIT_ERROR)
        }
    }
}

/// 比較して出力する。戻り値は「差分があったか」（§3 の終了コードの判定基準）。
fn run(cli: &Cli) -> Result<bool> {
    let a = load(&cli.a)?;
    let b = load(&cli.b)?;

    for w in a.warnings.iter().chain(&b.warnings) {
        let _ = w; // 警告の出力は Task 4（§4.7）で入れる。
    }

    let diffs = diff::compare(&a.vars, &b.vars);
    let summary = diff::summarize(&diffs);

    // JSON 出力は Task 8（§6.2）で入れる。
    render::render(&diffs, &summary, &cli.a, &cli.b, cli.all);

    // §2: --all は表示範囲のオプションであり、終了コードの判定基準を変えない。
    Ok(summary.has_differences())
}

/// ファイルを読んでパースする。エラーは `ファイル名:行番号: メッセージ` の形にする（§4）。
fn load(path: &Path) -> Result<parser::ParseOutput> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    parser::parse(&text).map_err(|e| anyhow::anyhow!("{}:{}", path.display(), e))
}
