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

- [ ] **Task 8** — JSON 出力（§6.2） · S · deps: 6
- [ ] **Task 9** — stderr の色（`owo-colors`）/ NO_COLOR / 非 TTY（§6.1） · S · deps: 4, 7
- [ ] **Task 10** — CLI 統合テスト（§8.3） · M · deps: 7, 8, 9

### ⛳ Checkpoint C: 完成

- [ ] `make check` が通る（fmt / check / clippy / test）
- [ ] §8.4 が守られている（テーブル出力の文字列一致テストがない）

---

## 並列化

- **並列可**: Task 8（JSON）と Task 9（色） — 互いに独立、どちらも Task 6/7 の後
- **順次必須**: Task 2 → 3 → 4（同一ファイル `parser.rs` を深くしていく）
- **要注意**: Task 5 は Task 2 の型定義にのみ依存。型が確定すれば Task 6 と並行可能

## 未解決

なし。計画中に判明した仕様の空白は SPEC.md に反映済み（§4.8 の判定表、§5.3 の出力順序、
§6.1 の `--all` かつ差分なし、§7 の `owo-colors`）。

**正典は SPEC.md 側。** plan.md の AD 群はその要約であり、食い違ったら SPEC を正とする。
