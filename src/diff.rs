//! 比較ロジック（SPEC §5）。
//!
//! 入力はパース済みの構造体で、ファイル I/O を含まない（§8.2）。

use crate::parser::Var;
use std::collections::HashMap;

/// キーの分類（§5.1）。必ずこの 4 状態のいずれかになる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Changed,
    OnlyInA,
    OnlyInB,
    Same,
}

/// 1 キーの比較結果。
///
/// `a` / `b` が `None` なのは「キーが存在しない」ことを意味する。値が空文字の変数
/// （`KEY=`）は `Some("")` であり、両者は区別される（§5.2）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diff {
    pub key: String,
    pub status: Status,
    pub a: Option<String>,
    pub b: Option<String>,
}

/// 件数の内訳（§6.1 のサマリ / §6.2 の summary）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Summary {
    pub changed: usize,
    pub only_in_a: usize,
    pub only_in_b: usize,
    pub same: usize,
}

impl Summary {
    /// 差分の総数。`same` は差分ではないので含めない（§3）。
    pub fn total(&self) -> usize {
        self.changed + self.only_in_a + self.only_in_b
    }

    /// §3: `only_in_a` / `only_in_b` / `changed` のいずれかが 1 件以上なら差分あり。
    pub fn has_differences(&self) -> bool {
        self.total() > 0
    }
}

/// 2 つのパース結果を比較する。結果は §5.3 の順序で返る。
///
/// 重複キーは後勝ち（§4.7）。`vars` はパーサが記載順に積んでいるため、後の要素で
/// 上書きすれば値・行番号ともに後勝ちした行のものになる（§5.3 の表）。
pub fn compare(a: &[(String, Var)], b: &[(String, Var)]) -> Vec<Diff> {
    let a_map = to_map(a);
    let b_map = to_map(b);

    let mut diffs = Vec::new();

    // 1. A に存在するキーを A の行番号の昇順で（§5.3）。
    let mut a_keys: Vec<_> = a_map.iter().collect();
    a_keys.sort_by_key(|(_, var)| var.line);
    for (key, a_var) in a_keys {
        let diff = match b_map.get(key) {
            Some(b_var) if b_var.value == a_var.value => Diff {
                key: key.to_string(),
                status: Status::Same,
                a: Some(a_var.value.clone()),
                b: Some(b_var.value.clone()),
            },
            Some(b_var) => Diff {
                key: key.to_string(),
                status: Status::Changed,
                a: Some(a_var.value.clone()),
                b: Some(b_var.value.clone()),
            },
            None => Diff {
                key: key.to_string(),
                status: Status::OnlyInA,
                a: Some(a_var.value.clone()),
                b: None,
            },
        };
        diffs.push(diff);
    }

    // 2. A にないキー（only_in_b）を B の行番号の昇順で、1 の後ろに続ける（§5.3）。
    let mut b_only: Vec<_> = b_map
        .iter()
        .filter(|(key, _)| !a_map.contains_key(*key))
        .collect();
    b_only.sort_by_key(|(_, var)| var.line);
    for (key, b_var) in b_only {
        diffs.push(Diff {
            key: key.to_string(),
            status: Status::OnlyInB,
            a: None,
            b: Some(b_var.value.clone()),
        });
    }

    diffs
}

/// 差分の件数を数える。
pub fn summarize(diffs: &[Diff]) -> Summary {
    let mut s = Summary::default();
    for d in diffs {
        match d.status {
            Status::Changed => s.changed += 1,
            Status::OnlyInA => s.only_in_a += 1,
            Status::OnlyInB => s.only_in_b += 1,
            Status::Same => s.same += 1,
        }
    }
    s
}

/// 記載順の並びをキー索引に畳む。重複キーは後勝ち（§4.7）。
fn to_map(vars: &[(String, Var)]) -> HashMap<&str, &Var> {
    let mut map = HashMap::new();
    for (key, var) in vars {
        map.insert(key.as_str(), var);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    /// テスト用に「記載順の変数列」を組み立てる。行番号は 1 から順に振る。
    fn vars(entries: &[(&str, &str)]) -> Vec<(String, Var)> {
        entries
            .iter()
            .enumerate()
            .map(|(i, (k, v))| {
                (
                    k.to_string(),
                    Var {
                        value: v.to_string(),
                        line: i + 1,
                    },
                )
            })
            .collect()
    }

    fn statuses(diffs: &[Diff]) -> Vec<(&str, Status)> {
        diffs.iter().map(|d| (d.key.as_str(), d.status)).collect()
    }

    fn keys(diffs: &[Diff]) -> Vec<&str> {
        diffs.iter().map(|d| d.key.as_str()).collect()
    }

    // §5.1
    #[test]
    fn classifies_differing_value_as_changed() {
        let diffs = compare(&vars(&[("PORT", "3000")]), &vars(&[("PORT", "8000")]));
        assert_eq!(statuses(&diffs), [("PORT", Status::Changed)]);
        assert_eq!(diffs[0].a.as_deref(), Some("3000"));
        assert_eq!(diffs[0].b.as_deref(), Some("8000"));
    }

    // §5.1
    #[test]
    fn classifies_identical_value_as_same() {
        let diffs = compare(&vars(&[("PORT", "3000")]), &vars(&[("PORT", "3000")]));
        assert_eq!(statuses(&diffs), [("PORT", Status::Same)]);
    }

    // §5.1
    #[test]
    fn classifies_key_missing_from_b_as_only_in_a() {
        let diffs = compare(&vars(&[("APP_ENV", "1")]), &vars(&[]));
        assert_eq!(statuses(&diffs), [("APP_ENV", Status::OnlyInA)]);
        assert_eq!(diffs[0].b, None);
    }

    // §5.1
    #[test]
    fn classifies_key_missing_from_a_as_only_in_b() {
        let diffs = compare(&vars(&[]), &vars(&[("DEBUG", "true")]));
        assert_eq!(statuses(&diffs), [("DEBUG", Status::OnlyInB)]);
        assert_eq!(diffs[0].a, None);
    }

    // §5.2 — 空文字で存在することと、キーが不在であることは違う。
    #[test]
    fn empty_value_in_a_versus_absent_in_b_is_only_in_a() {
        let diffs = compare(&vars(&[("API_KEY", "")]), &vars(&[]));
        assert_eq!(statuses(&diffs), [("API_KEY", Status::OnlyInA)]);
        assert_eq!(
            diffs[0].a.as_deref(),
            Some(""),
            "空文字は存在する（None ではない）"
        );
    }

    // §5.2 — プレースホルダが埋まった、という事実がそのまま差分になる。
    #[test]
    fn empty_value_versus_filled_value_is_changed() {
        let diffs = compare(&vars(&[("API_KEY", "")]), &vars(&[("API_KEY", "sk-xxx")]));
        assert_eq!(statuses(&diffs), [("API_KEY", Status::Changed)]);
    }

    // §5.3 / AD-3 — アルファベット順ならこの並びは通らない。
    #[test]
    fn orders_keys_present_in_a_by_a_line_number() {
        let diffs = compare(
            &vars(&[("ZEBRA", "1"), ("ALPHA", "2")]),
            &vars(&[("ALPHA", "9"), ("ZEBRA", "9")]),
        );
        assert_eq!(keys(&diffs), ["ZEBRA", "ALPHA"]);
    }

    // §5.3 / AD-3
    #[test]
    fn orders_only_in_b_keys_after_a_keys_in_b_line_order() {
        let diffs = compare(
            &vars(&[("A_KEY", "1")]),
            &vars(&[("Z_ONLY_B", "1"), ("A_ONLY_B", "2")]),
        );
        assert_eq!(keys(&diffs), ["A_KEY", "Z_ONLY_B", "A_ONLY_B"]);
    }

    // §5.3 / AD-3 — B の行番号は A の順序に混ぜない。
    #[test]
    fn does_not_interleave_b_line_numbers_into_a_ordering() {
        // B 側の only_in_b は B の 1 行目だが、A のキーより前には出ない。
        let a = vec![(
            "LATE_IN_A".to_string(),
            Var {
                value: "1".into(),
                line: 99,
            },
        )];
        let b = vec![(
            "EARLY_IN_B".to_string(),
            Var {
                value: "1".into(),
                line: 1,
            },
        )];
        assert_eq!(keys(&compare(&a, &b)), ["LATE_IN_A", "EARLY_IN_B"]);
    }

    // §5.3 の表 — 重複キーの行番号は後勝ちした行。
    #[test]
    fn duplicate_key_uses_last_wins_value_and_line() {
        let a = vec![
            (
                "DB_HOST".to_string(),
                Var {
                    value: "old".into(),
                    line: 9,
                },
            ),
            (
                "DB_HOST".to_string(),
                Var {
                    value: "new".into(),
                    line: 14,
                },
            ),
        ];
        let diffs = compare(&a, &vars(&[]));
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].a.as_deref(), Some("new"), "後勝ちの値を採る");
    }

    // §5.3 の表 — 後勝ちした行番号で並ぶ（前の行番号ではない）。
    #[test]
    fn duplicate_key_orders_by_last_wins_line() {
        let a = vec![
            (
                "DUP".to_string(),
                Var {
                    value: "old".into(),
                    line: 1,
                },
            ),
            (
                "OTHER".to_string(),
                Var {
                    value: "x".into(),
                    line: 2,
                },
            ),
            (
                "DUP".to_string(),
                Var {
                    value: "new".into(),
                    line: 3,
                },
            ),
        ];
        // DUP が 1 行目のままなら先頭に来る。後勝ちの 3 行目を採るので OTHER の後ろ。
        assert_eq!(keys(&compare(&a, &vars(&[]))), ["OTHER", "DUP"]);
    }

    #[test]
    fn ordering_is_deterministic_across_runs() {
        let a = vars(&[("K1", "1"), ("K2", "2"), ("K3", "3"), ("K4", "4")]);
        let b = vars(&[("Z1", "1"), ("Z2", "2"), ("Z3", "3")]);
        let first = keys(&compare(&a, &b)).join(",");
        for _ in 0..20 {
            assert_eq!(keys(&compare(&a, &b)).join(","), first);
        }
    }

    #[test]
    fn summarizes_counts_by_status() {
        let diffs = compare(
            &vars(&[("PORT", "3000"), ("APP_ENV", "1"), ("SAME", "s")]),
            &vars(&[("PORT", "8000"), ("DEBUG", "true"), ("SAME", "s")]),
        );
        let s = summarize(&diffs);
        assert_eq!((s.changed, s.only_in_a, s.only_in_b, s.same), (1, 1, 1, 1));
    }

    // §3 — same は差分ではない。
    #[test]
    fn total_and_has_differences_exclude_same() {
        let diffs = compare(&vars(&[("PORT", "3000")]), &vars(&[("PORT", "3000")]));
        let s = summarize(&diffs);
        assert_eq!(s.total(), 0);
        assert!(!s.has_differences());
    }

    #[test]
    fn reports_differences_when_any_category_is_nonempty() {
        let diffs = compare(&vars(&[("ONLY_A", "1")]), &vars(&[]));
        assert!(summarize(&diffs).has_differences());
    }

    // §2 — 同一ファイルを 2 回渡しても特別扱いしない。差分なしになるだけ。
    #[test]
    fn identical_inputs_produce_no_differences() {
        let v = vars(&[("A", "1"), ("B", "2")]);
        assert!(!summarize(&compare(&v, &v)).has_differences());
    }

    // §2 — 空ファイルはエラーではない。全キーが片方のみになる。
    #[test]
    fn empty_input_makes_every_key_one_sided() {
        let diffs = compare(&vars(&[]), &vars(&[("A", "1"), ("B", "2")]));
        assert_eq!(
            statuses(&diffs),
            [("A", Status::OnlyInB), ("B", Status::OnlyInB)]
        );
    }

    #[test]
    fn two_empty_inputs_produce_no_diffs() {
        assert_eq!(compare(&vars(&[]), &vars(&[])), []);
    }
}
