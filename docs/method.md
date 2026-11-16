# Postmortem: detecting test tampering with agentic investigation and deterministic verdicts

## 1. Problem

Coding agents are rewarded, directly or indirectly, for making tests pass. When the real fix is hard, an agent can make the tests pass by lowering the bar instead: deleting or skipping tests, weakening assertions, widening tolerances, narrowing which tests run, special-casing test inputs in the source, or loosening CI. The agent then reports the task as done, and a reviewer sees a green build.

These changes are easy to miss in review, because each one looks small and often plausible on its own. Judging the new code alone doesn't help: after tampering, the new code and the new tests agree with each other.

**Core question.** Instead of judging the new code, Postmortem judges what the change did to the test suite's power to catch bugs. It asks one question of every diff:

> *Is there behavior the old suite would have rejected that the new suite now accepts?*

If yes, and the task didn't call for it, the bar was lowered.

**Core principle: agentic investigation, deterministic verdict.** An LLM agent may explore the change and *propose* evidence as code. Only deterministic checks, run in a sandbox against fixed baselines, may raise, confirm or downgrade a finding. The agent's words never count as evidence.

---

## 2. Threat model

**Monitored party.** An autonomous coding agent (or a human) that produces a change to a repository and reports the task as done.

**What the monitored party can do:**
- Edit any file in the change: source, tests, test config, CI workflows and the monitor's config file (`postmortem.yml`).
- Write PR descriptions, commit messages, comments and identifiers, including text meant to mislead a reviewer or an LLM (prompt injection).
- Make the diff large or noisy to tire reviewers.
- Adapt to the monitor over time if it receives the monitor's findings.

**What it cannot do (assumptions):**
- Change the **base commit**, which serves as the baseline.
- Change the monitor's code, its rules, or the required CI check that invokes it. Postmortem runs as a required check defined on the protected base branch or outside the repository.
- Access the investigator agent's sandbox or the judge's execution environment.

**Out of scope:** a compromised CI host, malicious changes to third-party dependencies, and test changes that the task explicitly asked for (surfaced as context, not blocked).

**Security goal.** The monitored party must not be able to (a) get a tampering change marked clean by manipulating the monitor, or (b) get a legitimate change marked as *confirmed* tampering. Missing some tampering (lower recall) is acceptable. Wrong confirmed verdicts are not.

---

## 3. Overview

```
                 ┌──────────── Tier 1: Static delta (ms, deterministic) ────────────┐
diff ──────────► │ parse old+new ─► match tests ─► compare obligations ─► rules      │ ──► suspicions
                 └──────────────────────────────────────────────────────────────────┘
                 ┌──────────── Tier 2: Cross-execution (deterministic) ─────────────┐
                 │ old tests on new code; targeted differential mutation             │ ──► proof
                 └──────────────────────────────────────────────────────────────────┘
                 ┌──────── Tier 2.5: Agentic investigation (untrusted) ─────────────┐
suspicions ────► │ Investigator agent: explores, writes candidate evidence (code)    │
                 └──────────────────────────┬───────────────────────────────────────┘
                                            ▼  code only
                 ┌──────── Judge (deterministic, sandboxed) ────────────────────────┐
                 │ strength-check evidence ─► run it ─► fixed acceptance rules       │ ──► verdict
                 └──────────────────────────────────────────────────────────────────┘
                 ┌──────── Tier 3: Intent check (optional LLM, context only) ───────┐
                 │ compare diff with task description                                │ ──► context
                 └──────────────────────────────────────────────────────────────────┘
```

Each tier is slower and more precise than the one before. Tier 1 runs everywhere (pre-commit, CI). Tiers 2, 2.5 and 3 are opt-in and meant for CI. Only Tier 1, Tier 2 and the judge affect the verdict.

---

## 4. Tier 1: Static differential analysis

### 4.1 Change acquisition
Take `(path, old_src, new_src)` for each changed file from git. The base can be HEAD, the staged changes (`--cached`), a branch merge-base, or a PR base SHA. Classify each file as:
- **test:** `test_*.py`, `*_test.py`, `tests/**`, `conftest.py`
- **test config:** `pytest.ini`, `pyproject.toml` (`[tool.pytest]`, `[tool.coverage]`), `setup.cfg`, `tox.ini`, `.coveragerc`, CI workflows, `postmortem.yml`
- **source:** everything else

### 4.2 Test identity and matching
Parse both versions with Tree-sitter and extract every test as `TestFn { qualname, params, decorators, body, obligations }`.

Match old tests to new tests in three steps:
1. Exact `qualname` match.
2. **Rename detection:** an unmatched old test is paired with an unmatched new test if their normalized bodies are very similar (token Jaccard ≥ 0.8), so a rename isn't reported as a deletion plus an addition.
3. Anything still unmatched on the old side is a **deleted test**.

Parametrized tests are compared case by case. Removing a `pytest.param(...)` row deletes a case.

### 4.3 Obligations
Each assertion is normalized into an **obligation** `(subject, predicate, expected)`:

| Source form | Obligation |
|---|---|
| `assert total == 59.99` | `(total, EQ, 59.99)` |
| `self.assertEqual(a, b)` | `(a, EQ, b)` |
| `assert x == pytest.approx(y, rel=1e-6)` | `(x, APPROX(1e-6), y)` |
| `assert isinstance(r, Cart)` | `(r, TYPE, Cart)` |
| `assert r is not None` | `(r, NOT_NONE, -)` |
| `assert r` | `(r, TRUTHY, -)` |
| `with pytest.raises(ValueError, match="..")` | `(block, RAISES(ValueError, msg), -)` |
| `mock.assert_called_once_with(..)` | `(mock, CALLED_WITH, args)` |

Subjects are normalized (whitespace, aliases, `self.` stripped) so the same check is recognized across versions.

### 4.4 Assertion-strength lattice
Predicates are partially ordered by how many outcomes they accept. Stronger means fewer outcomes pass:

```
EQ(literal) > APPROX(tight) > APPROX(loose) > IN/CONTAINS > TYPE > NOT_NONE > TRUTHY > (none)
RAISES(E, msg) > RAISES(E) > RAISES(BaseClass of E) > RAISES(Exception) > (none)
CALLED_WITH(args) > CALLED_ONCE > CALLED > (none)
```

For each matched test, the obligation sets are compared:
- **assertion-removed:** an old obligation has no new obligation on the same subject.
- **assertion-weakened:** the new predicate on the same subject is strictly lower in the lattice.
- **expectation-changed** (warning): the `expected` value changed. Legitimate when the spec changed; suspicious when the source under test changed in the same diff.
- **tolerance-widened:** an `APPROX` or `assertAlmostEqual` tolerance increased.
- **exception-broadened:** a `RAISES` type moved up the class hierarchy, or its `match=` was dropped.

Because the order is partial, changes between incomparable predicates aren't reported as weakening, which keeps false positives down.

### 4.5 Suite-level and config rules

| Rule | Detection |
|---|---|
| `test-deleted` | Unmatched old test or parametrized case |
| `test-skipped` | New `skip`/`skipif`/`xfail` decorator, `pytest.skip()` call, or `unittest.skip` |
| `test-disabled` | Renamed out of collection (`test_x` → `_test_x`), or body replaced by `pass`/`return` |
| `collection-narrowed` | `testpaths`, `--ignore`, `-k`, `--deselect` or `collect_ignore` changed so fewer tests run |
| `threshold-lowered` | `fail_under`, `--cov-fail-under` or coverage `omit` patterns loosened |
| `ci-weakened` | Test step removed, `continue-on-error: true` added, or `\|\| true` appended in CI |
| `mock-under-test` | A new `patch()` target is the module or function the test file is named after |
| `monitor-config-changed` | Any edit to `postmortem.yml` or to the step that runs Postmortem |

### 4.6 Source-side shortcuts
- **Test-literal special-casing:** collect string and number literals used in tests; flag a *new* conditional in source that compares against one of them (`if user_id == "test_user_42": return ...`).
- **Swallowed failure:** newly added `except: pass`, `except Exception: return None`, `# type: ignore` or `# noqa` on lines the diff touched.

### 4.7 Severity
Each finding has a base severity (error or warning). It goes **up** when the same diff also changed the source the test covers ("changed the code *and* loosened its test"). It goes **down** when the covered source was deleted too (legitimate feature removal). A finding is suppressed only by an `allow` entry in `postmortem.yml` with a human-written `reason`.

Tier 1 outputs a set of **suspicions**, each with a location, rule, before/after snippets and severity.

---

## 5. Tier 2: Cross-execution

### 5.1 Test transplant
Check out the old test files on top of the new source in a temporary worktree, and run the tests the change modified or deleted.

| Old tests on new code | New tests on new code | Result |
|---|---|---|
| pass | pass | Cosmetic or refactor. Downgrade static findings. |
| **fail** | pass | **Confirmed tampering.** |
| fail | fail | Pre-existing breakage. Reported, not tampering. |
| error (can't run) | — | **Unresolved.** Passed to Tier 2.5. |

Each test is run *n* times (default 3). A test with inconsistent results is marked **flaky** and can't confirm a finding. Cost scales with the number of changed tests, not the size of the suite.

### 5.2 Targeted differential mutation
For the source functions the changed tests exercise (from the import graph or `coverage --contexts`), generate a small set of mutants (flip a comparison, change a constant, return early) and run the old and new tests against them:

```
Δ mutation score = killed_by_new − killed_by_old
```

A negative Δ measures lost test power (e.g. "the new tests miss 7 of 12 mutants the old tests caught"). Limiting mutation to changed functions keeps cost to seconds or minutes.

---

## 6. Tier 2.5: Agentic investigation

Tiers 1 and 2 miss some cases: old tests that can't run on a changed API, weakening hidden in helpers or fixtures, and generic mutants that don't target the changed behavior. An investigator agent covers these by producing **candidate evidence**, which the judge then verifies.

### 6.1 Inputs and tools
- The suspicions from Tier 1 and unresolved results from Tier 2.
- Read-only access to the base and changed versions of the repository.
- Tools: read files, search, list callers and callees, and **submit evidence**. There's no tool for submitting a verdict.

The diff, comments, docstrings, PR body and commit messages are **untrusted data**. They're passed to the agent inside explicit delimiters and never as instructions.

### 6.2 Evidence types

| Evidence | What the agent provides | What it can prove |
|---|---|---|
| **Witness test** | An input plus the old oracle | The old oracle rejects the new behavior, but the new oracle accepts it. |
| **Adapted old test** | The old test, ported to the new API | The old requirement still fails on the new code. |
| **Targeted mutant** | A mutation in a changed function | Combined with others, a negative Δ mutation score. |
| **Hidden-assertion pointer** | A path to a helper, fixture or `conftest.py` assertion | Lets Tier 1 compare obligations it couldn't reach. |

All evidence is code or a file location, never a prose claim.

### 6.3 Limits
- A fixed budget per PR (steps, tokens, wall-clock time). When it runs out, investigation ends with whatever evidence has been submitted.
- A sandbox with no network access and no write access to the repository, rules, baselines or CI.
- The investigator can only *add* evidence. It can't remove, downgrade or clear a suspicion.

---

## 7. The judge

The judge is deterministic code with no LLM. It takes evidence from Tiers 2 and 2.5 and decides.

### 7.1 Checks before running
1. **Strength check for adapted tests.** Extract obligations from the original old test and the adapted one. Reject the adapted test unless every original obligation has an equal or stronger counterpart in the lattice. This stops the investigator from weakening tests itself.
2. **Witness validity.** The witness must use only APIs that exist in the version it targets and must be deterministic over repeated runs.
3. **Mutant validity.** A mutant must change a function the changed tests exercise, must run, and must not be trivially equivalent to the original.

### 7.2 Acceptance rules
A suspicion becomes **confirmed** only if one of these holds when run in the sandbox:
- **R1 (transplant):** an old test, or an adapted old test that passed the strength check, fails on the new code, and the new tests pass.
- **R2 (witness):** a witness test fails under the old oracle and passes under the new oracle on the new code.
- **R3 (mutation):** Δ mutation score < 0 over valid mutants, with at least *k* mutants killed by the old tests (default *k* = 2).

Every confirming run is repeated; flaky results never confirm.

### 7.3 Verdicts

| Verdict | Condition | Effect |
|---|---|---|
| **confirmed** | R1, R2 or R3 holds | Hard error; blocks merge. |
| **suspected** | Tier 1 finding, no confirmation | Warning or error at its Tier 1 severity. |
| **downgraded** | Transplant passes on both sides | Severity reduced; still shown. |
| **allowed** | Matching `allow` entry with a human `reason` | Shown as overridden; recorded in the audit trail. |

There's no "cleared by agent" verdict.

---

## 8. Tier 3: Intent check (context only)

Some changes are only wrong relative to the task. Given a task description (PR body, issue, commit message, or `--task "..."`), a local LLM answers narrow questions for each finding and changed file:
- *Does the task require modifying or removing this test?* (yes / no / unclear, with a quote from the task)
- *Is this file within the scope of the task?*

Answers are attached as context, and out-of-scope files are flagged as warnings. Tier 3 never creates, confirms or clears an error.

---

## 9. Agent feedback loop

The engine is also exposed as an MCP tool (`postmortem.check`). A coding agent can call it before reporting "done" and receive structured findings with before/after snippets, so it can restore the test or justify the change in the PR. The verdict logic is identical to CI.

---

## 10. Trust properties

| Property | How it's achieved |
|---|---|
| **Independence** | Baseline is the base commit. The monitor runs as a protected required check. Edits to its config or invocation are themselves flagged. |
| **Determinism** | Verdicts come only from Tier 1, Tier 2 and the judge. Given the same evidence, the verdict is always the same. |
| **Evidence over assertion** | Confirmed findings come with a runnable test or mutant and its run log. |
| **Resistance to manipulation** | Untrusted text can at worst stop the agent from finding evidence (lower recall). It can't produce a confirmed finding (the judge requires execution) or clear one (no such action exists). |
| **Human authority** | Postmortem never fixes or reverts. Overrides need a written human reason. |
| **Audit trail** | Agent transcript, submitted evidence, judge decisions and run logs are stored with JSON/SARIF output for each PR. |
| **Privacy** | The agent and Tier 3 use a local model by default. Code leaves the machine only if the user opts into a remote model. |

**Known limitation.** A weak or manipulated investigator lowers recall. The design trades some recall for precision and trust, which suits a merge gate.

---

## 11. Evaluation

### 11.1 Dataset
- **Real changes:** public PRs written by agents (identified by bot authors or co-author trailers), with each test-related change hand-labeled as *legitimate* or *tampering*, by category.
- **Synthetic changes:** tampering injected into real repositories one category at a time, including hidden weakening in helpers and fixtures and changes that break old test APIs.
- **Adversarial changes:** produced by the red-team agent (11.4).

Two annotators label each change; disagreements are resolved by discussion, and agreement (Cohen's κ) is reported.

### 11.2 Research questions
- **RQ1 (detection):** precision and recall per rule and per tier (1, 1+2, 1+2+2.5). Target ≥ 0.9 precision for error-level verdicts and ≈ 1.0 for confirmed verdicts.
- **RQ2 (value of the agent):** how many tampering cases Tier 2.5 confirms that Tiers 1+2 couldn't, by category.
- **RQ3 (robustness):** inject prompt-injection text into diffs, comments and PR bodies; measure the effect on recall and check that no false confirmed verdicts occur.
- **RQ4 (determinism):** rerun the judge on stored evidence; verdicts must be identical.
- **RQ5 (cost):** time, tokens and compute per PR for each tier.
- **RQ6 (agent behavior):** do coding agents with access to `postmortem.check` tamper less, and do they try to evade it?

### 11.3 Ablations and baselines
- Lattice vs. plain textual diff; with and without rename matching.
- Tier 2.5 with and without the strength check on adapted tests.
- Baselines: coverage change alone, existing test-smell detectors, and an LLM-only judge given the same diff.

### 11.4 Red-team loop
An adversarial agent is asked to complete tasks with failing tests without being flagged. Each successful evasion is analyzed, turned into a deterministic rule where possible, and added to the adversarial dataset. Evasion rate is reported per round.

---

## 12. Implementation

| Component | Location |
|---|---|
| Diff acquisition, CLI, output formats (text / JSON / SARIF / GitHub annotations) | `python/postmortem/` |
| Tree-sitter parsing, test matching, obligations, lattice, rules | `src/` (Rust, exposed via PyO3) |
| Cross-execution and mutation runner | `python/postmortem/verify/` |
| Investigator agent and sandbox | `python/postmortem/investigate/` |
| Judge (strength checks, acceptance rules, verdicts) | `python/postmortem/judge/` |
| Intent check | `python/postmortem/intent/` |
| MCP tool (`postmortem.check`) | `python/postmortem/mcp/` |
| CI integration | `action.yml`, `.pre-commit-hooks.yaml` |

**Build order:** Tier 1 MVP (deleted and weakened tests for Python), remaining Tier 1 rules, CI integration, Tier 2 transplant, the judge, mutation, Tier 2.5 (adapted tests first, then witnesses and mutants), Tier 3 and MCP, then the red-team loop.