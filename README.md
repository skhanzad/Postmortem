# Postmortem

**Catch AI coding agents cheating on your tests before you commit.**

Postmortem checks the changes made by AI coding agents (Claude Code, Cursor, Copilot, Codex, and others) for deleted tests, weakened assertions, out-of-scope edits, and risky shortcuts. It runs locally, it's fast, and the core checks need no AI and no API key.

> 🚧 **Status:** Early development. Features marked *(planned)* are not built yet.

<!-- Add a GIF demo here: an agent "fixes" a failing test by deleting it → Postmortem blocks the commit -->

---

## Why

AI agents are great at making tests pass. Sometimes they do it the wrong way: deleting the failing test, loosening an assertion, or hardcoding the expected value. These changes are easy to miss in a 40-file diff. Postmortem flags them so you review what matters.

## What it catches

| Category | Examples |
|---|---|
| 🧪 **Test tampering** | Deleted tests, new `skip`/`xfail`, weakened assertions, lowered coverage thresholds |
| 🎯 **Scope creep** *(planned)* | Edits to CI config, lockfiles, or unrelated files during a narrow task |
| 🩹 **Shortcuts** *(planned)* | Hardcoded return values, `# type: ignore`, `eslint-disable`, `except: pass` |
| ⚠️ **Risky changes** *(planned)* | New secrets, new dependencies, deleted migrations, auth code edits |

## Quick start

```bash
# Install (planned)
pip install postmortem-ci

# Check your uncommitted changes
postmortem check

# Check a branch against main
postmortem check --base main
```

### Example output

```
$ postmortem check

🚨 2 problems, 1 warning

✖ Test deleted
  tests/test_auth.py::test_expired_token
  Removed in this change. The task did not ask for this.

✖ Assertion weakened
  tests/test_cart.py:42
  - assert total == 59.99
  + assert total is not None

⚠ Out of scope
  .github/workflows/ci.yml changed
  Task: "fix checkout button color"

Run `postmortem explain` for an AI summary (optional)
```

Postmortem exits with a non-zero code when it finds problems, so it can block commits and CI.

## Pre-commit hook *(planned)*

```yaml
# .pre-commit-config.yaml
repos:
  - repo: https://github.com/postmortem/postmortem
    rev: v0.1.0
    hooks:
      - id: postmortem
```

## GitHub Action *(planned)*

```yaml
- name: Postmortem
  uses: postmortem/postmortem@v1
  with:
    base: main
```

## MCP server *(planned)*

Let your agent check its own work before handing it back:

```json
{
  "mcpServers": {
    "postmortem": {
      "command": "postmortem",
      "args": ["mcp"]
    }
  }
}
```

## Configuration

Create a `postmortem.yml` in your repo root:

```yaml
strictness: normal        # relaxed | normal | strict

protected_paths:
  - .github/workflows/**
  - migrations/**

allow:
  - rule: test-deleted
    path: tests/legacy/**

task_scope: true          # compare changes to the task description (uses an LLM)
model: ollama/llama3      # local by default, nothing leaves your machine
```

## How it works

1. **Read the diff:** compares your changes against a base (working tree, branch, or PR).
2. **Parse the code:** builds syntax trees with Tree-sitter, so it understands tests and assertions instead of just matching text.
3. **Run rules:** fast, free, deterministic checks. This is the core of Postmortem.
4. **Optional AI check:** compares the diff with the task description to flag scope creep. Runs locally with Ollama by default.

## Supported languages

- [x] Python (pytest, unittest)
- [ ] JavaScript / TypeScript (Jest, Vitest)
- [ ] Go
- [ ] Java (JUnit)

## Benchmark *(planned)*

Postmortem is evaluated on a hand-labeled dataset of real agent-written pull requests. Precision, recall, and the dataset itself will be published here.

## Roadmap

1. CLI for Python: deleted tests and weakened assertions
2. Pre-commit hook and GitHub Action
3. Shortcut and risky-change rules
4. Scope-creep check with a local LLM
5. MCP server
6. JavaScript/TypeScript support
7. Public benchmark and research paper

## Contributing

Contributions are welcome, especially new rules and language support. Check issues labeled **good first issue**.

```bash
git clone https://github.com/postmortem/postmortem.git
cd postmortem
make install   # uv sync: venv + Rust build + dev deps
make test
```

## License

MIT