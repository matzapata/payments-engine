# Review Staged Changes

Review all staged git changes against the project design (`README.md` and `.cursor/rules/`), find issues, suggest improvements, and output a conventional commit message.

## Steps

1. **Gather staged changes**
   - Run `git diff --staged --stat` and `git diff --staged`
   - Run `git status` for context (unstaged files, branch)
   - If nothing is staged, tell the user to stage files first and stop

2. **Run verification** (when Rust sources are staged)
   - `cargo fmt --check` — note formatting drift
   - `cargo clippy -- -D warnings` — if the project compiles
   - `cargo test` — full suite when changes touch domain, application, or integration paths

3. **Review against project standards**
   - Layer boundaries: `presentation → application → domain ← infrastructure`
   - Domain has no I/O, serde, or csv dependencies
   - Fixed-point money only (`Amount` / scaled `i64`); no `f64` for balances
   - Invariant `available + held == total` preserved after every transition
   - Partner-error semantics: invalid dispute/resolve/chargeback silently ignored; withdrawal insufficient funds is no-op; locked accounts reject all further txs
   - Streaming CSV processing — no loading entire file into memory
   - Tests match the layer being changed (domain unit tests highest ROI)

4. **Classify findings**
   - **Critical** — correctness bugs, invariant violations, layer violations, missing error handling for fatal paths
   - **Warning** — missing tests for new behavior, unclear naming, avoidable complexity
   - **Nit** — style, minor readability, optional refactors

5. **Output report** using this structure:

---

## Summary

One or two sentences describing what the staged changes accomplish.

## Issues

### Critical
- (none) or bullet list with `file:line` when possible

### Warnings
- bullet list

### Nits
- bullet list

## Suggestions

Actionable improvements only — concrete code or test changes, not vague advice.

## Suggested commit message

```
type(scope): imperative summary in present tense

Optional body: explain why the change was made, not what files changed.
Keep subject ≤72 characters. Use Conventional Commits types:
feat, fix, refactor, test, docs, chore, perf.
Scope examples: domain, ledger, csv, cli, application.
```

Also provide a one-line copy-paste subject if a body is not needed.

---

6. **Do not commit** unless the user explicitly asks in a follow-up message.
