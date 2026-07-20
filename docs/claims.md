# Claim grammar (v0.1 freeze)

This document freezes the MVP claim language. Resist scripting features; extend only with a version bump.

## Line form

```
<claim-tokens> [-- <command argv…>]
```

- Tokens are whitespace-separated; single or double quotes preserve spaces.
- `#` starts a comment when not inside quotes (batch files: full-line or mid-line).
- Standalone `--` separates claim from command argv (no shell; pass `sh -c '…'` if needed).
- Workspace claims (`files`, `env`, `git`) take arguments **before** `--`, never after.

### Command rules

| Claim | Command |
|-------|---------|
| `exit`, `stdout`, `stderr`, `json`, `duration` | **Required** after `--` |
| `files`, `env`, `git` | **Forbidden** (usage error if present) |

## Kinds

### exit

```
exit <integer>
exit nonzero
```

- Compares to process exit code from wait status.
- Signal termination: not equal to any `exit N`; counts as success for `exit nonzero`.

### stdout / stderr

```
stdout|stderr contains <needle>
stdout|stderr !contains <needle>
stdout|stderr equals <needle>
stdout|stderr matches <regex>
```

| Op | Rule |
|----|------|
| `contains` | Substring of full stream (UTF-8 lossy) |
| `!contains` | Negated substring |
| `equals` | Exact match after stripping **one** trailing `\n` (and `\r` before it) from both sides |
| `matches` | Rust `regex` crate, unanchored; compile failure → parse error (exit 2) |

### json

```
json <path>
json <path> exists
json <path> == <value>
```

Source: command **stdout**, trimmed; must parse as one JSON value.

Also accepted: one shell-quoted expression token after `json`, e.g. `json '.status == "healthy"'`.

See [json-subset.md](json-subset.md).

### files

```
files exist <path> [<path>…]
files !exist <path> [<path>…]
```

- `exist` uses `Path::exists` (file or directory).
- Fails on first violating path; evidence names that path only.

### env

```
env set <NAME> [<NAME>…]
env !set <NAME> [<NAME>…]
```

- Set means `std::env::var_os` is `Some` (empty string is set).
- Evidence must never include values—only names and set/unset.

### git

```
git clean
git dirty
```

- Implementation: `git status --porcelain` in cwd.
- Empty porcelain → clean; any line → dirty.
- Not a repository → **claim failure** (exit 1), evidence `not a git repository`.
- Git tool missing / `git status` operational failure → **exit 2** (cannot evaluate).

### duration

```
duration lt <Nms|Ns|Nm>
```

- Wall clock from process spawn to wait completion.
- `lt` is strict less-than.
- Units: `ms`, `s`, `m` (integer magnitude only).

## Verdict & output

Each claim produces a `Verdict`:

| Field | Notes |
|-------|-------|
| `claim` | Canonical display string from the claim AST |
| `ok` | Pass/fail |
| `exit` | Present when a command ran |
| `ms` | Present when a command ran |
| `evidence` | Short human-readable reason (no env secrets) |

Process exit: `0` all pass, `1` any fail, `2` usage/parse/spawn/timeout/IO/git-tool error.

CLI flags (not claim grammar):

| Flag | Notes |
|------|-------|
| `--format human\|jsonl` | Output format (default `human`) |
| `--color auto\|always\|never` | Human color (default `auto`: TTY and no `NO_COLOR`) |
| `--timeout <duration>` | Kill command after duration (`30s`, `500ms`, …); exit 2 on timeout |
| `-f` / `--file` | Batch file |

Captured stdout/stderr are capped at 1 MiB per stream (excess discarded).

## Non-goals for v0.1

- Additional duration ops (`le`, `gt`, …)
- Full jq / JSONPath filters
- Shell evaluation of claim lines
- Network or browser claims
