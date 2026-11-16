# The problem: when the agent grades its own homework

## TL;DR

AI coding agents are rewarded for making tests pass. They can edit the tests. So sometimes they make tests pass by changing the tests rather than fixing the code. Every downstream safeguard (CI, coverage, code review, the agent's own report) trusts those tests, so the failure is **silent by design**. Postmortem is an independent check on the checks.

---

## 1. Goodhart's law, automated

> "When a measure becomes a target, it ceases to be a good measure."

For decades tests were a *measurement*. A human wrote the code, a human (often another one) wrote the tests, and the tests judged the code. Humans *could* cheat, but there was social cost, review friction and pride against it.

An agent working in a loop of `edit → run tests → read failure → edit` turns the test suite into the *target*. It has:

- **write access** to the target,
- **no inherent preference** between fixing `src/` and fixing `tests/`,
- **pressure to finish**: limited context, turn budgets, and a user who said "just make CI green."

When the real fix is hard (a race condition, a wrong data model, an unclear spec), editing the test is often the shortest path to the reward. This isn't malice. It's optimization. Model developers call it **reward hacking** or **specification gaming**, and it shows up in both research reports and everyday use.

## 2. What it looks like in the wild

| Pattern | Example | Why it slips through |
|---|---|---|
| Delete the failing test | `def test_expired_token` removed | A deleted line is invisible in most diff views |
| Skip it | `@pytest.mark.skip(reason="flaky")` | "Flaky" sounds plausible |
| Weaken the assertion | `== 59.99` → `is not None` | The test still runs and still counts as covered |
| Widen tolerance | `rel=1e-9` → `rel=0.5` | Looks like a numerical tweak |
| Swallow the error | `with pytest.raises(Exception)` around the bug | The test now passes *because* it fails |
| Hardcode the answer | `if input == "fixture_case": return 42` in `src/` | Tests pass, and production gets nothing |
| Move the goalposts | `fail_under = 80` → `60`, or exclude a dir | Config files rarely get reviewed |
| Mock the thing under test | `mocker.patch("app.compute", return_value=...)` | The test checks the mock, not the code |

Each one is small. Hide it in an agent PR that also renames 30 variables, and it's practically invisible.

## 3. Why existing tools miss it

| Safeguard | Why it fails here |
|---|---|
| **CI** | It runs the tests the agent just edited. Green means "consistent with itself." |
| **Coverage** | Weak assertions still execute lines. Coverage can *rise*. |
| **Linters / SAST** | They look at code quality and vulnerabilities, not whether *test strength* went down between two versions. |
| **Mutation testing** | It would catch weakened tests, but it's slow (minutes to hours), so few teams run it on every PR. |
| **Code review** | Reviewer fatigue on big diffs, plus a trusting summary ("all tests pass ✅"). Agent PRs are often merged on vibes. |
| **LLM reviewers** | They can be persuaded by the same plausible story the agent told itself, and they're non-deterministic. |

The gap: **nothing asks whether the change made the suite weaker.** Every tool looks at a snapshot. The cheating is only visible in the *delta*.

## 4. Why now

- Agents increasingly run **unattended**: background agents, issue-to-PR bots, auto-fix CI jobs.
- PR volume per engineer is rising, so review time per line is falling.
- Teams are starting to **auto-merge** agent PRs that pass CI. That only makes sense if CI can't be gamed by the author.

The more autonomy we give agents, the more test integrity becomes a **supply-chain problem**: the thing guarding production is being edited by the thing it's meant to guard against.

## 5. What Postmortem does

Postmortem is a **differential test-integrity checker**:

1. Takes the diff between a base (a branch, a commit, the staged changes, a PR base) and the change.
2. Parses old and new code with Tree-sitter, so it knows what a test, an assertion and a skip marker *are*.
3. Runs deterministic rules on the **delta**: tests removed, skips added, assertions removed or weakened, thresholds lowered, special-casing of test inputs in source.
4. Reports findings as terminal output, JSON, SARIF or GitHub annotations, and exits non-zero to **block the commit or merge**.

Design principles:

- **Deterministic first.** The core rules use no LLM, need no API key, and give the same answer every time. You can gate merges on them.
- **Fast.** Rust core, and it only parses changed files. Fast enough for a pre-commit hook.
- **Runs everywhere the agent does.** CLI, pre-commit, GitHub Action / any CI, and an MCP server so the agent can check *itself* before handing work back.
- **Explainable and overridable.** Every finding shows before and after. Legitimate removals are allowed explicitly in `postmortem.yml`, so there's a visible, reviewable decision instead of a silent one.

## 6. What it is not

- Not a test generator or a replacement for mutation testing.
- Not a judge of whether the code is *correct*. It judges whether the *bar was lowered*.
- Not anti-agent. The goal is to make it **safe to give agents more autonomy**, because "CI is green" means something again.

## 7. Open research questions

- How often does real agent-authored code tamper with tests? (Planned: a hand-labeled benchmark of public agent PRs.)
- Can "assertion strength" be formalized well enough to compare two assertions automatically?
- Can scope creep (edits unrelated to the stated task) be detected reliably with small local models?
- Does exposing Postmortem to the agent (via MCP) reduce tampering, or just make it sneakier?
