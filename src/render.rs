//! テーブル出力（SPEC §6.1）。
//!
//! 見た目の細部（列幅・折り返しの閾値・罫線）は調整してよい（§6.1 の注）。
//! 固定すべき契約は「1 キー 1 行」「`-` で不在」「STATUS は A/B 表記」
//! 「サマリと凡例を出す」の 4 点。§8.4 によりテーブルの文字列一致テストは書かない。

use crate::diff::{Diff, Status, Summary};
use comfy_table::{ContentArrangement, Table, presets::NOTHING};
use std::path::Path;

/// 不在を示す記号（§6.1）。空文字の値は空セルとして表示され、これとは区別される。
const ABSENT: &str = "-";

/// STATUS 列の表記（§6.1）。
///
/// ファイル名を繰り返さず `only in A` / `only in B` と書く。ヘッダーで宣言済みで
/// あり、`--json` の `only_in_a` / `only_in_b` とも語彙が一致する。
fn status_label(status: Status) -> &'static str {
    match status {
        Status::Changed => "changed",
        Status::OnlyInA => "only in A",
        Status::OnlyInB => "only in B",
        Status::Same => "same",
    }
}

/// 値のセル。不在は `-`、空文字の値は空セル（§6.1）。
fn value_cell(value: Option<&str>) -> &str {
    value.unwrap_or(ABSENT)
}

/// 表示対象の差分を絞る（§5.1）。
///
/// `same` はデフォルトでは出力しない。100 個中 3 個違うときに 97 行のノイズを
/// 読ませないため。`--all` で含める。
pub fn visible(diffs: &[Diff], all: bool) -> Vec<&Diff> {
    diffs
        .iter()
        .filter(|d| all || d.status != Status::Same)
        .collect()
}

/// サマリ行（§6.1）。語彙はテーブルの STATUS と一致させる。
fn summary_line(s: &Summary) -> String {
    let total = s.total();
    let noun = if total == 1 {
        "difference"
    } else {
        "differences"
    };
    format!(
        "{total} {noun} ({} changed, {} only in A, {} only in B)",
        s.changed, s.only_in_a, s.only_in_b
    )
}

/// 凡例行（§6.1）。テーブルの末尾は必ず画面に残るため、ヘッダーが流れても
/// A/B の意味を解決できる。
fn legend_line(a: &Path, b: &Path) -> String {
    format!("A = {}, B = {}", a.display(), b.display())
}

/// テーブル・サマリ・凡例を stdout に出す（§6.1）。
///
/// 差分がなく `--all` もなければ何も出力しない。テーブルに色は付けない（§6.1）。
pub fn render(diffs: &[Diff], summary: &Summary, a: &Path, b: &Path, all: bool) {
    let rows = visible(diffs, all);
    if rows.is_empty() {
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(NOTHING)
        // 長い値は切り詰めず折り返す（§6.1）。ターミナル幅に応じて調整される。
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            "KEY".to_string(),
            format!("{} (A)", a.display()),
            format!("{} (B)", b.display()),
            "STATUS".to_string(),
        ]);

    for d in rows {
        table.add_row(vec![
            d.key.as_str(),
            value_cell(d.a.as_deref()),
            value_cell(d.b.as_deref()),
            status_label(d.status),
        ]);
    }

    println!("{table}");
    println!();
    println!("{}", summary_line(summary));
    println!("{}", legend_line(a, b));
}

#[cfg(test)]
mod tests {
    // §8.4: テーブル出力の文字列一致テストは意図的に書かない。
    // ここで検証するのは §6.1 が契約として固定した項目だけ。
    use super::*;
    use crate::diff::Diff;

    fn diff(key: &str, status: Status, a: Option<&str>, b: Option<&str>) -> Diff {
        Diff {
            key: key.to_string(),
            status,
            a: a.map(String::from),
            b: b.map(String::from),
        }
    }

    // §6.1 — STATUS はファイル名を繰り返さず A/B 表記。
    #[test]
    fn status_labels_use_a_and_b_not_filenames() {
        assert_eq!(status_label(Status::Changed), "changed");
        assert_eq!(status_label(Status::OnlyInA), "only in A");
        assert_eq!(status_label(Status::OnlyInB), "only in B");
        assert_eq!(status_label(Status::Same), "same");
    }

    // §6.1 — `-` はキーが存在しないことを示す。
    #[test]
    fn absent_value_renders_as_dash() {
        assert_eq!(value_cell(None), "-");
    }

    // §6.1 / §5.2 — 空文字の値は空セルで、`-` とは区別される。
    #[test]
    fn empty_value_renders_as_empty_cell_not_dash() {
        assert_eq!(value_cell(Some("")), "");
        assert_ne!(value_cell(Some("")), value_cell(None));
    }

    // §5.1 — same はデフォルトでは出力しない。
    #[test]
    fn hides_same_rows_by_default() {
        let diffs = [
            diff("PORT", Status::Changed, Some("3000"), Some("8000")),
            diff("SAME", Status::Same, Some("x"), Some("x")),
        ];
        let rows = visible(&diffs, false);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].key, "PORT");
    }

    // §2 — --all で same を含める。
    #[test]
    fn includes_same_rows_with_all() {
        let diffs = [
            diff("PORT", Status::Changed, Some("3000"), Some("8000")),
            diff("SAME", Status::Same, Some("x"), Some("x")),
        ];
        assert_eq!(visible(&diffs, true).len(), 2);
    }

    // §6.1 — 1 キー 1 行。
    #[test]
    fn renders_one_row_per_key() {
        let diffs = [
            diff("A", Status::Changed, Some("1"), Some("2")),
            diff("B", Status::OnlyInA, Some("1"), None),
            diff("C", Status::OnlyInB, None, Some("1")),
        ];
        assert_eq!(visible(&diffs, false).len(), 3);
    }

    // §6.1 — サマリの語彙は STATUS と一致する。
    #[test]
    fn summary_line_matches_the_status_vocabulary() {
        let s = Summary {
            changed: 1,
            only_in_a: 1,
            only_in_b: 1,
            same: 0,
        };
        assert_eq!(
            summary_line(&s),
            "3 differences (1 changed, 1 only in A, 1 only in B)"
        );
    }

    #[test]
    fn summary_line_is_singular_for_one_difference() {
        let s = Summary {
            changed: 1,
            ..Default::default()
        };
        assert_eq!(
            summary_line(&s),
            "1 difference (1 changed, 0 only in A, 0 only in B)"
        );
    }

    // §3 — same はサマリの件数に入らない。
    #[test]
    fn summary_line_excludes_same_from_the_count() {
        let s = Summary {
            changed: 1,
            same: 97,
            ..Default::default()
        };
        assert!(summary_line(&s).starts_with("1 difference "));
    }

    // §6.1 — 凡例は A/B が何を指すかを示す。
    #[test]
    fn legend_line_names_both_files() {
        assert_eq!(
            legend_line(Path::new(".env"), Path::new(".env.example")),
            "A = .env, B = .env.example"
        );
    }
}
