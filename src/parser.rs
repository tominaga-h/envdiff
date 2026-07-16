//! `.env` パーサ（SPEC §4）。
//!
//! 入力全体を 1 つの文字ストリームとして走査する（AD-2）。行分割で実装すると
//! §4.6 の複数行の値で破綻するため、行番号は改行を数えて自前で追う。

/// パースされた 1 変数。
///
/// `value` が `Option` でないことに意味がある。`KEY=` は「値が空文字で存在する」
/// のであって「未設定」ではない（§5.2）。不在は「マップにキーがない」ことで表す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Var {
    pub value: String,
    /// 1 始まり。複数行の値なら開始行、重複キーなら後勝ちした行（§5.3）。
    pub line: usize,
}

/// パーサの警告（§4.7）。終了コードには影響しない。
///
/// AD-4 によりパーサは出力を行わず、警告は返り値として呼び出し側へ渡す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    pub line: usize,
    pub key: String,
    pub previous_line: usize,
}

/// パース結果。`vars` はファイル記載順（§5.3）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseOutput {
    pub vars: Vec<(String, Var)>,
    pub warnings: Vec<Warning>,
}

/// パースエラー（§4.8）。行を読み飛ばさず、その時点で停止する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseError {}

/// 入力を環境変数の並びとしてパースする。
///
/// エラーは `ファイル名:行番号: メッセージ` の形で表示されるが、ファイル名を知る
/// のは呼び出し側なので、ここでは行番号とメッセージだけを返す。
pub fn parse(input: &str) -> Result<ParseOutput, ParseError> {
    let mut vars: Vec<(String, Var)> = Vec::new();
    let mut line = 1usize;
    let mut rest = input;

    while !rest.is_empty() {
        let (raw, tail, consumed_newline) = take_line(rest);
        let start_line = line;
        rest = tail;
        if consumed_newline {
            line += 1;
        }

        let Some((key, value)) = parse_line(raw, start_line)? else {
            continue;
        };
        vars.push((
            key,
            Var {
                value,
                line: start_line,
            },
        ));
    }

    Ok(ParseOutput {
        vars,
        warnings: Vec::new(),
    })
}

/// 次の 1 行を切り出す。戻り値は (行の中身, 残り, 改行を消費したか)。
fn take_line(input: &str) -> (&str, &str, bool) {
    match input.find('\n') {
        Some(i) => (input[..i].trim_end_matches('\r'), &input[i + 1..], true),
        None => (input, "", false),
    }
}

/// 1 行を (key, value) に分解する。無視すべき行なら `None`。
fn parse_line(raw: &str, line: usize) -> Result<Option<(String, String)>, ParseError> {
    let content = strip_comment(raw);
    let content = content.trim();
    if content.is_empty() {
        return Ok(None);
    }

    // §4.2: export は剥がしてから §4.8 の判定にかける。
    let content = strip_export(content);

    // §4.1: `=` の最初の出現で分割する（値に `=` を含められる）。
    let Some((key, value)) = content.split_once('=') else {
        return Err(ParseError {
            line,
            message: format!("invalid line: `{content}` has no `=`"),
        });
    };

    let key = key.trim();
    if key.is_empty() {
        return Err(ParseError {
            line,
            message: "invalid line: no key before `=`".to_string(),
        });
    }

    Ok(Some((key.to_string(), value.trim().to_string())))
}

/// §4.3: `#` から行末までを捨てる。
///
/// クォート内の `#` はコメントではない（§4.3）が、その判定はクォート処理と同じ
/// スキャナ内でしか成立しない。Task 3 で修正するため、判定はこの 1 箇所に閉じる。
fn strip_comment(raw: &str) -> &str {
    match raw.find('#') {
        Some(i) => &raw[..i],
        None => raw,
    }
}

/// §4.2: `export ` プレフィックスを剥がす。
fn strip_export(content: &str) -> &str {
    match content.strip_prefix("export") {
        // `export` の直後が空白でなければ、`exportKEY=1` のような別のキー名。
        Some(rest) if rest.starts_with([' ', '\t']) => rest.trim_start(),
        _ => content,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(input: &str) -> Vec<(String, String)> {
        parse(input)
            .expect("should parse")
            .vars
            .into_iter()
            .map(|(k, v)| (k, v.value))
            .collect()
    }

    #[test]
    fn parses_key_value() {
        assert_eq!(vars("PORT=3000"), [("PORT".into(), "3000".into())]);
    }

    // §4.1
    #[test]
    fn splits_on_first_equals_so_value_can_contain_equals() {
        assert_eq!(vars("QUERY=a=b=c"), [("QUERY".into(), "a=b=c".to_string())]);
    }

    // §4.1
    #[test]
    fn trims_whitespace_around_key_and_equals() {
        assert_eq!(vars("  PORT  =  3000  "), [("PORT".into(), "3000".into())]);
    }

    // §4.1
    #[test]
    fn ignores_blank_and_whitespace_only_lines() {
        assert_eq!(
            vars("A=1\n\n   \n\t\nB=2"),
            [("A".into(), "1".into()), ("B".into(), "2".into())]
        );
    }

    // §4.2
    #[test]
    fn strips_export_prefix() {
        assert_eq!(vars("export PORT=3000"), [("PORT".into(), "3000".into())]);
    }

    // §4.2 — `export` は空白が続くときだけプレフィックス。
    #[test]
    fn does_not_strip_export_when_part_of_key_name() {
        assert_eq!(vars("exported=1"), [("exported".into(), "1".into())]);
    }

    // §4.3
    #[test]
    fn ignores_full_line_comment() {
        assert_eq!(vars("# just a comment\nA=1"), [("A".into(), "1".into())]);
    }

    // §4.3
    #[test]
    fn ignores_trailing_comment() {
        assert_eq!(vars("A=1 # trailing"), [("A".into(), "1".into())]);
    }

    // §4.5 — 変数展開は行わない。`${` は単なる文字。
    #[test]
    fn does_not_expand_variables() {
        assert_eq!(
            vars("API_URL=${BASE_URL}/api/v1"),
            [("API_URL".into(), "${BASE_URL}/api/v1".to_string())]
        );
    }

    // §5.2 / AD-5
    #[test]
    fn key_with_empty_value_exists_as_empty_string() {
        assert_eq!(vars("API_KEY="), [("API_KEY".into(), String::new())]);
    }

    // §4.8 / AD-5
    #[test]
    fn line_without_equals_is_a_parse_error() {
        let err = parse("KEY").expect_err("`KEY` has no binding");
        assert_eq!(err.line, 1);
    }

    // §4.8 / AD-5
    #[test]
    fn line_without_key_is_a_parse_error() {
        let err = parse("=VALUE").expect_err("`=VALUE` binds nothing");
        assert_eq!(err.line, 1);
    }

    // §4.8 / AD-5 — export を剥がした後に判定する。
    #[test]
    fn export_without_equals_is_a_parse_error() {
        parse("export KEY").expect_err("`export KEY` has no binding");
    }

    // §4.8 — 読み飛ばして続行してはならない。
    #[test]
    fn stops_at_first_invalid_line_without_parsing_the_rest() {
        let err = parse("A=1\nBROKEN\nB=2").expect_err("should stop at line 2");
        assert_eq!(err.line, 2);
    }

    // §5.3 / AD-3 — 行番号は 1 始まりで、無視した行も数える。
    #[test]
    fn records_line_numbers_counting_ignored_lines() {
        let out = parse("# comment\n\nA=1\nB=2").expect("should parse");
        assert_eq!(out.vars[0].1.line, 3);
        assert_eq!(out.vars[1].1.line, 4);
    }

    // §5.3 / AD-3 — 出力はファイル記載順（ソートしない）。
    #[test]
    fn preserves_file_order() {
        let keys: Vec<_> = vars("ZEBRA=1\nALPHA=2")
            .into_iter()
            .map(|(k, _)| k)
            .collect();
        assert_eq!(keys, ["ZEBRA", "ALPHA"]);
    }

    #[test]
    fn parses_empty_input_as_zero_vars() {
        assert_eq!(vars(""), []);
    }

    #[test]
    fn handles_crlf_line_endings() {
        assert_eq!(
            vars("A=1\r\nB=2"),
            [("A".into(), "1".into()), ("B".into(), "2".into())]
        );
    }
}
