//! JSON 出力（SPEC §6.2）。
//!
//! テーブル（§6.1）と違い、JSON は機械が読む契約なので形を固定する。
//! 切り詰め・折り返し・色を一切適用しない。

use crate::diff::{Diff, Status, Summary};
use serde::Serialize;
use std::path::Path;

/// `--json` の出力全体（§6.2）。
#[derive(Debug, Serialize)]
pub struct Output {
    pub files: Files,
    pub summary: SummaryJson,
    pub diffs: Vec<DiffJson>,
}

/// 実ファイル名はここに持たせる（§6.2）。
///
/// ファイル名を `diffs[]` のキーにすると入力によって JSON の形が変わり、
/// `jq` のクエリが書けなくなる。
#[derive(Debug, Serialize)]
pub struct Files {
    pub a: String,
    pub b: String,
}

/// 件数の内訳（§6.2）。
#[derive(Debug, Serialize)]
pub struct SummaryJson {
    /// 差分の総数。`same` は差分ではないので含めない（§3）。
    pub total: usize,
    pub changed: usize,
    pub only_in_a: usize,
    pub only_in_b: usize,
    pub same: usize,
}

/// 1 キーの差分（§6.2）。
///
/// `a` / `b` の `None` は「キーが存在しない」（JSON では `null`）。値が空文字の
/// 変数は `Some("")`（`""`）であり、型で区別される（§5.2）。テーブルの `-` と
/// 空セルの区別に対応する。
#[derive(Debug, Serialize)]
pub struct DiffJson {
    pub key: String,
    pub status: &'static str,
    pub a: Option<String>,
    pub b: Option<String>,
}

/// §6.2: status は snake_case の 4 種。テーブルの `only in A` 表記とは違い、
/// `--json` は `only_in_a` を使う（§6.1 が語彙の一致に言及している）。
fn status_str(status: Status) -> &'static str {
    match status {
        Status::Changed => "changed",
        Status::OnlyInA => "only_in_a",
        Status::OnlyInB => "only_in_b",
        Status::Same => "same",
    }
}

/// 出力用の構造体を組み立てる。`all` が真なら `same` も含める（§6.2）。
pub fn build(diffs: &[Diff], summary: &Summary, a: &Path, b: &Path, all: bool) -> Output {
    Output {
        files: Files {
            a: a.display().to_string(),
            b: b.display().to_string(),
        },
        summary: SummaryJson {
            total: summary.total(),
            changed: summary.changed,
            only_in_a: summary.only_in_a,
            only_in_b: summary.only_in_b,
            same: summary.same,
        },
        diffs: crate::render::visible(diffs, all)
            .into_iter()
            .map(|d| DiffJson {
                key: d.key.clone(),
                status: status_str(d.status),
                a: d.a.clone(),
                b: d.b.clone(),
            })
            .collect(),
    }
}

/// JSON を stdout に出す（§6.2）。
pub fn render(diffs: &[Diff], summary: &Summary, a: &Path, b: &Path, all: bool) {
    let output = build(diffs, summary, a, b, all);
    match serde_json::to_string_pretty(&output) {
        Ok(s) => println!("{s}"),
        // Output は必ず直列化できる形なので、ここには来ない。
        Err(e) => eprintln!("error: failed to serialize JSON: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn diff(key: &str, status: Status, a: Option<&str>, b: Option<&str>) -> Diff {
        Diff {
            key: key.to_string(),
            status,
            a: a.map(String::from),
            b: b.map(String::from),
        }
    }

    /// §8.3 に従い、文字列比較ではなく `serde_json::Value` の構造で検証する。
    fn to_value(diffs: &[Diff], all: bool) -> Value {
        let summary = crate::diff::summarize(diffs);
        let out = build(
            diffs,
            &summary,
            Path::new(".env"),
            Path::new(".env.example"),
            all,
        );
        serde_json::to_value(&out).expect("直列化できる")
    }

    // §6.2 — スキーマの例そのもの。
    #[test]
    fn matches_the_schema_in_the_spec() {
        let diffs = [
            diff("PORT", Status::Changed, Some("3000"), Some("8000")),
            diff("APP_ENV", Status::OnlyInA, Some("1"), None),
            diff("DEBUG", Status::OnlyInB, None, Some("true")),
        ];
        assert_eq!(
            to_value(&diffs, false),
            json!({
                "files": { "a": ".env", "b": ".env.example" },
                "summary": {
                    "total": 3, "changed": 1, "only_in_a": 1, "only_in_b": 1, "same": 0
                },
                "diffs": [
                    { "key": "PORT",    "status": "changed",   "a": "3000", "b": "8000" },
                    { "key": "APP_ENV", "status": "only_in_a", "a": "1",    "b": null },
                    { "key": "DEBUG",   "status": "only_in_b", "a": null,   "b": "true" },
                ]
            })
        );
    }

    // §6.2 — トップレベルは files / summary / diffs の 3 つ。
    #[test]
    fn has_the_three_top_level_keys() {
        let v = to_value(&[diff("K", Status::Changed, Some("1"), Some("2"))], false);
        let obj = v.as_object().expect("オブジェクト");
        let mut keys: Vec<_> = obj.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["diffs", "files", "summary"]);
    }

    // §6.2 — 不在は null。
    #[test]
    fn absent_value_is_null() {
        let v = to_value(&[diff("K", Status::OnlyInA, Some("1"), None)], false);
        assert_eq!(v["diffs"][0]["b"], Value::Null);
    }

    // §6.2 / §5.2 — 空文字は "" であり、null とは型で区別される。
    #[test]
    fn empty_string_value_is_not_null() {
        let v = to_value(&[diff("K", Status::Changed, Some(""), Some("x"))], false);
        assert_eq!(v["diffs"][0]["a"], json!(""));
        assert_ne!(v["diffs"][0]["a"], Value::Null, "空文字は不在ではない");
    }

    // §6.2 — キー名は a / b 固定。ファイル名は files に入る。
    #[test]
    fn diff_keys_are_a_and_b_not_filenames() {
        let v = to_value(&[diff("K", Status::Changed, Some("1"), Some("2"))], false);
        let d = v["diffs"][0].as_object().expect("オブジェクト");
        let mut keys: Vec<_> = d.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["a", "b", "key", "status"]);
        assert_eq!(v["files"]["a"], json!(".env"));
        assert_eq!(v["files"]["b"], json!(".env.example"));
    }

    // §6.2 — status は snake_case の 4 種。
    #[test]
    fn status_values_are_the_four_snake_case_variants() {
        assert_eq!(status_str(Status::Changed), "changed");
        assert_eq!(status_str(Status::OnlyInA), "only_in_a");
        assert_eq!(status_str(Status::OnlyInB), "only_in_b");
        assert_eq!(status_str(Status::Same), "same");
    }

    // §5.1 — same はデフォルトでは出力しない。
    #[test]
    fn hides_same_by_default() {
        let diffs = [
            diff("K", Status::Changed, Some("1"), Some("2")),
            diff("S", Status::Same, Some("x"), Some("x")),
        ];
        let v = to_value(&diffs, false);
        assert_eq!(v["diffs"].as_array().expect("配列").len(), 1);
    }

    // §6.2 — --all で same を含め、summary.same が数を持つ。
    #[test]
    fn all_includes_same_rows_and_counts_them() {
        let diffs = [
            diff("K", Status::Changed, Some("1"), Some("2")),
            diff("S", Status::Same, Some("x"), Some("x")),
        ];
        let v = to_value(&diffs, true);
        assert_eq!(v["diffs"].as_array().expect("配列").len(), 2);
        assert_eq!(v["summary"]["same"], json!(1));
        assert_eq!(v["diffs"][1]["status"], json!("same"));
    }

    // §3 — summary.total は same を含まない（same は差分ではない）。
    #[test]
    fn summary_total_excludes_same() {
        let diffs = [
            diff("K", Status::Changed, Some("1"), Some("2")),
            diff("S", Status::Same, Some("x"), Some("x")),
        ];
        let v = to_value(&diffs, true);
        assert_eq!(v["summary"]["total"], json!(1), "same を含めない");
        assert_eq!(v["summary"]["same"], json!(1));
    }

    // §5.3 — 出力順序はテーブルと同じ（A の記載順 → only_in_b）。
    #[test]
    fn preserves_the_diff_order() {
        let diffs = [
            diff("ZEBRA", Status::Changed, Some("1"), Some("2")),
            diff("ALPHA", Status::OnlyInA, Some("1"), None),
        ];
        let v = to_value(&diffs, false);
        assert_eq!(v["diffs"][0]["key"], json!("ZEBRA"));
        assert_eq!(v["diffs"][1]["key"], json!("ALPHA"));
    }

    // §6.2 — 差分がなくても diffs は空配列として存在する（null や欠落ではない）。
    #[test]
    fn empty_diffs_is_an_empty_array() {
        let v = to_value(&[], false);
        assert_eq!(v["diffs"], json!([]));
        assert_eq!(v["summary"]["total"], json!(0));
    }

    // §6.2 の例 — jq でこのクエリが書けることが --json の存在理由。
    #[test]
    fn supports_the_jq_query_from_the_spec() {
        let diffs = [
            diff("PORT", Status::Changed, Some("3000"), Some("8000")),
            diff("APP_ENV", Status::OnlyInA, Some("1"), None),
        ];
        let v = to_value(&diffs, false);
        let only_in_a: Vec<_> = v["diffs"]
            .as_array()
            .expect("配列")
            .iter()
            .filter(|d| d["status"] == json!("only_in_a"))
            .collect();
        assert_eq!(only_in_a.len(), 1);
        assert_eq!(only_in_a[0]["key"], json!("APP_ENV"));
    }
}
