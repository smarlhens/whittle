# whittle

Lint and auto-normalize [Conventional Commit](https://www.conventionalcommits.org/) subjects. Plugs into [`pre-commit`](https://pre-commit.com/), [`prek`](https://github.com/j178/prek), [`husky`](https://typicode.github.io/husky/), and [`lefthook`](https://github.com/evilmartians/lefthook) as a `commit-msg` hook.

`whittle` parses a commit message, applies a configurable set of transforms (lowercase, strip noise chars, replace `and` with `&`, normalize scope separators, …), drops body + trailers, and finally validates the result against your rules. If the message can't be brought into compliance, the hook fails.

## Install

### pre-commit / prek

Add to your `.pre-commit-config.yaml`:

```yaml
repos:
  - repo: https://github.com/smarlhens/whittle
    rev: v0.1.0
    hooks:
      - id: whittle-fix
```

Then install the `commit-msg` stage hook:

```sh
prek install --hook-type commit-msg
# or
pre-commit install --hook-type commit-msg
```

### husky

Install the [npm wrapper](./npm) (downloads the matching binary on install):

```sh
npm install --save-dev @smarlhens/whittle
npx husky add .husky/commit-msg 'npx whittle fix "$1"'
```

### lefthook (`lefthook.yml`)

```sh
npm install --save-dev @smarlhens/whittle
```

```yaml
commit-msg:
  commands:
    whittle:
      run: npx whittle fix {1}
```

### Cargo / Homebrew / direct binary

```sh
# Rust toolchain
cargo install whittle

# Or download a prebuilt binary
# https://github.com/smarlhens/whittle/releases
```

Once `whittle` is on `$PATH`, wire it up manually:

```sh
# .husky/commit-msg or .git/hooks/commit-msg
#!/bin/sh
whittle fix "$1"
```

## Defaults

Out of the box, `whittle-fix` applies these transforms to the commit subject:

| Component | Transform |
|-----------|-----------|
| type | lowercase |
| scope | lowercase, `/` → `-` |
| description | lowercase, `and` → `&`, strip `/ \ [ ] { }`, strip standalone dots (keep version dots like `1.2.3`), strip trailing dot, collapse whitespace |
| body | dropped |
| footers | dropped (incl. `Co-Authored-By`) |

Validation rules:

- Must parse as Conventional Commits.
- Type must be one of: `feat, fix, refactor, perf, docs, test, chore, build, ci, style, revert`.
- Subject ≤ 72 characters.

## Configuration

Point `whittle` at a `whittle.toml`:

```yaml
hooks:
  - id: whittle-fix
    args: [--config, whittle.toml]
```

Example `whittle.toml` that mirrors the defaults:

```toml
[scope]
lowercase = true
replace = [{ from = "/", to = "-" }]

[description]
lowercase = true
collapse_whitespace = true
trailing_dot = "strip"             # keep | strip
strip_chars = ["/", "\\", "[", "]", "{", "}"]
internal_dots = "keep_in_numbers"  # all | none | keep_in_numbers
replace = [{ from = '\band\b', to = "&", regex = true }]

[body]
keep = false

[footers]
keep = false
deny = ["Co-Authored-By", "Co-authored-by"]

[rules]
max_subject_length = 72
require_conventional = true
allowed_types = [
  "feat", "fix", "refactor", "perf", "docs", "test",
  "chore", "build", "ci", "style", "revert",
]
```

## CLI

```sh
whittle check <file>   # validate only, exit 1 on violation
whittle fix   <file>   # apply transforms in place, then validate
whittle --config whittle.toml fix .git/COMMIT_EDITMSG
```

## Why not commitlint / conventional-pre-commit?

- `commitlint` requires Node + complex config.
- `compilerla/conventional-pre-commit` validates but cannot rewrite. You must hand-fix every message.

`whittle` does both, ships as a single static binary, and runs under either `pre-commit` or `prek`.

## License

[Blue Oak Model License 1.0.0](LICENSE.md).
