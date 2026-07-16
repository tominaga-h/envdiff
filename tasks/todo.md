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

- [ ] **Task 11** — SPEC 改訂: §6.1 に「強調」節を追記 + §7 に `similar`（AD-8, AD-10） · S · deps: なし · 🚦 **ハヤトの文面承認待ち**
- [ ] **Task 12** — `highlight.rs`: 強調範囲を返す純粋関数（AD-10, AD-12） · M · deps: 11
- [ ] **Task 13** — `render.rs`: 値セルに太字を被せる + stdout の TTY/NO_COLOR（AD-9, AD-11） · M · deps: 12
- [ ] **Task 14** — CLI 統合テスト: 非 TTY / NO_COLOR で ANSI が出ない（§8.3） · S · deps: 13

### ⛳ Checkpoint D: ハイライトが動く

- [ ] `make check` が通る（fmt / check / clippy / test）
- [ ] 手動確認: 長い `DATABASE_URL` の差分で、**変わった箇所だけ**が太字になる
- [ ] 手動確認: `envdiff a b | cat` で ANSI が出ない（非 TTY）
- [ ] 手動確認: `NO_COLOR=1 envdiff a b` で ANSI が出ない
- [ ] `--json` の出力が Phase 3 から 1 バイトも変わらない（AD-11 の検証）
- [ ] **人間レビュー: 実物を見て、太字が「うるさくない」か**

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
| §6.1 の改訂文面 | **ハヤトの承認待ち**（Task 11） |
| 強調の粒度が細かすぎないか（`3000`→`8000` で 1 文字だけ光る） | Checkpoint D の人間レビューで実物を見て判断。**先に作り込まない** |

**正典は SPEC.md 側。** plan.md の AD 群はその要約であり、食い違ったら SPEC を正とする。
