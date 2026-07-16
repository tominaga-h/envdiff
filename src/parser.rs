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
    Scanner::new(input).run()
}

/// 入力全体を 1 つの文字ストリームとして走査する（AD-2）。
///
/// キーの切り出しまでは行単位で足りるが、値はクォート次第で行を跨ぐ（§4.6）ため、
/// 値の読み取りは文字単位で進み、行番号は改行を数えて自前で追う。
struct Scanner<'a> {
    input: &'a str,
    /// `input` 内の現在位置（バイト単位）。
    pos: usize,
    /// 現在位置の行番号（1 始まり）。
    line: usize,
}

impl<'a> Scanner<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            pos: 0,
            line: 1,
        }
    }

    fn run(mut self) -> Result<ParseOutput, ParseError> {
        let mut vars: Vec<(String, Var)> = Vec::new();

        while self.pos < self.input.len() {
            // 変数の行番号は「その変数が始まる行」（§5.3 の表）。空行やコメントを
            // 読み飛ばした後の行なので、take_key に決めさせる。値が複数行に跨いでも
            // （§4.6）、ユーザーがその変数を探して開くのはこの行。
            let Some((key, start_line)) = self.take_key()? else {
                continue;
            };
            let value = self.take_value(start_line)?;

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

    /// 現在位置から先の残り。
    fn rest(&self) -> &'a str {
        &self.input[self.pos..]
    }

    /// 次の 1 文字。
    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    /// 1 文字進む。改行なら行番号を繰り上げる。
    fn bump(&mut self, c: char) {
        if c == '\n' {
            self.line += 1;
        }
        self.pos += c.len_utf8();
    }

    /// 行末（改行の手前）まで読み飛ばす。改行自体は消費しない。
    fn skip_to_eol(&mut self) {
        while let Some(c) = self.peek() {
            if c == '\n' {
                return;
            }
            self.bump(c);
        }
    }

    /// 次の変数のキーと、それが書かれた行番号を切り出す。読むものがなければ `None`。
    ///
    /// キーの位置に `#` が来た時点でその行はコメント（§4.3）。クォートは値にしか
    /// 現れないため、ここでの `#` 判定に曖昧さはない。
    fn take_key(&mut self) -> Result<Option<(String, usize)>, ParseError> {
        // 空白・空行・コメント行を読み飛ばす（§4.1, §4.3）。
        loop {
            let Some(c) = self.peek() else {
                return Ok(None);
            };
            match c {
                '#' => self.skip_to_eol(),
                c if c.is_whitespace() => self.bump(c),
                _ => break,
            }
        }

        // 変数が書かれた行。空白・空行・コメントを読み飛ばした後に採る（§5.3）。
        // §4.8 のエラーもこの行を指す。
        let line = self.line;

        // `=` か行末までがキーの候補。
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c == '=' || c == '\n' {
                break;
            }
            self.bump(c);
        }
        let raw = self.input[start..self.pos].trim();

        // §4.8: `=` がなければ束縛が存在せず、何を意味する行か決めようがない。
        if self.peek() != Some('=') {
            return Err(ParseError {
                line,
                message: format!("invalid line: `{}` has no `=`", strip_export(raw)),
            });
        }
        self.bump('=');

        // §4.2: export は剥がしてから §4.8 の判定にかける。
        let key = strip_export(raw).trim();
        if key.is_empty() {
            return Err(ParseError {
                line,
                message: "invalid line: no key before `=`".to_string(),
            });
        }

        Ok(Some((key.to_string(), line)))
    }

    /// `=` の直後から値を読む。
    fn take_value(&mut self, line: usize) -> Result<String, ParseError> {
        // `=` の後ろの空白はトリムする（§4.1）。ただし改行は値の終わり。
        while let Some(c) = self.peek() {
            if c == '\n' || !c.is_whitespace() {
                break;
            }
            self.bump(c);
        }

        // §4.4: 値の先頭がクォートのときだけ「値を囲む」とみなす。
        match self.peek() {
            Some(q @ ('"' | '\'')) => {
                self.bump(q);
                self.take_quoted_value(q, line)
            }
            _ => Ok(self.take_bare_value()),
        }
    }

    /// クォートで囲まれた値を読む。閉じるまで改行を跨いで読み続ける（§4.6）。
    ///
    /// EOF まで閉じなかったときのエラーはクォートの**開始行**を指す（§4.6）。
    /// 「EOF で閉じていません」だけでは、200 行のファイルのどこが原因か
    /// ユーザーが探すことになる。
    fn take_quoted_value(&mut self, quote: char, line: usize) -> Result<String, ParseError> {
        let opened_at = self.line;
        let mut value = String::new();

        loop {
            let Some(c) = self.peek() else {
                return Err(ParseError {
                    line: opened_at,
                    message: "quote opened here is never closed (reached end of file)".to_string(),
                });
            };

            // §4.4: `"..."` は 5 種のみ解釈する。`'...'` は一切解釈しない。
            if quote == '"' && c == '\\' {
                if let Some(esc) = self.rest().chars().nth(1).and_then(escape_char) {
                    self.bump('\\');
                    self.bump(
                        self.peek()
                            .expect("escape_char が Some なので次の文字はある"),
                    );
                    value.push(esc);
                    continue;
                }
                // 表にないエスケープは `\` ごと素通しする。
                self.bump(c);
                value.push(c);
                continue;
            }

            if c == quote {
                self.bump(c);
                self.finish_quoted_value(line)?;
                return Ok(value);
            }

            self.bump(c);
            value.push(c);
        }
    }

    /// 閉じクォートの後始末。空白とコメントのみを許す（§4.3, §4.4）。
    ///
    /// `KEY="foo"bar` のように文字が続く行は曖昧なのでエラー（§4.4 の表）。
    fn finish_quoted_value(&mut self, line: usize) -> Result<(), ParseError> {
        loop {
            let Some(c) = self.peek() else {
                return Ok(());
            };
            match c {
                '\n' => {
                    self.bump(c);
                    return Ok(());
                }
                '#' => {
                    self.skip_to_eol();
                    return Ok(());
                }
                c if c.is_whitespace() => self.bump(c),
                _ => {
                    return Err(ParseError {
                        line,
                        message: "unexpected characters after closing quote".to_string(),
                    });
                }
            }
        }
    }

    /// クォートで囲まれていない値を読む。行末または `#` まで（§4.1, §4.3）。
    ///
    /// 値の途中のクォートは値を囲んでいないため、単なる文字として含める
    /// （§4.4 の表）。`${` を単なる文字として扱う §4.5 と同じ理屈。
    fn take_bare_value(&mut self) -> String {
        let start = self.pos;
        // 行末・コメント直前の空白を値に含めないため、非空白の終端を覚えておく。
        let mut end = self.pos;

        while let Some(c) = self.peek() {
            if c == '\n' {
                self.bump(c);
                break;
            }
            if c == '#' {
                self.skip_to_eol();
                break;
            }
            self.bump(c);
            if !c.is_whitespace() {
                end = self.pos;
            }
        }

        self.input[start..end].to_string()
    }
}

/// §4.4: `"..."` で解釈するエスケープ。表にない文字は `None`。
fn escape_char(c: char) -> Option<char> {
    match c {
        'n' => Some('\n'),
        'r' => Some('\r'),
        't' => Some('\t'),
        '\\' => Some('\\'),
        '"' => Some('"'),
        _ => None,
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

    // ── §4.4 クォート ───────────────────────────────────────

    // §4.4 — クォートは値の入れ物であって値ではない。
    #[test]
    fn strips_double_quotes() {
        assert_eq!(vars(r#"PORT="3000""#), [("PORT".into(), "3000".into())]);
    }

    // §4.4
    #[test]
    fn strips_single_quotes() {
        assert_eq!(vars("PORT='3000'"), [("PORT".into(), "3000".into())]);
    }

    // §4.4 — この等価性が §4.4 の存在理由。
    #[test]
    fn quoted_and_unquoted_values_are_equal() {
        let unquoted = vars("PORT=3000");
        let quoted = vars(r#"PORT="3000""#);
        assert_eq!(unquoted, quoted);
    }

    // §4.4 — クォート内の空白は値の一部（トリムされない）。
    #[test]
    fn preserves_whitespace_inside_quotes() {
        assert_eq!(
            vars(r#"MSG="  padded  ""#),
            [("MSG".into(), "  padded  ".to_string())]
        );
    }

    // §4.4 — `"..."` は \n \r \t \\ \" のみ解釈する。
    #[test]
    fn interprets_only_the_five_escapes_in_double_quotes() {
        assert_eq!(
            vars(r#"V="a\nb\rc\td\\e\"f""#),
            [("V".into(), "a\nb\rc\td\\e\"f".to_string())]
        );
    }

    // §4.4 — 表にないエスケープは解釈せず、バックスラッシュごと残す。
    #[test]
    fn leaves_unlisted_escapes_untouched_in_double_quotes() {
        assert_eq!(
            vars(r#"V="a\qb""#),
            [("V".into(), r"a\qb".to_string())],
            r"\q は表にないので \ ごと素通しする"
        );
    }

    // §4.4 — `'...'` は一切解釈しない（シェルと同じ規則）。
    #[test]
    fn does_not_interpret_escapes_in_single_quotes() {
        assert_eq!(vars(r"V='a\nb\\c'"), [("V".into(), r"a\nb\\c".to_string())]);
    }

    // §4.4 — シングルクォート内のダブルクォートは文字。
    #[test]
    fn keeps_double_quote_inside_single_quotes() {
        assert_eq!(
            vars(r#"V='say "hi"'"#),
            [("V".into(), r#"say "hi""#.to_string())]
        );
    }

    // §4.3 — クォート内の `#` はコメントではない。
    #[test]
    fn hash_inside_quotes_is_not_a_comment() {
        assert_eq!(
            vars(r##"PASSWORD="p#ss#word""##),
            [("PASSWORD".into(), "p#ss#word".to_string())]
        );
    }

    // §4.3 — クォートを閉じた後の `#` はコメント。
    #[test]
    fn hash_after_closing_quote_is_a_comment() {
        assert_eq!(vars(r#"V="x" # comment"#), [("V".into(), "x".to_string())]);
    }

    // §4.5 — クォート内でも変数展開しない。
    #[test]
    fn does_not_expand_variables_inside_quotes() {
        assert_eq!(
            vars(r#"API_URL="${BASE_URL}/api""#),
            [("API_URL".into(), "${BASE_URL}/api".to_string())]
        );
    }

    // §4.4 の表 — 先頭が非クォートならクォートは値の一部（素通し）。
    #[test]
    fn quote_in_the_middle_of_a_value_is_a_literal_character() {
        assert_eq!(
            vars(r#"KEY=foo"bar""#),
            [("KEY".into(), r#"foo"bar""#.to_string())]
        );
    }

    // §4.4 の表 — dotenv が読めるファイルは envdiff も読める（§4.6/§4.7 の原則）。
    #[test]
    fn reads_json_fragment_and_quoted_words_without_quoting_the_whole_value() {
        assert_eq!(
            vars("JSON={\"k\": \"v\"}\nMSG=say \"hello\" to them"),
            [
                ("JSON".into(), r#"{"k": "v"}"#.to_string()),
                ("MSG".into(), r#"say "hello" to them"#.to_string()),
            ]
        );
    }

    // §4.4 の表 — 先頭がクォートなのに囲めていない行は曖昧なのでエラー。
    #[test]
    fn characters_after_closing_quote_are_a_parse_error() {
        let err = parse(r#"KEY="foo"bar"#).expect_err("foo か foobar か決まらない");
        assert_eq!(err.line, 1);
    }

    // §4.4 — 閉じクォートの後の空白は許す（トリムされる）。
    #[test]
    fn allows_trailing_whitespace_after_closing_quote() {
        assert_eq!(vars(r#"KEY="foo"   "#), [("KEY".into(), "foo".into())]);
    }

    // §5.2 — `KEY=""` は空文字の変数として存在する。
    #[test]
    fn empty_quotes_are_an_empty_string_value() {
        assert_eq!(vars(r#"KEY="""#), [("KEY".into(), String::new())]);
    }

    // ── §4.6 複数行の値 ──────────────────────────────────────

    // §4.6 — クォートが閉じないまま行が終わったら次行以降を値として読み続ける。
    #[test]
    fn reads_multiline_value_in_double_quotes() {
        assert_eq!(
            vars("KEY=\"line1\nline2\nline3\""),
            [("KEY".into(), "line1\nline2\nline3".to_string())]
        );
    }

    // §4.6 — `'` にも同じ規則を適用する。
    #[test]
    fn reads_multiline_value_in_single_quotes() {
        assert_eq!(
            vars("KEY='line1\nline2'"),
            [("KEY".into(), "line1\nline2".to_string())]
        );
    }

    // §4.6 — 実在する形式（Rails の master.key、GCP 認証情報など）。
    #[test]
    fn reads_multiline_private_key() {
        let input = "PRIVATE_KEY=\"-----BEGIN RSA PRIVATE KEY-----\nMIIEpAIBAAKCAQEA...\n-----END RSA PRIVATE KEY-----\"\nNEXT=1";
        let out = parse(input).expect("RSA 秘密鍵を含む .env は読めるべき");
        assert_eq!(out.vars.len(), 2);
        assert!(out.vars[0].1.value.starts_with("-----BEGIN RSA"));
        assert!(out.vars[0].1.value.ends_with("KEY-----"));
    }

    // §5.3 の表 / AD-3 — 複数行の値の行番号は開始行（`KEY=` の行）。
    #[test]
    fn multiline_value_takes_the_line_number_where_it_starts() {
        let out = parse("A=1\nKEY=\"v1\nv2\nv3\"\nB=2").expect("should parse");
        assert_eq!(out.vars[1].0, "KEY");
        assert_eq!(out.vars[1].1.line, 2, "終端行(4)ではなく開始行(2)");
    }

    // §4.6 / AD-3 — 値の内部の改行の分だけ、後続の行番号が進む。
    #[test]
    fn line_numbers_after_a_multiline_value_account_for_its_newlines() {
        let out = parse("KEY=\"v1\nv2\nv3\"\nAFTER=1").expect("should parse");
        assert_eq!(out.vars[1].0, "AFTER");
        assert_eq!(out.vars[1].1.line, 4, "値の中の 2 つの改行を数える");
    }

    // §4.6 — EOF まで閉じなければパースエラー。
    #[test]
    fn unclosed_quote_at_eof_is_a_parse_error() {
        parse("KEY=\"never closed").expect_err("EOF で閉じていない");
    }

    // §4.6 — エラーはクォートの開始行を指す。「EOF で閉じていない」だけでは
    // 200 行のファイルのどこが原因か探すことになる。
    #[test]
    fn unclosed_quote_error_points_at_the_opening_line() {
        let input = "A=1\nB=2\n\n# comment\nKEY=\"opened here\nstill going\nand going";
        let err = parse(input).expect_err("EOF で閉じていない");
        assert_eq!(err.line, 5, "終端(7)ではなくクォートの開始行(5)を指す");
    }

    // §4.6 — シングルクォートでも開始行を指す。
    #[test]
    fn unclosed_single_quote_error_points_at_the_opening_line() {
        let err = parse("A=1\nKEY='opened\nmore").expect_err("EOF で閉じていない");
        assert_eq!(err.line, 2);
    }

    // §4.6 — 複数行の値の中の `#` はコメントではない。
    #[test]
    fn hash_inside_a_multiline_value_is_not_a_comment() {
        assert_eq!(
            vars("KEY=\"line1\n# not a comment\nline3\""),
            [("KEY".into(), "line1\n# not a comment\nline3".to_string())]
        );
    }

    // §4.6 — 複数行の値の中の `KEY=` 風の行も値の一部。
    #[test]
    fn key_like_line_inside_a_multiline_value_is_part_of_the_value() {
        let out = parse("KEY=\"line1\nNOT_A_KEY=1\nline3\"").expect("should parse");
        assert_eq!(out.vars.len(), 1, "値の中の行は変数として拾わない");
        assert_eq!(out.vars[0].0, "KEY");
    }

    #[test]
    fn handles_crlf_line_endings() {
        assert_eq!(
            vars("A=1\r\nB=2"),
            [("A".into(), "1".into()), ("B".into(), "2".into())]
        );
    }
}
