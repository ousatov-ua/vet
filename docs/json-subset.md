# JSON path subset (jq-lite)

`vet json` is **not** jq. This document lists the supported subset so agents do not assume full jq.

## Source

- Command **stdout** only (trimmed).
- Must be a single JSON value (`serde_json`).
- Parse errors → claim **fail** with `invalid json: …`.

## Paths

| Input | Meaning |
|-------|---------|
| `.` or empty | Root value |
| `.status` / `status` | Object field `status` |
| `.items.0.name` | `items` → index `0` → field `name` |

Rules:

1. Leading `.` is optional.
2. Segments separated by `.` only (no `['key']` syntax).
3. If the current value is an array, the segment must parse as a decimal `usize` index.
4. Missing field, bad index, or descent into a scalar → path **missing**.

Unsupported (always out of scope for v0.1):

- `[]` wildcards, `..` recursion  
- Filters (`select`, `map`, `?`)  
- Pipes, functions, operators inside the path  
- Multiple values / streaming JSON  

## Operators

### Truthy (default)

```
json PATH
```

Path must exist and the value must be truthy:

| Value | Truthy? |
|-------|---------|
| `null` | no |
| `false` | no |
| `true` | yes |
| number `0` / `0.0` | no |
| other numbers | yes |
| `""` | no |
| non-empty string | yes |
| `[]` / `{}` | no |
| non-empty array/object | yes |

### Exists

```
json PATH exists
```

Path present. `null` is a successful exists.

### Equals

```
json PATH == VALUE
```

Deep equality (`serde_json::Value` equality).

`VALUE` parsing:

1. If the token parses as JSON (`true`, `false`, `null`, numbers, `"quoted"`, arrays/objects), use that.
2. Otherwise treat the raw token as a JSON string (so `healthy` ≡ `"healthy"`).

## Examples

```bash
# {"ok": true, "status": "healthy", "n": 1, "items": [{"id": 7}]}
vet json .ok -- …                    # pass
vet json .status == healthy -- …     # pass
vet json .status == "healthy" -- …   # pass (quoted JSON string token)
vet json .n == 1 -- …                # pass
vet json .missing -- …               # fail: path missing
vet json .status exists -- …         # pass
vet json .items.0.id == 7 -- …       # pass
```

## Evidence strings

| Situation | Example evidence |
|-----------|------------------|
| Missing path | `path .status missing` |
| Not truthy | `not truthy: false` |
| Bad equality | `got "down", expected "healthy"` |
| Invalid body | `invalid json: …` |
| Empty stdout | `empty stdout` |
