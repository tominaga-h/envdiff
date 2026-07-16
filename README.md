# envdiff

Compare two `.env` files **variable by variable**.

[日本語版 README](docs/README_ja.md)

`diff` compares lines, so comments, blank lines, and ordering show up as noise —
and the thing you actually want to know (which environment variables differ) gets
buried. envdiff reads each file as a set of environment variables and compares
them by key.

```console
$ envdiff .env.example .env.local
┌──────────────┬────────────────────────────────┬───────────────────────────────────┬───────────┐
│ KEY          │ .env.example (A)               │ .env.local (B)                    │ STATUS    │
╞══════════════╪════════════════════════════════╪═══════════════════════════════════╪═══════════╡
│ APP_ENV      │ development                    │ production                        │ changed   │
├──────────────┼────────────────────────────────┼───────────────────────────────────┼───────────┤
│ API_KEY      │                                │ sk-live-abc123                    │ changed   │
├──────────────┼────────────────────────────────┼───────────────────────────────────┼───────────┤
│ DATABASE_URL │ postgres://localhost/myapp_dev │ postgres://prod.example.com/myapp │ changed   │
├──────────────┼────────────────────────────────┼───────────────────────────────────┼───────────┤
│ SENTRY_DSN   │ -                              │ https://xxx@sentry.io/1           │ only in B │
└──────────────┴────────────────────────────────┴───────────────────────────────────┴───────────┘

4 differences (3 changed, 0 only in A, 1 only in B)
A = .env.example, B = .env.local
```

## Install

```sh
cargo install --path .
```

Requires Rust 1.85+ (2024 edition).

## Usage

```
envdiff <A> <B> [OPTIONS]
```

`A` and `B` are just the first and second arguments — they map to the A and B
columns and nothing more. envdiff does not treat either one as "the baseline",
and does not guess your intent.

| Option | Description |
| --- | --- |
| `--all` | Include variables that are identical (`same`) in the output |
| `--json` | Output JSON |

`--all` (what to show) and `--json` (how to print it) are orthogonal — combine
them freely, and neither changes the meaning of the other.

## Exit codes

Follows the `diff(1)` convention, so envdiff can replace `diff` in CI without
breaking anything.

| Code | Meaning |
| --- | --- |
| `0` | No differences |
| `1` | Differences found |
| `2` | Error (parse error, unreadable file, bad arguments) |

Errors are `2`, not `1`, on purpose: with `1` the caller cannot tell "ran fine
and found differences" apart from "couldn't read the file". A parse error logged
as "differences found" sends someone hunting for a difference that doesn't exist.

This makes a drift check a one-liner:

```sh
envdiff .env.example .env   # exits 1 if they've drifted apart
```

## Status

Every key falls into exactly one of four states.

| Status | Meaning |
| --- | --- |
| `changed` | Present in both, values differ |
| `only in A` | Present only in A |
| `only in B` | Present only in B |
| `same` | Present in both, values match |

`same` is hidden by default — a diff tool should print differences, not make you
read 97 unchanged lines to find 3 changed ones. Use `--all` to include them.

`only in A` and `only in B` are kept separate because, in the common
`.env.example` vs `.env` case, they mean completely different things: in the
example but not on your machine = missing config; on your machine but not in the
example = forgot to document it, or leftover cruft.

## JSON output

```console
$ envdiff --json .env .env.example
{
  "files": { "a": ".env", "b": ".env.example" },
  "summary": { "total": 3, "changed": 1, "only_in_a": 1, "only_in_b": 1, "same": 0 },
  "diffs": [
    { "key": "PORT",    "status": "changed",   "a": "3000", "b": "8000" },
    { "key": "APP_ENV", "status": "only_in_a", "a": "1",    "b": null },
    { "key": "DEBUG",   "status": "only_in_b", "a": null,   "b": "true" }
  ]
}
```

- A missing key is `null`; an empty value is `""`. The two are distinct.
- `diffs[].a` / `.b` are always named `a` and `b`, never after your filenames, so
  `jq` queries keep working whatever you pass in. Real filenames live in `files`.
- `summary.total` counts differences only — `same` is not a difference.

envdiff deliberately refuses to rank differences by importance ("this one is
dangerous"), so needs like "only fail on missing keys" are yours to express:

```sh
envdiff --json .env.example .env | jq '.diffs[] | select(.status == "only_in_a")'
```

That escape hatch is why there's no `--only-missing` flag.

## Parsing

`.env` has no formal spec, so envdiff defines a minimal one and sticks to it.

### The rule behind the rules

> **envdiff compares what is written.**

Not "what would end up being set at runtime". That single line decides everything
below.

### Quotes are stripped

`PORT=3000` and `PORT="3000"` are **`same`**. Load either through dotenv and you
get the same variable; the quotes are the container, not the content. Reporting
that as `changed` would be exactly the noise you came here to avoid.

Escapes inside `"..."` are `\n`, `\r`, `\t`, `\\`, `\"` — nothing else.
`'...'` interprets nothing at all, same as the shell.

Quotes are stripped only when the value *starts* with a quote that closes at the
end. So `KEY=foo"bar"` and `JSON={"k": "v"}` pass through untouched — those
quotes aren't wrapping anything. If dotenv can read a file, envdiff should too.

### Variables are not expanded

`API_URL=${BASE_URL}/api/v1` and `API_URL=https://example.com/api/v1` are
**`changed`**. `${` is just a character.

Expanding would tie the result to the environment it ran in — if `BASE_URL` isn't
in the file, do you read the shell, use empty, or fail? Any answer makes CI and
your laptop disagree, and a comparison tool that gives two answers isn't worth
much. It also throws away the interesting part: one side was refactored and the
other is still hardcoded. That's a difference a human should see, not one to
silently flatten into `same`.

There is no `--expand` opt-in either — an option is still a door to
environment-dependent output.

### Multi-line values work

```env
PRIVATE_KEY="-----BEGIN RSA PRIVATE KEY-----
MIIEpAIBAAKCAQEA...
-----END RSA PRIVATE KEY-----"
```

Rails `master.key`, Firebase service accounts, GCP credentials — these are real.
If a quote is left open at end of line, envdiff keeps reading until it closes.

If it never closes, that's an error pointing at the line where the quote
**opened**:

```
error: .env:12: quote opened here is never closed (reached end of file)
```

"Unclosed quote at EOF" alone would leave you scanning 200 lines for the cause.

### Duplicate keys warn

Last one wins, and a warning goes to stderr:

```
warning: .env:14: duplicate key 'DB_HOST' (overrides value from line 9)
```

A duplicate key is almost always a bug — whoever wrote it thinks the earlier line
is in effect. Handling it silently would let envdiff report "no differences"
while walking past a time bomb. It's not a parse error, though: a duplicate in
one file shouldn't stop you from seeing the differences you came for.

Warnings go to stderr, not stdout, so pipes stay clean. They don't affect the
exit code — a warning is a warning, not a failure.

### Other rules

| Input | Result |
| --- | --- |
| `KEY=VALUE` | Split at the **first** `=`, so values may contain `=` |
| `export KEY=VALUE` | `export` is stripped |
| `# comment` | Ignored — but a `#` inside quotes is not a comment |
| `KEY=` | A variable whose value is the empty string (**not** "unset") |
| `KEY` (no `=`) | **Parse error** — nothing is bound, so the line has no meaning |
| `=VALUE` | **Parse error** — nothing to bind to |

A bad line stops the run (exit 2) rather than being skipped. Silently skipping
would report "this variable is missing from one side", and you'd go debugging
your file instead of envdiff. A comparison tool must not report what it failed to
read as a difference.

### Output order

Keys appear in **the order they're written**, not alphabetically: A's keys in A's
line order, then B-only keys in B's line order.

You read a diff and then open the file to fix it — matching order lets you follow
both top to bottom. Sorting alphabetically throws away "where do I look?".

## Colors

The table is never colored. `STATUS` already says `only in B`; color adds
nothing, and red/green would imply "bad/good" — which is envdiff deciding for you
which differences matter.

Warnings and errors on stderr *are* colored, since drawing your eye is the whole
job there. Respects `NO_COLOR`, and turns off when stderr isn't a TTY.

## Development

```sh
make check          # fmt, check, clippy, test — same as CI and the pre-push hook
make install-hooks  # install the pre-push hook
```

## License

MIT
