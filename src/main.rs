mod diff;
mod json;
mod parser;
mod render;

use anyhow::{Context, Result};
use clap::Parser;
use owo_colors::{OwoColorize, Stream};
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
            // §6.1: stderr の error は赤。判定は stderr のみを見るため、stdout を
            // リダイレクトしても色は残る（AD-6）。
            eprintln!(
                "{} {e:#}",
                "error:".if_supports_color(Stream::Stderr, |t| t.red())
            );
            ExitCode::from(EXIT_ERROR)
        }
    }
}

/// 比較して出力する。戻り値は「差分があったか」（§3 の終了コードの判定基準）。
fn run(cli: &Cli) -> Result<bool> {
    let a = load(&cli.a)?;
    let b = load(&cli.b)?;

    // §4.7: 警告は stderr（人間が読む注意）へ。stdout（機械が読む結果）と分ける
    // ことで、パイプにも影響しない。警告は終了コードにも影響しない（§3）。
    // 色付けは Task 9（§6.1）で入れる。
    warn(&cli.a, &a.warnings);
    warn(&cli.b, &b.warnings);

    let diffs = diff::compare(&a.vars, &b.vars);
    let summary = diff::summarize(&diffs);

    // §2: --all（表示範囲）と --json（出力形式）は直交する。
    if cli.json {
        json::render(&diffs, &summary, &cli.a, &cli.b, cli.all);
    } else {
        render::render(&diffs, &summary, &cli.a, &cli.b, cli.all);
    }

    // §2: --all は表示範囲のオプションであり、終了コードの判定基準を変えない。
    // §6.2: --json も同じく終了コードの判定を変えない。
    Ok(summary.has_differences())
}

/// 重複キーの警告を stderr に出す（§4.7）。ラベルは黄（§6.1）。
fn warn(path: &Path, warnings: &[parser::Warning]) {
    for w in warnings {
        eprintln!(
            "{} {}:{}: duplicate key '{}' (overrides value from line {})",
            "warning:".if_supports_color(Stream::Stderr, |t| t.yellow()),
            path.display(),
            w.line,
            w.key,
            w.previous_line
        );
    }
}

/// ファイルを読んでパースする。エラーは `ファイル名:行番号: メッセージ` の形にする（§4）。
fn load(path: &Path) -> Result<parser::ParseOutput> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    parser::parse(&text).map_err(|e| anyhow::anyhow!("{}:{}", path.display(), e))
}
