<div align="center">

<pre>
╔══════════════════════════════════════════════════════════╗
║                                                          ║
║               ██╗   ██╗ ███████╗ ████████╗               ║
║               ██║   ██║ ██╔════╝ ╚══██╔══╝               ║
║               ██║   ██║ █████╗      ██║                  ║
║               ╚██╗ ██╔╝ ██╔══╝      ██║                  ║
║                ╚████╔╝  ███████╗    ██║                  ║
║                 ╚═══╝   ╚══════╝    ╚═╝                  ║
║                                                          ║
║                ✓  claim · check · ground                 ║
║                                                          ║
╚══════════════════════════════════════════════════════════╝
</pre>

### **Stop agents from lying.**

**vet** — claim checker for agent grounding

Agents (and humans) assert *“tests pass”*, *“status is ok”*, *“diff is clean”* —
often wrong. **vet** turns those assertions into small, named claims: run a command
or inspect the workspace, get **pass/fail with evidence**. No test harness. No YAML
framework. Pretty output for humans; **JSONL for agents**.

Ground every *“done”* in a check.

<p>
  <a href="https://github.com/ousatov-ua/vet/actions/workflows/ci.yml"><img src="https://github.com/ousatov-ua/vet/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/ousatov-ua/vet/actions/workflows/security-check.yml"><img src="https://github.com/ousatov-ua/vet/actions/workflows/security-check.yml/badge.svg" alt="Security Check"></a>
  <a href="https://github.com/ousatov-ua/vet/releases/latest"><img src="https://img.shields.io/github/v/release/ousatov-ua/vet?label=version&color=0e8a16" alt="Version"></a>
  <a href="https://github.com/ousatov-ua/vet/releases/latest"><img src="https://img.shields.io/github/release-date/ousatov-ua/vet?label=released" alt="Release date"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License"></a>
  <a href="https://github.com/ousatov-ua/vet/stargazers"><img src="https://img.shields.io/github/stars/ousatov-ua/vet?style=social" alt="GitHub Stars"></a>
</p>

<p>
  <a href="#install">Install</a>&nbsp;&nbsp;·&nbsp;&nbsp;
  <a href="#quick-start">Quick start</a>&nbsp;&nbsp;·&nbsp;&nbsp;
  <a href="#claim-kinds-v01">Claims</a>&nbsp;&nbsp;·&nbsp;&nbsp;
  <a href="#how-it-works">How it works</a>&nbsp;&nbsp;·&nbsp;&nbsp;
  <a href="docs/claims.md">Docs</a>&nbsp;&nbsp;·&nbsp;&nbsp;
  <a href="https://github.com/ousatov-ua/vet/releases/latest">Releases</a>
</p>

</div>

---

| Without **vet** | With **vet** |
|-----------------|--------------|
| Agent says “tests passed” | `vet exit 0 -- cargo test -q` — evidence or fail |
| “Health is fine” hand-waved | `vet json '.status == "healthy"' -- curl …` |
| “Repo is clean” … isn’t | `vet git clean` |
| Ad-hoc shell in agent loops | Batch claims + `--format jsonl` for tools |

---

## Install

### Homebrew

```bash
brew tap ousatov-ua/vet
brew install vet
```

### Prebuilt binaries

Grab the asset for your OS from the
[latest release](https://github.com/ousatov-ua/vet/releases/latest):

| Platform | Asset |
|----------|--------|
| Linux (amd64) | `vet-<version>-linux-amd64.tar.gz` |
| macOS (arm64) | `vet-<version>-darwin-arm64.tar.gz` |
| Windows (amd64) | `vet-<version>-windows-amd64.zip` |

```bash
# Linux / macOS — unpack and install to PATH
tar -xzf vet-*-linux-amd64.tar.gz   # or vet-*-darwin-arm64.tar.gz
install -m 0755 vet ~/.local/bin/vet

# or with GitHub CLI
gh release download -R ousatov-ua/vet -p 'vet-*-linux-amd64.tar.gz'
```

### From source

```bash
cargo install --path .
# or from a clone:
cargo build --release && cp target/release/vet ~/.local/bin/
```

Requires a Rust toolchain (edition 2021). Runtime needs `git` on `PATH` only for `git` claims.

## Quick start

```bash
vet exit 0 -- cargo test -q
vet json '.status == "healthy"' -- curl -s localhost:8080/health
vet stdout !contains 'DEPRECATED' -- ./migrate --dry-run
vet git clean
vet files exist src/main.rs Cargo.toml
vet duration lt 30s -- npm test
vet env set DATABASE_URL
```

Batch / agent mode:

```bash
vet --format jsonl <<'EOF'
exit 0 -- cargo test -q
json '.ok' -- curl -s "$URL/health"
git clean
EOF
```

## How it works

| Form | Role |
|------|------|
| `vet <claim> -- <cmd…>` | Run command, check claim against result |
| `vet <claim>` | Workspace/env claims (no command): `git`, `files`, `env` |
| `vet --format jsonl` | One JSON object per claim (agent default) |
| `vet --color auto\|always\|never` | Human color (default `auto`: TTY, no `NO_COLOR`) |
| `vet --timeout 30s …` | Kill command after duration; exit `2` on timeout |
| `vet -f claims.txt` | Batch claims from file (`-` = stdin) |

No plugins, no YAML test framework, no CI runner. Claims only.

Process exit codes:

| Code | Meaning |
|------|---------|
| `0` | All claims passed |
| `1` | One or more claims failed |
| `2` | Usage, parse, spawn, timeout, I/O, or git-tool error |

## Claim kinds (v0.1)

### `exit` — process status

```bash
vet exit 0 -- cargo test -q
vet exit nonzero -- false
vet exit 101 -- sh -c 'exit 101'
```

### `stdout` / `stderr` — stream content

```bash
vet stdout contains OK -- ./build
vet stdout !contains DEPRECATED -- ./migrate --dry-run
vet stdout equals done -- printf done
vet stdout matches 'error: .+' -- ./check
vet stderr contains panic -- ./run
```

Ops: `contains`, `!contains`, `equals` (one trailing newline stripped), `matches` (Rust regex, unanchored). Invalid regex → exit `2`.

### `json` — jq-lite path on command stdout

```bash
vet json .ok -- curl -s "$URL/health"
vet json .status exists -- curl -s "$URL/health"
vet json .status == healthy -- curl -s "$URL/health"
vet json '.status == "healthy"' -- curl -s "$URL/health"
```

Path language (not full jq):

- Dotted segments: `.status`, `items.0.name` (leading `.` optional)
- Numeric segments are array indexes
- No filters, pipes, wildcards, or functions

Modes:

| Form | Meaning |
|------|---------|
| `json PATH` | Path exists and value is **truthy** (not `null` / `false` / `0` / `""` / `[]` / `{}`) |
| `json PATH exists` | Path present (`null` counts) |
| `json PATH == VALUE` | Deep equality; `VALUE` is a JSON literal or bare string |

Invalid JSON body → **claim fail** (exit `1`), not usage error.

### `files` — path existence

```bash
vet files exist src/main.rs Cargo.toml
vet files !exist tmp/scratch
```

No command allowed. Paths are relative to the current working directory.

### `env` — variable presence

```bash
vet env set DATABASE_URL HOME
vet env !set AWS_SECRET_ACCESS_KEY
```

Empty string counts as **set**. **Values are never printed** in human or JSONL evidence.

### `git` — working tree

```bash
vet git clean
vet git dirty
```

Uses `git status --porcelain`. Not a git repository → claim **fail** (exit `1`).  
Git binary missing / status invocation failure → **exit `2`** (cannot evaluate).

### `duration` — wall clock

```bash
vet duration lt 30s -- npm test
vet duration lt 500ms -- ./fast-check
vet duration lt 2m -- cargo test
```

Units: `ms`, `s`, `m`. Comparator in v0.1: **`lt` only**.

## Output

**Human (default):** green/red mark, claim text, evidence (exit, snippet, path).  
Color: `--color auto` (default) only when stdout is a TTY and `NO_COLOR` is unset; `--color always|never` override.

```text
✓ exit 0  (exit 0, 1234ms)
✗ json .status  (path .status missing, 80ms)
```

**Agent (`--format jsonl`):** one record per claim:

```json
{"claim":"exit 0","ok":true,"exit":0,"ms":1234,"evidence":"exit 0"}
{"claim":"json .status","ok":false,"exit":0,"ms":80,"evidence":"path .status missing"}
```

## Batch files

```text
# claims.txt
exit 0 -- cargo test -q
json .ok -- curl -s localhost:8080/health
git clean
files exist Cargo.toml
```

```bash
vet -f claims.txt
vet --format jsonl -f claims.txt
vet -f - < claims.txt
```

Blank lines and `#` comments (full-line or mid-line outside quotes) are skipped. Claims run sequentially; claim failures do not stop the batch; parse/spawn/timeout/git-tool errors abort with exit `2`.

Captured stdout/stderr are capped at 1 MiB per stream.

## Documentation

| Doc | Contents |
|-----|----------|
| [docs/claims.md](docs/claims.md) | Grammar freeze, edge cases, evidence rules |
| [docs/json-subset.md](docs/json-subset.md) | jq-lite path language limits |
| [vet.md](vet.md) | Product pitch and scope |

## Develop

```bash
cargo test
cargo build --release
cargo run -- exit 0 -- true
```

## Scope

**In:** spawn command, capture exit/stdout/stderr/time; pure path/env/git checks; small JSON path language; JSONL.

**Out:** browser E2E, flake retry, remote CI, monorepo graphs, full jq, secret scanning, network mocking, shell-by-default.

## License

MIT
