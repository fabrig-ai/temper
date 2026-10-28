# Limits

Kind: reference

Every limit a deployment's configuration may override, by its dotted name: where it is set, its unit, its value and what it bounds.

A limit bounds what a deployment holds or does at once: a count, bytes or a time. Each is set by its dotted name in one of `temper.json`'s three `limits` members, each `{"profile": "small", "overrides": {"NAME": VALUE}}`: temper's own in `application.limits`, the engine's in `engine.limits`, and a worker's in `workers[N].limits`. `small`, for one person's machine, is the one profile; `temper init` writes some overrides of its own, which each row gives. A value is a whole number in its limit's unit; a time is in milliseconds, its name ending `_ms`.

The start reads them all and refuses one that is wrong, naming its path: an unknown name, a value past its type, or two limits that disagree, as in `temper serve: application.limits.overrides.setup.backoff_ms: must agree with application.limits.overrides.setup.backoff_max_ms`. A change takes effect at the next start.

Each prefix is a part of the deployment. A prefix of 8 names or more has a page of its own; the others' rows follow on this page.
