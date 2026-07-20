# vet — claim checker for agent grounding

## Pitch

Agents (and humans) assert “tests pass”, “status is ok”, “diff is clean” — often wrong.  
`vet` runs small, named claims against a command or workspace and returns pass/fail with evidence. No test file required. Pretty output for humans; JSONL for agents.

Stop agents from lying. Ground every “done” in a check.

## The problem

- Agents declare success after noisy logs without verifying exit codes, payloads, or git state.
- Humans juggle ad-hoc checks: `echo $?`, `jq`, `test -f`, `git status` — different tools, no shared schema.
- Existing options miss the seat: bats/shellspec need test *suites*; CI is not interactive mid-session; hurl is HTTP-only; token compressors shrink noise but do not verify truth.

## What it does

One job: **evaluate claims, emit evidence**.

```bash
vet exit 0 -- cargo test -q
vet json '.status == "healthy"' -- curl -s localhost:8080/health
vet stdout !contains 'DEPRECATED' -- ./migrate --dry-run
vet git clean
vet files exist -- src/main.rs Cargo.toml
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

## Subcommands / surface (deliberately small)

| Form | Role |
|------|------|
| `vet <claim> -- <cmd…>` | Run command, check claim against result |
| `vet <claim>` | Workspace/env claims (no command): `git clean`, `files exist`, `env set` |
| `vet --format jsonl` | One JSON object per claim (agent default) |
| `vet -f claims.txt` | Batch claims from file or stdin |

No plugins, no YAML test framework, no CI runner. Claims only.

## MVP claim kinds

1. **exit** — `exit 0`, `exit nonzero`
2. **stdout / stderr** — `contains`, `!contains`, optional `equals` / `matches` (regex)
3. **json** — path + compare (`==`, exists, truthy); jq-lite subset, not full jq
4. **files** — `exist` / `!exist` for paths
5. **env** — `set` / `!set` (value never printed by default)
6. **git** — `clean` / `dirty`
7. **duration** — `lt 30s` (wall clock of the command)

## Output

**Human (default):** green/red, claim text, one evidence line (exit, snippet, path).

**Agent (`--format jsonl`):** one record per claim, e.g.

```json
{"claim":"exit 0","ok":true,"exit":0,"ms":1234,"evidence":"cargo test -q"}
{"claim":"json .status","ok":false,"exit":0,"ms":80,"evidence":"path .status missing"}
```

Exit code of `vet`: `0` if all claims pass, `1` if any fail, `2` on usage/runtime error.

## Why original

Not a shell test framework (bats), not CI, not HTTP-only (hurl), not log compression (rtk/chop).  
Category: **interactive claim verification for agent grounding** — mid-session, no fixtures file required, agent-native JSONL.

## Dual value

| Audience | Get |
|----------|-----|
| Humans | Instant “is it actually true?” without writing tests |
| Agents | Deterministic pass/fail + evidence → next step without re-parsing megabyte logs |

## Scope boundaries (keep simplicity)

**In:** spawn command, capture exit/stdout/stderr/time; pure path/env/git checks; small JSON path language; jsonl.

**Out:** browser E2E, flake retry ML, remote CI fetch, monorepo “affected tests” graphs, full jq, secret scanning, network mocking.

## Rust fit

Pure orchestration + parsers. Offline, zero config, one static binary.  
Crates (illustrative): `clap`, `serde_json`, `std::process` (or `duct`), optional `jsonpath_lib` / hand-rolled path, `git2` or shell-out to `git status --porcelain`.

Weekend MVP → six claim kinds + jsonl; decade of daily use if grammar stays frozen.

## Scorecard (target)

| Axis | Target | Note |
|------|--------|------|
| Original | 8/10 | Empty seat: agent-native claims, not test suites |
| Useful | 8/10 | Daily false-green problem for agents and humans |
| Simple | 8/10 | Glue + small claim set; no research core |
| Popular | 8/10 | Viral pitch; skill one-liner; 10s demo |

## Risks

- **Name collision** — `vet` used elsewhere → alts: `must`, `fact`, `holds`, `so`
- **Claim language creep** — freeze grammar v0.1; resist full scripting
- **JSONPath subset** — document limits; do not promise jq

## Non-goals

- Replace pytest/cargo test
- Replace CI
- Become a general workflow engine

## Success looks like

```text
Agent: "tests pass"
Human: vet exit 0 -- cargo test -q
→ fail, evidence: exit 101, last error line
Agent: fixes, re-vet → ok
```

One binary. Same command for human and agent. Truth over vibes.
