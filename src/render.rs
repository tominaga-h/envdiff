//! テーブル出力（SPEC §6.1）。
//!
//! 見た目の細部（列幅・折り返しの閾値・罫線）は調整してよい（§6.1 の注）。
//! 固定すべき契約は「1 キー 1 行」「`-` で不在」「STATUS は A/B 表記」
//! 「サマリと凡例を出す」の 4 点。§8.4 によりテーブルの文字列一致テストは書かない。

use crate::diff::{Diff, Status, Summary};
use crate::highlight;
use comfy_table::{
    ContentArrangement, Table, modifiers::UTF8_SOLID_INNER_BORDERS, presets::UTF8_FULL,
};
use owo_colors::{OwoColorize, Stream};
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

/// 1 行分の A/B の値セルを組む（§6.1 の「強調」）。
///
/// `changed` 行だけが強調の対象になる。`only in A` / `only in B` は片側にキーが
/// 存在せず比較対象がないため、値を丸ごと太字にすると「値の中のどこかが変わった」
/// という誤読を招く。実際にはキーごと存在しないのであり、それは STATUS 列が言っている。
/// `same` は定義上差分がない。
fn value_cells(d: &Diff) -> (String, String) {
    match (d.status, d.a.as_deref(), d.b.as_deref()) {
        (Status::Changed, Some(a), Some(b)) => {
            let (a_segs, b_segs) = highlight::segments(a, b);
            (emphasize(&a_segs), emphasize(&b_segs))
        }
        _ => (
            value_cell(d.a.as_deref()).to_string(),
            value_cell(d.b.as_deref()).to_string(),
        ),
    }
}

/// 強調セグメントに太字を被せてセル文字列にする（§6.1）。
///
/// 判定は **stdout** に対して行う（stderr の warning / error とは独立）。
/// `NO_COLOR` および非 TTY では `if_supports_color` が素の文字列を返すため、
/// セル文字列は強調なしのときと完全に同一になる。
///
/// 太字は色ではないため、§6.1 の「テーブルに色を付けない」とは矛盾しない。
fn emphasize(segs: &[highlight::Segment]) -> String {
    segs.iter()
        .map(|s| {
            if s.emphasized {
                s.text
                    .if_supports_color(Stream::Stdout, |t| t.bold())
                    .to_string()
            } else {
                s.text.clone()
            }
        })
        .collect()
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

/// ヘッダー行（§6.1 の列）。
///
/// A/B の列名にファイル名を入れるからこそ、STATUS 側でファイル名を繰り返さずに
/// `only in A` と書ける（§6.1）。
fn header_row(a: &Path, b: &Path) -> Vec<String> {
    vec![
        "KEY".to_string(),
        format!("{} (A)", a.display()),
        format!("{} (B)", b.display()),
        "STATUS".to_string(),
    ]
}

/// 行からテーブルを組む（§6.1）。
///
/// `render` から分けてあるのは、セルに ANSI が入ったときの桁揃えをテストする
/// ため。`comfy-table` は `custom_styling` feature がなければエスケープを
/// ただの文字として数え、太字を入れた行だけ幅が狂う（Cargo.toml のコメント）。
/// print と組み立てが同じ関数にあると、この回帰を検出する術がない。
fn build_table(rows: &[&Diff], a: &Path, b: &Path) -> Table {
    let mut table = Table::new();
    table
        // 罫線付き。折り返しで行の高さが不揃いになっても（§6.1）、行間の罫線で
        // どこまでが 1 変数かが分かる。
        .load_preset(UTF8_FULL)
        // 内側の罫線を実線にする（既定は点線 `┆` / `╌`）。
        .apply_modifier(UTF8_SOLID_INNER_BORDERS)
        // 長い値は切り詰めず折り返す（§6.1）。ターミナル幅に応じて調整される。
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(header_row(a, b));

    for d in rows {
        let (a_cell, b_cell) = value_cells(d);
        table.add_row(vec![
            d.key.clone(),
            a_cell,
            b_cell,
            status_label(d.status).to_string(),
        ]);
    }
    table
}

/// テーブル・サマリ・凡例を stdout に出す（§6.1）。
///
/// 差分がなく `--all` もなければ何も出力しない。テーブルに色は付けない（§6.1）。
pub fn render(diffs: &[Diff], summary: &Summary, a: &Path, b: &Path, all: bool) {
    let rows = visible(diffs, all);
    if rows.is_empty() {
        return;
    }

    println!("{}", build_table(&rows, a, b));
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

    // §6.1 — 列は KEY / <Aのファイル名> (A) / <Bのファイル名> (B) / STATUS。
    #[test]
    fn header_row_names_the_four_columns_with_filenames() {
        assert_eq!(
            header_row(Path::new(".env"), Path::new(".env.example")),
            ["KEY", ".env (A)", ".env.example (B)", "STATUS"]
        );
    }

    // §6.1 — 先頭行はデータ行ではなくヘッダーとして組まれる。
    // 罫線の見た目自体は検証しない（§8.4）。ヘッダーが存在することだけを固定する。
    #[test]
    fn table_renders_the_header_above_the_data_rows() {
        let diffs = [diff("PORT", Status::Changed, Some("3000"), Some("8000"))];
        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_SOLID_INNER_BORDERS)
            .set_header(header_row(Path::new(".env"), Path::new(".env.example")));
        for d in visible(&diffs, false) {
            table.add_row(vec![d.key.as_str(), "3000", "8000", "changed"]);
        }
        assert!(table.header().is_some(), "ヘッダーが設定されていること");
        let out = table.to_string();
        let key_at = out.find("KEY").expect("ヘッダーの KEY が出力にある");
        let port_at = out.find("PORT").expect("データ行の PORT が出力にある");
        assert!(key_at < port_at, "ヘッダーはデータ行より前に出る");
    }

    // §6.1 — 強調は changed 行だけ。片側にキーがない行は無加工で出る。
    //
    // テストは非 TTY（cargo test はパイプ経由）で走るため `if_supports_color` は
    // 素の文字列を返す。ここで主張するのは「セルの中身が値そのものであること」
    // であり、太字の見え方ではない（§8.4）。
    #[test]
    fn one_sided_rows_are_not_emphasized() {
        let only_a = diff("KEY", Status::OnlyInA, Some("value"), None);
        assert_eq!(value_cells(&only_a), ("value".to_string(), "-".to_string()));

        let only_b = diff("KEY", Status::OnlyInB, None, Some("value"));
        assert_eq!(value_cells(&only_b), ("-".to_string(), "value".to_string()));
    }

    // §6.1 — same 行も強調しない（定義上、差分がない）。
    #[test]
    fn same_rows_are_not_emphasized() {
        let d = diff("KEY", Status::Same, Some("x"), Some("x"));
        assert_eq!(value_cells(&d), ("x".to_string(), "x".to_string()));
    }

    // §6.1 / §5.2 — 強調しても `-` と空セルの区別は保たれる。
    #[test]
    fn emphasis_preserves_the_distinction_between_absent_and_empty() {
        let empty_vs_absent = diff("KEY", Status::OnlyInA, Some(""), None);
        assert_eq!(
            value_cells(&empty_vs_absent),
            ("".to_string(), "-".to_string())
        );
    }

    // §6.1 — changed 行のセルは、強調の有無にかかわらず値そのものを運ぶ。
    //
    // 強調が有効かどうかは端末に依存する（TTY か / `NO_COLOR` か）。テストは
    // どちらの環境でも走るため、ANSI の有無を前提にしてはいけない。ここで
    // 主張するのは「ANSI を取り除けば元の値が残る」＝ 強調が値を書き換えない
    // ことであり、これはどちらの環境でも真になる。
    #[test]
    fn changed_cells_carry_the_values_through_the_emphasis() {
        let d = diff("PORT", Status::Changed, Some("3000"), Some("8000"));
        let (a, b) = value_cells(&d);
        assert_eq!(strip_ansi(&a), "3000");
        assert_eq!(strip_ansi(&b), "8000");
    }

    // §6.1 — セルに ANSI が入っても桁が揃う。
    //
    // これは見た目のテストではなく、罫線が壊れないことの回帰テスト（§8.4 の対象外）。
    // `comfy-table` の `custom_styling` feature がないと、エスケープシーケンスを
    // ただの文字として数えて太字の行だけ幅が広がり、罫線が破綻する。
    //
    // 通常のテストは非 TTY で走るため強調が無効になり、この経路を一度も通らない。
    // ここでは ANSI を直接セルに入れて、feature が効いていることを確かめる。
    #[test]
    fn ansi_in_a_cell_does_not_break_column_alignment() {
        // 中身は同じ「3000」だが、片方は太字の ANSI 付き。
        let plain = build_row_widths("3000", "8000");
        let bolded = build_row_widths("\x1b[1m3\x1b[0m000", "\x1b[1m8\x1b[0m000");
        assert_eq!(
            plain, bolded,
            "ANSI の有無で列幅が変わっている（custom_styling feature が効いていない）"
        );
    }

    /// ANSI エスケープを取り除く。
    ///
    /// 端末が実際に表示する文字列を得るための処理。エスケープは画面上で
    /// 幅を持たないため、桁揃えを見るテストは必ずこれを通す。
    fn strip_ansi(s: &str) -> String {
        let mut out = String::new();
        let mut in_escape = false;
        for c in s.chars() {
            match c {
                '\x1b' => in_escape = true,
                'm' if in_escape => in_escape = false,
                _ if in_escape => {}
                _ => out.push(c),
            }
        }
        out
    }

    /// テーブルを組んで各行の**表示幅**を返す。
    fn build_row_widths(a_val: &str, b_val: &str) -> Vec<usize> {
        let d = diff("PORT", Status::Changed, Some(a_val), Some(b_val));
        // value_cells を通さず直接セルに入れる（強調の有無ではなく
        // ANSI の扱いを見たいため）。
        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_SOLID_INNER_BORDERS)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(header_row(Path::new("a.env"), Path::new("b.env")));
        table.add_row(vec![
            d.key.clone(),
            a_val.to_string(),
            b_val.to_string(),
            status_label(d.status).to_string(),
        ]);
        table
            .to_string()
            .lines()
            .map(|line| strip_ansi(line).chars().count())
            .collect()
    }

    // §6.1 — 実際の描画経路（value_cells 経由）でも行の幅が揃う。
    // 罫線行とデータ行がすべて同じ**表示幅**であることが、テーブルが壊れて
    // いないことの定義。
    //
    // 幅は必ず ANSI を除いて数える。強調が有効な端末では changed 行のセルに
    // エスケープが入るが、それは画面上で幅を持たない。`chars().count()` で
    // 数えると、comfy-table が犯していたのと同じ誤り（エスケープを表示文字と
    // して数える）をテスト側で繰り返すことになり、正しい出力を失敗と判定する。
    #[test]
    fn every_line_of_the_table_has_the_same_display_width() {
        let diffs = [
            diff(
                "DATABASE_URL",
                Status::Changed,
                Some("postgres://user@localhost:5432/appdb"),
                Some("postgres://user@db.prod.internal:5432/appdb"),
            ),
            diff("SECRET", Status::OnlyInA, Some("only-in-a"), None),
        ];
        let rows = visible(&diffs, false);
        let table = build_table(&rows, Path::new("a.env"), Path::new("b.env"));
        let widths: Vec<usize> = table
            .to_string()
            .lines()
            .map(|l| strip_ansi(l).chars().count())
            .collect();
        let first = widths[0];
        assert!(
            widths.iter().all(|w| *w == first),
            "行ごとに表示幅が違う（罫線が壊れている）: {widths:?}"
        );
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
