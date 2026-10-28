Question: can the owner free disk space to at least 70 GB and resume validation, or should this remain an unfinished draft? Recommend freeing space and resuming; common.md requires stopping below 50 GB, and only 45 GB is free. heavy requires 70 GB before starting this build.

T3a is incomplete. No commit was made and nothing was pushed. The draft remains visible on `docs-reference/uses` in the requested worktree; git status has three modified tracked files and two new files. It must not be landed as validated work.

Implemented draft:

- `crates/temper-service/src/limits/jig.rs`: 210 rows in jig's order, the requested public `Place`, `Use`, and `JIG`, and all 27 supplied notes copied byte for byte. Other notes are empty.
- `limits.rs`: only `mod jig;` and the requested re-export.
- `tests/jig.rs`, registered in `tests.rs`: the three requested tests, changing each name through jig's `overrides::value` and `overrides::apply`, with increment/decrement fallback and no branches on row names.
- Projection visibility changed: `startup::engine_limits` from private to `pub(crate)`. No other function's visibility changed. The test calls it for startup's effective client wire bound, observes `limits::engine_candidate` before sizing, and calls `workers::limits`, `worker_settings::root_limits`, `worker_settings::payload_limits`, `worker_settings::link_limits`, and `worker_settings::credential_bytes`. It compares the account-driver and client-component limits passed to their constructors. It also calls `jig_protocol_config::core`, as startup does, to observe runtime resume/tail settings and hub turns. The checked root result is deliberately discarded because the task requires the candidate even for adjacent inputs that sizing rejects.
- Inputs are jig's small profile, temper's small profile, `fixtures::engine`, the owned-worker document fixture, `fixtures::startup_agent`, and fixture seeds.

Provisional table counts, NOT test findings: Engine 147; Worker 12; Both 28; Neither 23. All places still need the required run. No test/S2 disagreement can be certified because zero tests ran.

Source inspection found these points to resolve or record when validation resumes:

- `fleet.slots`: S2 says `side: both`, while its `temper:` line and `worker_settings::link_limits` use the worker's configured slots. The draft still has the initial S2 `Both`; the predicted test result is `Engine`. Fill this from the test, as instructed.
- `hub.slots`: S2 says `side: worker`, while its `temper:` line says the configured worker slots replace it. The draft predicts `Neither` for the requested effective projections; the supplied note explains the worker-slots bound checked at start.
- `core.brief_core_budgets.tail`: S2's `side: engine` agrees with the draft's `Engine`, but its `temper:` line describes only the fixed priced bound. jig's `core` function, called by temper startup, also copies this override into `core.settings.tail_bytes`. The supplied owner note says temper does not read it. That note has been preserved verbatim; its conflict with the runtime-settings projection needs the owner's decision. No documentation wording or behavior was changed to conceal the conflict.

The draft's other `Neither` rows follow S2's `temper:` replacement lines despite S2's generic `side: engine` labels:

- `views.runs`: provisional `Neither`; S2 `side: engine`, with its `temper:` line taking temper-owned limits or constants.
- `views.watchers`: provisional `Neither`; S2 `side: engine`, with its `temper:` line taking temper-owned limits or constants.
- `views.watchers_per_party`: provisional `Neither`; S2 `side: engine`, with its `temper:` line taking temper-owned limits or constants.
- `views.subjects_per_change`: provisional `Neither`; S2 `side: engine`, with its `temper:` line taking temper-owned limits or constants.
- `views.text_bytes`: provisional `Neither`; S2 `side: engine`, with its `temper:` line taking temper-owned limits or constants.
- `views.facts`: provisional `Neither`; S2 `side: engine`, with its `temper:` line taking temper-owned limits or constants.
- `brief.briefs`: provisional `Neither`; S2 `side: engine`, with its `temper:` line taking temper-owned limits or constants.
- `brief.sections`: provisional `Neither`; S2 `side: engine`, with its `temper:` line taking temper-owned limits or constants.
- `brief.read_bytes`: provisional `Neither`; S2 `side: engine`, with its `temper:` line taking temper-owned limits or constants.
- `brief.brief_bytes`: provisional `Neither`; S2 `side: engine`, with its `temper:` line taking temper-owned limits or constants.
- `core.fill_mark`: provisional `Neither`; S2 `side: engine`, with its `temper:` line taking temper-owned limits or constants.
- `core.connectors`: provisional `Neither`; S2 `side: engine`, with its `temper:` line taking temper-owned limits or constants.
- `core.run_bytes`: provisional `Neither`; S2 `side: engine`, with its `temper:` line taking temper-owned limits or constants.
- `core.escalation_reason_bytes`: provisional `Neither`; S2 `side: engine`, with its `temper:` line taking temper-owned limits or constants.
- `core.load_slots`: provisional `Neither`; S2 `side: engine`, with its `temper:` line taking temper-owned limits or constants.
- `core.load_bytes`: provisional `Neither`; S2 `side: engine`, with its `temper:` line taking temper-owned limits or constants.
- `core.brief_parts`: provisional `Neither`; S2 `side: engine`, with its `temper:` line taking temper-owned limits or constants.
- `core.brief_core_budgets.task`: provisional `Neither`; S2 `side: engine`, with its `temper:` line taking temper-owned limits or constants.
- `core.brief_core_budgets.dependencies`: provisional `Neither`; S2 `side: engine`, with its `temper:` line taking temper-owned limits or constants.
- `core.brief_core_budgets.attempts`: provisional `Neither`; S2 `side: engine`, with its `temper:` line taking temper-owned limits or constants.
- `core.brief_core_budgets.plan`: provisional `Neither`; S2 `side: engine`, with its `temper:` line taking temper-owned limits or constants.
- `core.brief_core_budgets.notes`: provisional `Neither`; S2 `side: engine`, with its `temper:` line taking temper-owned limits or constants.

Verification:

- `cargo fmt` completed. `git diff --check` passed after cleanup. A source-level check counted 210 rows and confirmed every supplied note is byte-identical; this does not replace the Rust tests.
- `heavy cargo nextest run -p temper-service --lib -E 'test(jig)'`: two queued submissions, neither started. First token `a27f0ccf77a84462a3686e9f515f9492` was cancelled to tighten the startup projection while waiting (40 seconds queued, 0 seconds running). Second token `a3d76b5775b24a1ca6f0dc5c58bcbc89` was cancelled when disk space fell below the task's floor (183 seconds queued, 0 seconds running). Both exited 130. Test count: 0. Test execution time: 0 seconds; the under-one-second requirement remains unverified. Their printed log files were empty when read.
- Logs: `/home/free/src/rust/tmp/logs/a27f0ccf77a84462a3686e9f515f9492.log` and `/home/free/src/rust/tmp/logs/a3d76b5775b24a1ca6f0dc5c58bcbc89.log`.
- Clippy was not submitted: the disk rule stopped further builds. The default/focused suite was waived. No gate, land, or push ran.

Cleanup completed: the local jig patch was removed, `Cargo.lock` restored to its original tracked contents, and the worktree's `target/` removed if present. Jig was not changed. Nothing is staged. Git status is intentionally not clean because the unvalidated source draft remains uncommitted for continuation. Commit and final clean-status acceptance remain outstanding.

Resume by restoring the documented local patch to `jig-names`, checking disk space, running the narrow test, filling every place from its results, addressing compilation/lint findings, running clippy, and committing only the permitted source files with the required co-author trailer. Before committing, inspect `git diff --cached --name-only` to exclude the patch and path-based lock. Then remove the patch, restore the lock, remove this worktree's target, and replace this report with measured results and commit IDs.
