//! 値の文字単位ハイライト（SPEC §6.1 の「強調」）。
//!
//! `changed` 行の A/B の値を突き合わせ、**どの範囲を強調するか**だけを返す。
//! ANSI も端末判定もここには入らない（§8.4 の線引き）。それは `render.rs` の仕事。
//!
//! このモジュールが純粋であることには理由がある。§8.4 は「テーブルの見た目を
//! 固定するテストを書かない」と定める一方、強調**範囲**の計算はロジックであり、
//! 壊れても出力を見ない限り気付けない。範囲の決定をここに切り出すことで、
//! 見た目を固定せずに範囲だけをテストできる。

use similar::{ChangeTag, TextDiff};

/// 強調に挟まれた「共通部分」をこの文字数未満なら強調へ吸収する（§6.1）。
///
/// LCS は**最小の編集**を返すが、人間が読みたいのは**まとまった塊**である。
/// `localhost` と `db.prod.internal` には `o` や `al` がたまたま共通で含まれ、
/// 素の LCS はそこで一致を取って強調を `[l]o[c]al[host]` のように砕く。
/// 編集距離としては正しいが、「どこが違うか」を示す出力としては読めない。
///
/// 3 は実際の値で確認して決めた閾値。1〜2 では砕けたままで、4 以上に上げても
/// 結果が変わらない（`3000`/`8000` や `v1.2.3`/`v1.10.3` のような短い値は
/// どの閾値でも最小限の強調に留まる）。
const MIN_COMMON_RUN: usize = 3;

/// 値の一部分と、それを強調するかどうか（§6.1）。
///
/// 「強調する」は太字を意味しない。太字は `render.rs` が被せる表現であって、
/// ここが決めるのは「A と B で実際に異なる部分か否か」だけである。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub text: String,
    pub emphasized: bool,
}

impl Segment {
    fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            emphasized: false,
        }
    }

    fn emphasized(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            emphasized: true,
        }
    }
}

/// A/B の値を突き合わせ、それぞれのセグメント列を返す（§6.1）。
///
/// 返り値は `(A 用, B 用)`。A 側は「共通部分 + A にしかない部分」、
/// B 側は「共通部分 + B にしかない部分」で構成され、後者が強調対象になる。
///
/// **文字単位で差分を取る。** 空白や区切り文字でトークナイズしない。
/// `DATABASE_URL` のような区切りのない値では、トークン分割は巨大なトークン 1 個を
/// 生んで強調が値全体に広がり、「どこが違うか」を示す目的が消える（§6.1）。
///
/// **位置合わせは LCS に任せる。** 先頭から 1 文字ずつ index を揃えて比較する実装は、
/// 挿入・削除が入った時点で以降が全てずれ、共通の末尾まで差分と判定する（§6.1）。
///
/// **LCS の結果はそのままでは使わない。** `MIN_COMMON_RUN` を参照。
pub fn segments(a: &str, b: &str) -> (Vec<Segment>, Vec<Segment>) {
    let diff = TextDiff::from_chars(a, b);

    let mut a_segs = Vec::new();
    let mut b_segs = Vec::new();

    for change in diff.iter_all_changes() {
        // `from_chars` は 1 文字ずつ返す。この時点のセグメント列は文字数と同じ
        // 長さになり、畳みと整理は下の 2 段で行う。
        match change.tag() {
            // 共通部分は両側に、強調なしで入る。
            ChangeTag::Equal => {
                a_segs.push(Segment::plain(change.value()));
                b_segs.push(Segment::plain(change.value()));
            }
            // A にしかない ＝ A 側で強調。
            ChangeTag::Delete => a_segs.push(Segment::emphasized(change.value())),
            // B にしかない ＝ B 側で強調。
            ChangeTag::Insert => b_segs.push(Segment::emphasized(change.value())),
        }
    }

    (absorb_short_runs(a_segs), absorb_short_runs(b_segs))
}

/// 隣接する同種のセグメントを 1 つに畳む。
///
/// `TextDiff::from_chars` は 1 文字ずつ返すため、畳まないとセグメント列が
/// 文字数と同じ長さになる。強調範囲としては等価だが、`render.rs` が
/// 1 文字ごとに ANSI を開閉することになり、出力が無駄に膨らむ。
fn coalesce(segs: Vec<Segment>) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::new();
    for seg in segs {
        match out.last_mut() {
            Some(last) if last.emphasized == seg.emphasized => last.text.push_str(&seg.text),
            _ => out.push(seg),
        }
    }
    out
}

/// 強調に挟まれた短い共通部分を強調へ吸収する（§6.1、`MIN_COMMON_RUN`）。
///
/// 吸収すると前後の強調と地続きになるため、畳み直して初めて 1 つの塊になる。
/// 吸収 → 畳み で新たな「挟まれた短い共通部分」が現れることがあるため、
/// 変化がなくなるまで繰り返す。
fn absorb_short_runs(segs: Vec<Segment>) -> Vec<Segment> {
    let mut out = coalesce(segs);

    loop {
        let absorbed: Vec<Segment> = out
            .iter()
            .enumerate()
            .map(|(i, seg)| {
                // 両隣が強調で、自身が短い共通部分なら強調に倒す。
                // 端（前後どちらかがない）は対象外 — 値の先頭・末尾の共通部分は
                // 「砕けている」のではなく本当に共通なので、残す。
                let sandwiched = i > 0
                    && i + 1 < out.len()
                    && !seg.emphasized
                    && out[i - 1].emphasized
                    && out[i + 1].emphasized
                    && seg.text.chars().count() < MIN_COMMON_RUN;
                if sandwiched {
                    Segment::emphasized(seg.text.clone())
                } else {
                    seg.clone()
                }
            })
            .collect();

        let next = coalesce(absorbed);
        if next == out {
            return next;
        }
        out = next;
    }
}

#[cfg(test)]
mod tests {
    // §8.4: ここでテストするのは「どの範囲が強調されるか」だけ。
    // ANSI エスケープや太字の見え方には一切触れない — それは見た目であり、
    // §8.4 が固定を禁じている領域。
    use super::*;

    /// 強調された部分だけを連結する。テストが「範囲」を主張するための補助。
    fn emphasized_text(segs: &[Segment]) -> String {
        segs.iter()
            .filter(|s| s.emphasized)
            .map(|s| s.text.as_str())
            .collect()
    }

    /// セグメントを連結すると元の値に戻ること（どのテストでも成り立つべき不変条件）。
    fn joined(segs: &[Segment]) -> String {
        segs.iter().map(|s| s.text.as_str()).collect()
    }

    // §6.1 — 変わった部分だけが強調される。
    #[test]
    fn only_the_differing_character_is_emphasized() {
        let (a, b) = segments("3000", "8000");
        assert_eq!(emphasized_text(&a), "3");
        assert_eq!(emphasized_text(&b), "8");
    }

    // §6.1 — 位置合わせの回帰テスト。
    //
    // 先頭から index を揃えて比較する実装だと `localhost`(9) と `db.prod`(7) の
    // 長さの差で以降が 2 文字ずれ、両者に同一で存在する `:5432/db` まで差分と
    // 判定される。共通の末尾が強調されないことが、LCS が効いていることの証明。
    #[test]
    fn the_common_suffix_after_an_insertion_is_not_emphasized() {
        let (a, b) = segments("postgres://localhost:5432/db", "postgres://db.prod:5432/db");
        assert!(
            !emphasized_text(&a).contains("5432"),
            "A 側で共通の `:5432/db` が強調されている（位置合わせが壊れている）"
        );
        assert!(
            !emphasized_text(&b).contains("5432"),
            "B 側で共通の `:5432/db` が強調されている（位置合わせが壊れている）"
        );
    }

    // §6.1 — 共通の接頭辞も強調されない。
    #[test]
    fn the_common_prefix_is_not_emphasized() {
        let (a, b) = segments("postgres://localhost:5432/db", "postgres://db.prod:5432/db");
        assert!(!emphasized_text(&a).contains("postgres"));
        assert!(!emphasized_text(&b).contains("postgres"));
    }

    // §6.1 — 偶然の一致で強調が砕けない（semantic cleanup）。
    //
    // `live` と `test` は `e` を共有するため、素の LCS は B 側を
    // `[t]e[st]` と 2 つに割る。編集距離としては最小だが読めない。
    // 短い共通部分は強調に吸収され、1 つの塊になる。
    #[test]
    fn an_incidental_shared_character_does_not_split_the_emphasis() {
        let (a, b) = segments("sk-live-abc123", "sk-test-abc123");
        assert_eq!(emphasized_text(&a), "liv");
        assert_eq!(emphasized_text(&b), "test");
        // B 側の強調は 1 セグメントに収まる（`t` と `st` に割れていない）。
        assert_eq!(b.iter().filter(|s| s.emphasized).count(), 1);
    }

    // §6.1 — 長い値でも強調は 1 つの塊になる。
    //
    // `localhost` と `db.prod.internal` は `o` と `al` をたまたま共有し、
    // 素の LCS は `[l]o[c]al[host]` と 3 つに砕く。
    #[test]
    fn incidental_matches_inside_a_hostname_do_not_shatter_the_emphasis() {
        let (a, b) = segments(
            "postgres://user@localhost:5432/appdb",
            "postgres://user@db.prod.internal:5432/appdb",
        );
        assert_eq!(a.iter().filter(|s| s.emphasized).count(), 1, "A: {a:?}");
        assert_eq!(b.iter().filter(|s| s.emphasized).count(), 1, "B: {b:?}");
        assert_eq!(emphasized_text(&a), "localhost");
    }

    // §6.1 — 吸収は短い値を過剰に強調しない。
    // `3000`/`8000` は閾値を上げても先頭 1 文字のまま（両隣が強調でないため）。
    #[test]
    fn short_values_are_not_over_emphasized_by_the_absorption() {
        let (a, b) = segments("v1.2.3", "v1.10.3");
        assert_eq!(emphasized_text(&a), "2");
        assert_eq!(emphasized_text(&b), "10");
    }

    // §6.1 — 値の先頭・末尾の共通部分は吸収されない。
    // 端は「砕けている」のではなく本当に共通なので残す。
    #[test]
    fn common_text_at_the_edges_is_never_absorbed() {
        let (a, b) = segments("ab-XXX-yz", "ab-YYY-yz");
        assert_eq!(emphasized_text(&a), "XXX");
        assert_eq!(emphasized_text(&b), "YYY");
        assert!(!a[0].emphasized, "先頭の共通部分が強調されている: {a:?}");
        assert!(
            !a.last().unwrap().emphasized,
            "末尾の共通部分が強調されている: {a:?}"
        );
    }

    // §6.1 — 共通部分が全くない値は、全体が強調される。
    #[test]
    fn completely_different_values_are_emphasized_entirely() {
        let (a, b) = segments("xxx", "yyy");
        assert_eq!(emphasized_text(&a), "xxx");
        assert_eq!(emphasized_text(&b), "yyy");
    }

    // §5.2 / §6.1 — 空文字と非空。空文字側には強調する対象がない。
    #[test]
    fn empty_versus_non_empty_emphasizes_only_the_non_empty_side() {
        let (a, b) = segments("", "true");
        assert_eq!(emphasized_text(&a), "");
        assert!(a.is_empty(), "空文字側にセグメントは生まれない");
        assert_eq!(emphasized_text(&b), "true");
    }

    // §6.1 — マルチバイト文字で char 境界を割らない。
    // byte index で切る実装なら panic するか、壊れた文字列が出る。
    #[test]
    fn multibyte_values_split_on_char_boundaries() {
        let (a, b) = segments("日本語です", "日本語だよ");
        assert_eq!(emphasized_text(&a), "です");
        assert_eq!(emphasized_text(&b), "だよ");
        // 連結して元に戻る ＝ 文字が壊れていない。
        assert_eq!(joined(&a), "日本語です");
        assert_eq!(joined(&b), "日本語だよ");
    }

    // 不変条件: セグメントを連結すると必ず元の値に戻る。
    // 強調範囲がどうであれ、表示される文字列が入力と変わってはいけない。
    #[test]
    fn segments_always_reconstruct_the_original_values() {
        let cases = [
            ("3000", "8000"),
            ("postgres://localhost:5432/db", "postgres://db.prod:5432/db"),
            ("", "true"),
            ("same", "same"),
            ("日本語です", "日本語だよ"),
        ];
        for (a, b) in cases {
            let (a_segs, b_segs) = segments(a, b);
            assert_eq!(joined(&a_segs), a, "A 側が復元できない: {a:?}");
            assert_eq!(joined(&b_segs), b, "B 側が復元できない: {b:?}");
        }
    }

    // 隣接する同種のセグメントが畳まれていること。
    // `8000` が 4 セグメント（`8` / `0` / `0` / `0`）に割れず、
    // `8`(強調) + `000`(非強調) の 2 つになる。
    #[test]
    fn adjacent_segments_of_the_same_kind_are_coalesced() {
        let (_, b) = segments("3000", "8000");
        assert_eq!(
            b,
            vec![Segment::emphasized("8"), Segment::plain("000")],
            "隣接セグメントが畳まれていない"
        );
    }

    // 畳んだ結果、同種のセグメントが連続しないこと（構造の不変条件）。
    #[test]
    fn no_two_adjacent_segments_share_the_same_kind() {
        let (a, b) = segments("postgres://localhost:5432/db", "postgres://db.prod:5432/db");
        for segs in [&a, &b] {
            for pair in segs.windows(2) {
                assert_ne!(
                    pair[0].emphasized, pair[1].emphasized,
                    "同種のセグメントが隣接している: {pair:?}"
                );
            }
        }
    }
}
