//! CLI 統合テスト（SPEC §8.3）。
//!
//! 検証するのは**契約**のみ。テーブル出力の文字列一致テストは書かない（§8.4）
//! — 見た目は調整する前提であり、固めるとテストがリファクタの足枷になる。
//! `--json` の形は固定（機械が読むので契約）、テーブルの見た目は自由。この非対称は
//! 意図的である。

use assert_cmd::Command;
use serde_json::Value;
use std::path::Path;
use tempfile::TempDir;

/// 一時ディレクトリに `.env` を書き、そのパスを返す。
fn write(dir: &TempDir, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, content).expect("テスト用ファイルを書けること");
    path
}

fn envdiff() -> Command {
    Command::cargo_bin("envdiff").expect("バイナリがビルドされていること")
}

/// 引数を渡して実行し、(exit code, stdout, stderr) を返す。
fn run(args: &[&Path]) -> (i32, String, String) {
    let out = envdiff()
        .args(args)
        .output()
        .expect("プロセスを起動できること");
    (
        out.status.code().expect("シグナルで死んでいないこと"),
        String::from_utf8(out.stdout).expect("stdout が UTF-8"),
        String::from_utf8(out.stderr).expect("stderr が UTF-8"),
    )
}

// ── §3 終了コードの 3 経路 ──────────────────────────────

// §3 — 差分なし → 0。
#[test]
fn exits_0_when_there_are_no_differences() {
    let dir = TempDir::new().expect("一時ディレクトリ");
    let a = write(&dir, "a.env", "PORT=3000\nAPP_ENV=production\n");
    let b = write(&dir, "b.env", "PORT=3000\nAPP_ENV=production\n");

    let (code, stdout, _) = run(&[&a, &b]);
    assert_eq!(code, 0);
    assert!(stdout.is_empty(), "差分がなければ何も出力しない（§6.1）");
}

// §3 — 差分あり → 1。
#[test]
fn exits_1_when_there_are_differences() {
    let dir = TempDir::new().expect("一時ディレクトリ");
    let a = write(&dir, "a.env", "PORT=3000\n");
    let b = write(&dir, "b.env", "PORT=8000\n");

    let (code, stdout, _) = run(&[&a, &b]);
    assert_eq!(code, 1);
    assert!(!stdout.is_empty(), "差分があれば出力する");
}

// §3 — 読み込み失敗 → 2。エラーは 1 ではない（「差分あり」と区別するため）。
#[test]
fn exits_2_when_a_file_cannot_be_read() {
    let dir = TempDir::new().expect("一時ディレクトリ");
    let a = write(&dir, "a.env", "PORT=3000\n");
    let missing = dir.path().join("does-not-exist.env");

    let (code, stdout, stderr) = run(&[&missing, &a]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty(), "エラー時に stdout を汚さない");
    assert!(
        stderr.contains("does-not-exist.env"),
        "メッセージにパスを含める（§2）: {stderr}"
    );
}

// §3 / §4.8 — パースエラー → 2。
#[test]
fn exits_2_on_a_parse_error() {
    let dir = TempDir::new().expect("一時ディレクトリ");
    let a = write(&dir, "a.env", "PORT=3000\nBROKEN\n");
    let b = write(&dir, "b.env", "PORT=3000\n");

    let (code, _, stderr) = run(&[&a, &b]);
    assert_eq!(code, 2);
    assert!(stderr.contains("a.env:2"), "行番号を指す（§4）: {stderr}");
}

// §2 — 引数が 2 個でなければ usage を stderr に出して 2。
#[test]
fn exits_2_when_arguments_are_not_two() {
    let dir = TempDir::new().expect("一時ディレクトリ");
    let a = write(&dir, "a.env", "PORT=3000\n");

    for args in [vec![], vec![a.clone()]] {
        let out = envdiff().args(&args).output().expect("起動できること");
        assert_eq!(out.status.code(), Some(2), "引数 {} 個", args.len());
        assert!(out.stdout.is_empty(), "usage は stdout に出さない（§2）");
        let stderr = String::from_utf8(out.stderr).expect("UTF-8");
        assert!(stderr.contains("Usage"), "usage を出す: {stderr}");
    }
}

// §2 — 同一ファイルを 2 回渡しても特別扱いしない。
#[test]
fn exits_0_when_the_same_file_is_passed_twice() {
    let dir = TempDir::new().expect("一時ディレクトリ");
    let a = write(&dir, "a.env", "PORT=3000\n");

    assert_eq!(run(&[&a, &a]).0, 0);
}

// §2 — 空ファイルはエラーではない。
#[test]
fn empty_files_are_not_an_error() {
    let dir = TempDir::new().expect("一時ディレクトリ");
    let empty = write(&dir, "empty.env", "");
    let other = write(&dir, "other.env", "");

    assert_eq!(run(&[&empty, &other]).0, 0);
}

// §2 — 空ファイル vs 中身ありは、全キーが片方のみ。
#[test]
fn empty_versus_populated_reports_every_key_as_one_sided() {
    let dir = TempDir::new().expect("一時ディレクトリ");
    let empty = write(&dir, "empty.env", "");
    let full = write(&dir, "full.env", "A=1\nB=2\n");

    let (code, stdout, _) = run(&[&empty, &full]);
    assert_eq!(code, 1);
    assert!(stdout.contains('A') && stdout.contains('B'));
}

// §2 — --all は表示範囲のオプションであり、終了コードの判定を変えない。
#[test]
fn all_flag_does_not_change_the_exit_code() {
    let dir = TempDir::new().expect("一時ディレクトリ");
    let a = write(&dir, "a.env", "PORT=3000\n");
    let b = write(&dir, "b.env", "PORT=8000\n");
    let same = write(&dir, "same.env", "PORT=3000\n");

    let all: &Path = Path::new("--all");
    assert_eq!(run(&[&a, &b]).0, run(&[all, &a, &b]).0, "差分あり");
    assert_eq!(run(&[&a, &same]).0, run(&[all, &a, &same]).0, "差分なし");
}

// ── §6.2 JSON のスキーマ ───────────────────────────────

/// §8.3: 文字列比較ではなく `serde_json::Value` にパースして構造で検証する
/// （キー順に依存しないため）。
#[test]
fn json_output_matches_the_schema() {
    let dir = TempDir::new().expect("一時ディレクトリ");
    let a = write(&dir, "a.env", "PORT=3000\nAPP_ENV=1\n");
    let b = write(&dir, "b.env", "PORT=8000\nDEBUG=true\n");

    let (code, stdout, _) = run(&[Path::new("--json"), &a, &b]);
    assert_eq!(code, 1);

    let v: Value = serde_json::from_str(&stdout).expect("stdout が JSON として読めること");

    // files には実ファイル名が入る（§6.2）。
    assert_eq!(v["files"]["a"], Value::String(a.display().to_string()));
    assert_eq!(v["files"]["b"], Value::String(b.display().to_string()));

    // summary の内訳（§6.2）。total は same を含まない（§3）。
    assert_eq!(v["summary"]["total"], 3);
    assert_eq!(v["summary"]["changed"], 1);
    assert_eq!(v["summary"]["only_in_a"], 1);
    assert_eq!(v["summary"]["only_in_b"], 1);
    assert_eq!(v["summary"]["same"], 0);

    let diffs = v["diffs"].as_array().expect("diffs は配列");
    assert_eq!(diffs.len(), 3);

    // 不在は null、値は文字列（§6.2）。
    let changed = &diffs[0];
    assert_eq!(changed["key"], "PORT");
    assert_eq!(changed["status"], "changed");
    assert_eq!(changed["a"], "3000");
    assert_eq!(changed["b"], "8000");

    let only_a = diffs
        .iter()
        .find(|d| d["key"] == "APP_ENV")
        .expect("APP_ENV");
    assert_eq!(only_a["status"], "only_in_a");
    assert_eq!(only_a["b"], Value::Null);

    let only_b = diffs.iter().find(|d| d["key"] == "DEBUG").expect("DEBUG");
    assert_eq!(only_b["status"], "only_in_b");
    assert_eq!(only_b["a"], Value::Null);
}

// §6.2 / §5.2 — 空文字は ""、キー不在は null。型で区別される。
#[test]
fn json_distinguishes_empty_string_from_absent_key() {
    let dir = TempDir::new().expect("一時ディレクトリ");
    let a = write(&dir, "a.env", "EMPTY=\nONLY_A=1\n");
    let b = write(&dir, "b.env", "EMPTY=x\n");

    let (_, stdout, _) = run(&[Path::new("--json"), &a, &b]);
    let v: Value = serde_json::from_str(&stdout).expect("JSON");
    let diffs = v["diffs"].as_array().expect("配列");

    let empty = diffs.iter().find(|d| d["key"] == "EMPTY").expect("EMPTY");
    assert_eq!(empty["a"], "", "空文字は存在する");
    assert_ne!(empty["a"], Value::Null, "空文字は null ではない");

    let only_a = diffs.iter().find(|d| d["key"] == "ONLY_A").expect("ONLY_A");
    assert_eq!(only_a["b"], Value::Null, "キー不在は null");
}

// §6.2 — --json と --all は直交する。
#[test]
fn json_with_all_includes_same_entries() {
    let dir = TempDir::new().expect("一時ディレクトリ");
    let a = write(&dir, "a.env", "SAME=x\nPORT=3000\n");
    let b = write(&dir, "b.env", "SAME=x\nPORT=8000\n");

    let (code, stdout, _) = run(&[Path::new("--json"), Path::new("--all"), &a, &b]);
    assert_eq!(code, 1, "--all は終了コードを変えない");

    let v: Value = serde_json::from_str(&stdout).expect("JSON");
    assert_eq!(v["diffs"].as_array().expect("配列").len(), 2);
    assert_eq!(v["summary"]["same"], 1);
    assert_eq!(v["summary"]["total"], 1, "same は差分に数えない（§3）");
}

// §6.2 — --json でも終了コードの判定は変わらない。
#[test]
fn json_does_not_change_the_exit_code() {
    let dir = TempDir::new().expect("一時ディレクトリ");
    let a = write(&dir, "a.env", "PORT=3000\n");
    let same = write(&dir, "same.env", "PORT=3000\n");

    let json: &Path = Path::new("--json");
    assert_eq!(run(&[json, &a, &same]).0, 0, "差分なし");
    assert_eq!(run(&[&a, &same]).0, 0);
}

// §6.2 — エラー時に stdout へ壊れた JSON を出さない。
#[test]
fn json_emits_nothing_on_stdout_when_parsing_fails() {
    let dir = TempDir::new().expect("一時ディレクトリ");
    let broken = write(&dir, "broken.env", "BROKEN\n");
    let ok = write(&dir, "ok.env", "PORT=3000\n");

    let (code, stdout, _) = run(&[Path::new("--json"), &broken, &ok]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty(), "壊れた JSON を出さない: {stdout}");
}

// ── §4.7 警告の出力先 ───────────────────────────────────

// §4.7 — 警告は stdout ではなく stderr に出る。
#[test]
fn duplicate_key_warning_goes_to_stderr_not_stdout() {
    let dir = TempDir::new().expect("一時ディレクトリ");
    let a = write(&dir, "a.env", "DB_HOST=old\nDB_HOST=new\n");
    let b = write(&dir, "b.env", "DB_HOST=new\n");

    let (code, stdout, stderr) = run(&[&a, &b]);

    assert!(
        stderr.contains("duplicate key 'DB_HOST'"),
        "警告は stderr に出る: {stderr}"
    );
    assert!(
        !stdout.contains("duplicate"),
        "警告は stdout に出ない（パイプに影響しない）: {stdout}"
    );
    assert_eq!(
        code, 0,
        "警告は終了コードに影響しない（§3）。後勝ちで差分なし"
    );
}

// §4.7 — 警告文が前回出現の行番号を含む。
#[test]
fn duplicate_key_warning_names_the_overridden_line() {
    let dir = TempDir::new().expect("一時ディレクトリ");
    let a = write(&dir, "a.env", "# c\nDB_HOST=old\nPORT=1\nDB_HOST=new\n");
    let b = write(&dir, "b.env", "DB_HOST=new\nPORT=1\n");

    let (_, _, stderr) = run(&[&a, &b]);
    assert!(
        stderr.contains("a.env:4:") && stderr.contains("overrides value from line 2"),
        "後勝ちした行と上書きされた行を指す: {stderr}"
    );
}

// §4.7 / §6.2 — 警告が stderr なので、--json の stdout は JSON として読めるまま。
#[test]
fn warnings_do_not_corrupt_json_on_stdout() {
    let dir = TempDir::new().expect("一時ディレクトリ");
    let a = write(&dir, "a.env", "DUP=1\nDUP=2\n");
    let b = write(&dir, "b.env", "DUP=3\n");

    let (_, stdout, stderr) = run(&[Path::new("--json"), &a, &b]);
    assert!(!stderr.is_empty(), "警告は出る");
    serde_json::from_str::<Value>(&stdout).expect("stdout は純粋な JSON のまま");
}

// ── §6.1 stdout に色を付けない ─────────────────────────

// §6.1 — テーブルに色を付けない（MVP）。テスト実行時は非 TTY だが、
// stdout に色を付けていないことはここでも確認できる。
#[test]
fn table_output_has_no_ansi_escapes() {
    let dir = TempDir::new().expect("一時ディレクトリ");
    let a = write(&dir, "a.env", "PORT=3000\n");
    let b = write(&dir, "b.env", "PORT=8000\n");

    let (_, stdout, _) = run(&[&a, &b]);
    assert!(!stdout.contains('\u{1b}'), "stdout に ANSI を出さない");
}
