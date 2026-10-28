Answer to your question: yes to both recommendations.

- **Units:** `root.loads.bytes` and the eight `root.brief.budgets.*` names are `Bytes`, as their documentation says. In `rows_are_unique_and_well_formed`, a name ending `_bytes` or `.bytes` is `Bytes`; the other `Bytes` rows without that ending are listed in the test by name (the four `forge_component` ones and these eight budgets), so a new one is a deliberate change.
- **Routing:** keep today's routing exactly. In `every_name_the_start_takes_is_a_row_routed_to_its_parser`: `handed_on` is true for the five setup rows the start hands on (`setup.orgs`, `setup.deadline_ms`, `setup.backoff_ms`, `setup.backoff_max_ms`, `setup.fetch_bytes`) and false for the three host-only rows (`setup.runs`, `setup.states`, `setup.state_life_ms`); `host_setup` is true for exactly the seven `apply_host` takes; each row is taken by the parser(s) the start routes it to (the four plan names by both `apply_host` and the plan's `apply`). `checked` keeps refusing the host-only names as it does today: no behaviour changes.

Go on with the task as written otherwise; the report is `reports/t2-2.md`.
