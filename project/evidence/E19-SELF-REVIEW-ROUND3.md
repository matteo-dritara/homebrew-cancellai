# E19 review record - third self-review of E19-S02 (read-only private-directory design)

Review-Scope: epic
Round: 5

- Epic: E19 - Desktop Experience
- Stories judged: E19-S02 only (E19-S01 has been `done` since independent round 1)
- Review target: `d5ba09c` (permission changes removed; graceful connection close), examined at
  `d5ba09c`
- Reviewer: Claude (the executor's own model), run as a forked `epic-verifier` context
- Date: 2026-09-23

## Reviewer independence and limits - read this before trusting the verdict

**This is a SELF-REVIEW, not an independent review.** The model that executed the work produced
it. Context isolation keeps the executor's reasoning out of the input; it does not remove
self-preference bias (`docs/development/AGENT_PROTOCOL.md`, "Self-review"). This record changes no
story status: E19-S02 stays `in_progress`.

Limits:

- **This host is macOS.** Nothing Windows-specific ran here. Windows statements are reasoned from
  the code and from Windows/.NET semantics, and each says so.
- **No second local account and no `sudo`.** Every cross-account statement is reasoned from the
  permissions the code checks. None was reproduced.
- This round was narrower in execution than rounds 1-2: it ran the gates listed below and read the
  code and documents. It did not build new swap harnesses or re-run mutation checks. The executor's
  claims that removing the base check or the macOS ACL check fails a test were **not
  independently re-run**.
- At the time of writing, `rust.yml` run 35852263450 (`d5ba09c`) had every job green except
  `quality (windows-latest)`, which was still running. Condition (3) of `CEILING_DECISION.md`
  (two green Windows runs) is therefore **not yet shown**.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E19-S02 | FAIL | SR2-F1 is closed: no non-test code in `cancellai-desktop` changes a permission any more (only tests call `set_permissions`/`icacls`), so a swapped path can no longer redirect a permission change. SR2-F2's repair is in place, but the two green Windows runs are not yet shown. SR2-F3 is only partly closed (`EVIDENCE.md` line 5 is still stale). The new base check does not fully match the guarantee ADR-0038 states: a sticky base is accepted **whatever its owner**, although the owner of a sticky directory can rename entries in it (SR3-F1). The repairs are small and listed below. AC1 and AC2 still hold. |

## Self-review 2 findings - closure check

| Finding | Status | Evidence |
| --- | --- | --- |
| SR2-F1 permission change redirected through a swapped path | **Closed** | `launch.rs` at `d5ba09c`: `PrivateDir::create` makes the directory with `DirBuilder` (mode `0700` on Unix) and then only reads (`symlink_metadata`, `canonicalize`, `/bin/ls -led`, and on Windows a PowerShell script that only reads the security descriptor). `grep` for `set_permissions`, `chmod`, `SetAccessControl`, `Set-Acl` and `icacls` finds them only in tests. ADR-0038's SI-019 sentence is accurate again, because the shell now changes nothing itself |
| SR2-F2 Windows flake (a reset discarded the refusal) | **Repair present, verification pending** | `transport.rs` `close_gracefully`: shut down the write side, then drain at most 64 KiB with a 250 ms timeout per read. Local tests pass. Windows `quality` leg of run 35852263450 still running; a second run is required by the ceiling decision |
| SR2-F3 stale statements | **Partly open** | ADR-0038 and the `windows_acl` module doc are corrected. `project/evidence/E19-S02/EVIDENCE.md` line 5 still reads "Independent verifier: Codex (pending, E19 round 1)", which SR2-F3 named explicitly. Line 107 still names tests that no longer exist (`macos_inherited_access_entries_...`, `windows_private_directories_and_files_grant_only_the_current_user_under_a_broad_parent`). That row is historical, so the stale names are acceptable, but a note saying they were replaced would help a reader |

## Findings

### SR3-F1 - A sticky base is accepted regardless of who owns it (implementation bug, low)

`verify_private` (Unix, `launch.rs:117-119`) accepts the base if `owned_and_closed || sticky`. The
comment and ADR-0038 justify the sticky case with "other users cannot rename or remove this user's
entries". That is true of `/tmp`, which root owns. It is not true in general: the **owner of a
sticky directory** may rename or remove any entry in it. So a sticky base owned by another
non-root account does not provide what the design depends on: "other accounts ... cannot rename
entries in an accepted base" (ADR-0038, Negative/cost).

The default bases are not affected: the per-user macOS `TMPDIR`, root-owned `/tmp`, and
`XDG_RUNTIME_DIR` all pass for the right reason. The gap needs a non-default `TMPDIR` that is a
sticky directory owned by someone else. Reasoned only; not reproduced (no second account).

**Required repair:** accept a sticky base only when it is owned by the current user or by root,
and add a test for the rule. A true positive test needs a second owner, so a unit test of the
owner rule as a pure function is enough. Correct the comment in `launch.rs` and ADR-0038 so they
state the owner condition.

### SR3-F2 - "The current user" is inferred from the new directory, not from the process (spec/comment gap, low)

`verify_private` takes `me` from the owner of the directory it just created, and then compares
the base's owner with that. The doc comment says "owned by the same user as the process". The two
are the same unless the directory at that path is no longer the one this process created. The
design's own argument (nobody else can rename entries in an accepted base) makes that
circular in the one case where it matters. No default configuration is affected (reasoned).

**Required repair:** either compare against the process's own identity, obtained by a means
ADR-0019 allows, or rewrite the comment to state what is actually checked and why that is
sufficient.

### SR3-F3 - Windows check: what it does not look at (reasoned, not run; low)

The PowerShell script (`launch.rs:189-197`) is read-only and fails closed on every error path read
here: `$ErrorActionPreference='Stop'`, a missing path gives exit 4, a reparse point exit 5, any
foreign Allow rule exit 3, and a failure to start PowerShell is an I/O error that refuses. It does
not check:

- the **owner** of the base or of the new directory. An object's owner keeps implicit rights to
  change its DACL, so a base owned by another non-admin account is not private even if its current
  rules look private;
- ACE types that .NET's `GetAccessRules` does not return. My recollection of the .NET Framework
  source is that callback (conditional) ACEs are skipped. **This was not verified on Windows.**

Neither applies to the default per-user `%TEMP%`. `powershell.exe` is also found through the
search path rather than an absolute `%SystemRoot%` path. That is a same-user/administrator-trust
matter, recorded as a residual.

**Required repair:** add an owner check (the current user, `SYSTEM` or `Administrators`) for the
base and the directory. Either confirm on a Windows host how conditional ACEs are reported, or
disclose the limit in ADR-0038.

## Other axes examined

| Axis | Observed |
| --- | --- |
| Any remaining permission change by path | None in non-test code (see SR2-F1) |
| Base canonicalization | The Unix base is canonicalized and `lstat`ed; the directory itself is `lstat`ed (not followed). Only the leaf base is checked, not its ancestors. For the default bases every ancestor is owned by root or the user. Recorded as a residual: a custom base under an ancestor another account can write is outside what the check sees |
| macOS `ls -led` parsing | Unchanged since self-review 2, which found it fails closed on a split first line. It is now used only to refuse, never to confirm a change it made |
| Graceful close: can it hang? | Each drain read is bounded at 250 ms and the total is bounded at 64 KiB, but total time is not capped: a client that trickles bytes keeps the single-threaded server busy for as long as the byte limit allows. This is the same class as the existing `IDLE_TIMEOUT` (a per-read bound on a single-threaded server, reachable by any local process before authentication). It adds no new capability; recorded as a residual. A total drain deadline would close it cheaply |
| Token leakage | Code paths unchanged since self-review 2 for stdout/stderr/argv (`main.rs` prints only the port and the URL-file path; the opener gets the launcher path). Not re-run this round |
| No mutation path | `cancellai-desktop`'s only cancellai dependency is still `cancellai-desktop-api`; the shell no longer changes permissions itself |
| CLI parity (AC2) | Unchanged; `cancellai-cli/tests/desktop_api.rs` not re-run this round |
| Headless core (AC1) | Unchanged |

## Is "same-user attackers are out of scope" honest?

Yes, as far as it goes. A process running as the same user can already read the user's files and,
on most desktop systems, the dashboard's memory, so a private directory cannot defend against it.
The statement is honest now that the dashboard changes no permission, because a same-user swap
can no longer turn the dashboard into a tool that re-permissions some other directory. That was
the part self-review 2 found dishonest. The companion claim, that **other** accounts cannot rename
entries in an accepted base, is where the ADR currently overstates the code (SR3-F1, and on Windows
SR3-F3's owner point).

## Gates run

| Gate | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` (macOS) | PASS |
| `cargo test -p cancellai-desktop -p cancellai-desktop-api` | PASS (all suites ok) |
| CI `rust.yml` run 35852263450 at `d5ba09c` | all jobs green except `quality (windows-latest)`, still in progress at writing |
| Not run this round | workspace-wide `cargo test`, Windows/Linux-target clippy, `cargo deny check`, mutation checks, the Python gate set beyond `process_metrics.py`/`check_process.py` |

## Documents opened

`AGENTS.md`; `CLAUDE.md`; `project/evidence/E19-SELF-REVIEW-ROUND2.md`;
`project/evidence/E19-S02/CEILING_DECISION.md`; `project/evidence/E19-S02/EVIDENCE.md` (header and
the `d5ba09c` diff); `docs/adrs/0038-the-desktop-shell-is-a-loopback-dashboard-over-the-desktop-api.md`
(Context, Decision, Consequences, Safety impact); `CHANGELOG.md` and
`project/evidence/RELEASE-v1.19.0.md` (the `d5ba09c` diff); the verifier brief from
`python3 scripts/project_os.py brief E19-S02 --role verifier`;
`rust/crates/cancellai-desktop/src/launch.rs` (whole); `rust/crates/cancellai-desktop/src/main.rs`
(whole); `rust/crates/cancellai-desktop-api/src/transport.rs` (whole). Not opened this round:
`docs/security/SAFETY_INVARIANTS.md`, `docs/security/THREAT_MODEL.md`,
`docs/development/AGENT_PROTOCOL.md`, `session.rs`, `server.rs`, `render.rs`.

## Round verdict: FAIL (self-review)

The read-only redesign closes SR2-F1's class: the dashboard no longer changes any permission, so
it cannot be redirected into changing someone else's. The remaining gaps are narrow and not
reachable in default configurations. However, the base check accepts a case (a sticky base owned
by another account) that contradicts the guarantee ADR-0038 relies on. SR2-F3 is not fully closed,
and condition (3) of the ceiling decision is not yet met. Required before this story can close:
SR3-F1's owner rule and test; SR3-F2's comment or check; SR3-F3's Windows owner check (or a
disclosure in the ADR); the stale `EVIDENCE.md` header; and two green Windows runs. Status is
unchanged at `in_progress`.
