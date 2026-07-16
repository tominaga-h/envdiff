# envdiff — Task List

詳細（受け入れ基準・検証手順・設計判断）は [plan.md](./plan.md) を参照。

## 検証コマンド

タスク完了の判定は **`make check`** で行う（`cargo test` 単体では使わない）。
`make check` は fmt(自動修正) → check → clippy → test の 4 段を回すため、
テストだけでなくフォーマットと lint の破れも同時に検出できる。中身は
`githooks/pre-push.sh` で、pre-push フックと同一（`make install-hooks` で導入）。

| コマンド | 用途 |
| --- | --- |
| `make check` | 完了判定。CI と pre-push と同じ内容 |
| `cargo test parser::` 等 | 開発中の絞り込み実行（`make check` の代わりにはしない） |
| `cargo build` | 手動確認前にバイナリを更新する（`make check` は `cargo check` のため実行ファイルを更新しない） |

## Phase 1: 骨格と最小の縦串

- [x] **Task 1** — Cargo.toml 依存追加 + モジュール骨格（§7） · S · deps: なし
- [x] **Task 2** — パーサ: 基本形・export・コメント・行番号・パースエラー（§4.1-4.3, §4.8） · M · deps: 1
- [x] **Task 5** — 比較ロジック: 4 分類 + 出力順序（§5.1, §5.2, §5.3） · S · deps: 2
- [x] **Task 6** — CLI 骨格: 引数・ファイル読み込み・終了コード（§2, §3） · M · deps: 2, 5
- [x] **Task 7** — テーブル出力（§6.1） · M · deps: 6

### ⛳ Checkpoint A

- [x] `make check` が通る（fmt / check / clippy / test。48 tests）
- [x] 手動確認: `.env` 2 つを比較してテーブルが出て exit 1
- [x] 手動確認: 出力の並びが A の記載順と一致する（§5.3）
- [x] **人間レビュー: 実際の出力を見て §5.3 の順序が読みやすいか** ← 未実施（ハヤト待ち）

## Phase 2: パーサの深掘り

- [x] **Task 3** — パーサ: クォートと複数行の値（§4.4, §4.6） · M · deps: 2 · ⚠️ 最難所
- [x] **Task 4** — パーサ: 重複キーの後勝ちと警告（§4.7） · S · deps: 3

### ⛳ Checkpoint B

- [x] §8.1 の 12 項目すべてにテストが存在し、通る
- [x] `make check` が通る（82 tests）
- [x] 手動確認: 複数行の RSA 秘密鍵を含む `.env` が比較できる
- [x] 手動確認: EOF 未閉鎖のクォートが、**開始行**を指すエラーで exit 2

## Phase 3: 出力の完成

- [x] **Task 8** — JSON 出力（§6.2） · S · deps: 6
- [x] **Task 9** — stderr の色（`owo-colors`）/ NO_COLOR / 非 TTY（§6.1） · S · deps: 4, 7
- [x] **Task 10** — CLI 統合テスト（§8.3） · M · deps: 7, 8, 9

### ⛳ Checkpoint C: 完成

- [x] `make check` が通る（fmt / check / clippy / test。112 tests）
- [x] §8.4 が守られている（テーブル出力の文字列一致テストがない）
- [x] SPEC の全 § に対応する実装とテストが存在する

## Phase 4: changed 行の値の文字単位ハイライト（v1.0.0 後）

詳細は [plan.md の Addendum](./plan.md) を参照（AD-8..AD-12）。

**方針:** 色ではなく**太字**。`changed` 行の A/B 値セルだけ、`similar` の文字単位差分で
変わった箇所を強調する。§6.1 の「テーブルに色を付けない」は**撤回しない**（太字は色ではない）。

- [x] **Task 11** — SPEC 改訂: §6.1 に「強調」節を追記 + §7 に `similar`（AD-8, AD-10） · S · deps: なし · ハヤト承認済み
- [x] **Task 12** — `highlight.rs`: 強調範囲を返す純粋関数（AD-10, AD-12） · M · deps: 11
- [x] **Task 13** — `render.rs`: 値セルに太字を被せる + stdout の TTY/NO_COLOR（AD-9, AD-11） · M · deps: 12
- [ ] **Task 14** — CLI 統合テスト: 非 TTY / NO_COLOR で ANSI が出ない（§8.3） · S · deps: 13
  - 当初の想定に加えて、**既存の `table_output_has_no_ansi_escapes` を環境非依存にする**。
    現状は環境変数を制御せず親から継承するため、`FORCE_COLOR` が立った環境では落ちる

### ⛳ Checkpoint D: ハイライトが動く

- [x] `make check` が通る（fmt / check / clippy / test。131 tests）
- [x] 手動確認: 長い `DATABASE_URL` の差分で、**変わった箇所だけ**が太字になる
- [x] 手動確認: `envdiff a b | cat` で ANSI が出ない（非 TTY）
- [x] 手動確認: `NO_COLOR=1 envdiff a b` で ANSI が出ない
- [x] `--json` の出力が Phase 3 から 1 バイトも変わらない（AD-11 の検証）
      — `git diff d735406..HEAD -- src/json.rs` が空。`json.rs` は 1 行も変わっていない
- [x] **人間レビュー: 実物を見て、太字が「うるさくない」か** — ハヤト確認済み（罫線の崩れも解消）

### Phase 4 で顕在化した問題（記録）

計画が「調査済み」としていた前提が誤っており、実装後に 2 つの問題が出た。どちらも
**非 TTY でしかテストしていなかった**ことが共通の原因。

| 問題 | 原因 | 対処 |
| --- | --- | --- |
| 太字の行だけ罫線が崩れた | `comfy-table` の `custom_styling` feature が既定で無効。ANSI を表示文字として数える。plan の Risks 表は「ANSI 除去後の幅で計算する（調査済み）」と書いていたが、確認せずに書いた誤り | feature を有効化。Cargo.toml と SPEC §6.1/§7 に「必須」と根拠付きで明記。`render.rs` に回帰テスト（ANSI を直接セルに入れて幅を比較） |
| 実端末で `make check` が落ちた（テスト 2 本） | テスト側が同じ誤りを犯していた。`chars().count()` で ANSI を表示文字として数え、正しい出力を失敗と判定。もう 1 本は「強調が無効なとき」と名乗りながら無効化を制御せず環境任せ | 表示幅で測るよう修正。強調の有無に依存しない主張（ANSI を除けば元の値が残る）に変更 |

**教訓:** 強調は端末依存であるため、**非 TTY だけのテストはこの機能の経路を一度も通らない**。
以降この領域を触るときは TTY / 非 TTY / `NO_COLOR` の 3 環境で確認する。

---

## 並列化

- **並列可**: Task 8（JSON）と Task 9（色） — 互いに独立、どちらも Task 6/7 の後
- **順次必須**: Task 2 → 3 → 4（同一ファイル `parser.rs` を深くしていく）
- **要注意**: Task 5 は Task 2 の型定義にのみ依存。型が確定すれば Task 6 と並行可能
- **Phase 4 は並列不可**: 11 → 12 → 13 → 14 が一直線。Task 11 は仕様確定、
  12 は 13 の入力、13 は 14 の検証対象

## 未解決

Phase 1–3: なし。計画中に判明した仕様の空白は SPEC.md に反映済み（§4.8 の判定表、
§5.3 の出力順序、§6.1 の `--all` かつ差分なし、§7 の `owo-colors`）。

Phase 4:

| 論点 | 状態 |
| --- | --- |
| §6.1 の改訂文面 | **解決。** ハヤト承認済み（Task 11） |
| 強調の粒度が細かすぎないか（`3000`→`8000` で 1 文字だけ光る） | **解決したが、問題は逆だった。** 実データでは細かすぎるどころか**砕けた**（`live`/`test` が `[t]e[st]`、`localhost`/`db.prod.internal` が `[l]o[c]al[host]`）。偶然の一致を LCS が拾うため。3 文字未満の共通部分を強調に吸収する規則を追加（SPEC §6.1、`MIN_COMMON_RUN`）。閾値 3 は実データで決めた — 1〜2 では砕けたまま、4 以上で結果が変わらない |
| `liv`/`test` で A/B の強調幅が揃わない | **残っている（軽微）。** `live` の末尾 `e` が共通のため A 側は `liv` の 3 文字。吸収規則は「両隣が強調」の条件を満たさない端には効かない。ハヤトが実物を見て許容。気になれば端の短い共通部分も吸収する形に広げられる |

**正典は SPEC.md 側。** plan.md の AD 群はその要約であり、食い違ったら SPEC を正とする。
