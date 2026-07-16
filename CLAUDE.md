# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```sh
make check          # fmt, check, clippy, test — run this before pushing
make install-hooks  # install githooks/pre-push.sh as the pre-push hook
make build          # cargo build
make test           # cargo test
make release        # cargo build --release

cargo test parser                       # run one module's unit tests
cargo test --test cli                   # CLI integration tests only
cargo test classifies_identical_value   # a single test by name
```

`make check` runs `githooks/pre-push.sh --no-hook --fix`, which auto-applies `cargo fmt`.

## SPEC.md is the source of truth

`SPEC.md` (Japanese) defines every behavior, and the code cites it: doc comments and test names carry
section references like `§4.6`, `§5.3`, `§6.2`. `tasks/plan.md` holds the architecture decisions
(AD-1..AD-6) that the code also cites.

**Before changing behavior, read the relevant SPEC section.** Nearly every rule has a written rationale
behind it, and most "obvious improvements" (expanding `${VAR}`, sorting keys alphabetically, coloring the
table, adding `--only-missing`) are decisions the spec explicitly rejects with reasons. If a change is
genuinely warranted, update SPEC.md alongside the code, and keep the `§` citations accurate.

The governing line of the spec: **envdiff compares what is written** — not what would be set at runtime.
That single rule decides the parsing behavior (quotes stripped, variables never expanded).

## Architecture

Single binary crate. Data flows one direction, and each module maps to a test layer in §8:

```
main.rs    clap args → read files → parse → compare → render → ExitCode
parser.rs  §4 — &str → Vec<(key, Var)> in file order + duplicate-key warnings
diff.rs    §5 — two parse results → Vec<Diff> (pure; no I/O by design, §8.2)
render.rs  §6.1 — comfy-table output; also owns `visible()` (the --all filter)
json.rs    §6.2 — serde structs; calls render::visible() so both outputs filter identically
```

Load-bearing details:

- **The parser scans characters, not lines** (AD-2). Line-splitting breaks on multi-line quoted values
  (§4.6), so `Scanner` tracks line numbers by counting `\n` itself and remembers where a quote *opened*
  so an unclosed-quote error can point back at it.
- **`Var.value` is `String`, never `Option<String>`.** `KEY=` is an empty-string value that exists;
  absence is "no key in the map". `Diff.a`/`.b` are `Option<String>` where `None` means absent (`null` in
  JSON, `-` in the table) and `Some("")` means empty. This distinction is tested and must not collapse.
- **The parser never prints** (AD-4). Warnings are returned in `ParseOutput.warnings`; `main.rs` prints
  them to stderr. Errors carry only line + message; `main.rs` prepends the filename.
- **Ordering is file order, not alphabetical** (§5.3): A's keys by A's line number, then B-only keys by
  B's line number. Duplicate keys take the last-wins value *and* its line number.
- **stdout is machine-readable, stderr is human-readable.** Warnings and errors go to stderr and are
  colored via `owo_colors`' `if_supports_color(Stream::Stderr, ..)`; the table is never colored. Warnings
  do not affect the exit code.
- **Exit codes** (§3): 0 = no differences, 1 = differences, 2 = error. `--all` and `--json` change what and
  how you see, never the exit code.

## Testing

Unit tests live in `#[cfg(test)] mod tests` inside each module; `tests/cli.rs` covers the CLI contract with
`assert_cmd` + `tempfile`. Test names are written as spec statements (`empty_value_in_a_versus_absent_in_b_is_only_in_a`)
with a `// §5.2` comment naming the clause — follow that convention.

JSON is asserted as `serde_json::Value` structure, never as a string (§8.3).

**Do not add string-equality tests for the table output** (§8.4): the table's appearance is meant to be
adjustable, and pinning it makes tests fight refactors. The JSON shape is a fixed contract; the table's look
is not. This asymmetry is deliberate.

## Conventions

- Comments and docs in the source are Japanese; user-facing CLI output, README.md, and commit subjects are
  English. `docs/README_ja.md` mirrors `README.md`.
- Rust 2024 edition, MSRV 1.85.
- Doc comments explain *why* a decision holds (usually citing `§`), not what the code does.
- Commits follow Conventional Commits (`feat:`, `docs:`, `test(cli):`).
