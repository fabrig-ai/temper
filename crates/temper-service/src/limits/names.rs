//! Dotted paths mirror the named scalars in their owner's limits.
use super::{Limits, Refused};
use jig_protocol_config::names::{Name, Unit};
use skein_lib::Duration;
/// Every name this parser takes, in the order of its owner prefixes.
pub const NAMES: [Name; 148] = [
    Name {
        name: "sign_in.members",
        unit: Unit::Count,
        line: "The members the Google sign-in may name, at most 64; a start naming more is refused.",
    },
    Name {
        name: "sign_in.pending",
        unit: Unit::Count,
        line: "Google sign-ins under way at once; a sign-in started past them is answered the busy page.",
    },
    Name {
        name: "sign_in.deadline_ms",
        unit: Unit::Milliseconds,
        line: "A Google sign-in's life, from its start to its return, and the sign-in cookie's Max-Age.",
    },
    Name {
        name: "sign_in.fetches",
        unit: Unit::Count,
        line: "Exchanges with Google at once; more wait for one to end.",
    },
    Name {
        name: "sign_in.body_bytes",
        unit: Unit::Bytes,
        line: "An answer's body from Google, its key set or a token, at most 65536; a longer answer fails the sign-in.",
    },
    Name {
        name: "sign_in.url_bytes",
        unit: Unit::Bytes,
        line: "A configured Google endpoint, and the authorization URL given to the browser; a longer endpoint refuses the start.",
    },
    Name {
        name: "sign_in.secret_bytes",
        unit: Unit::Bytes,
        line: "The Google client secret's file; a longer one refuses the start.",
    },
    Name {
        name: "picker.page",
        unit: Unit::Count,
        line: "The repositories one read of a forge's repository picker shows, from 1 to 100; the forge's own page cap may show fewer.",
    },
    Name {
        name: "routes.query_bytes",
        unit: Unit::Bytes,
        line: "The query of a forge's setup routes on the clients' listener, as a GitHub App's returns; a longer one is answered 400.",
    },
    Name {
        name: "root.journal.commits",
        unit: Unit::Count,
        line: "The engine's commits awaiting the store's durability; past them, new decisions wait.",
    },
    Name {
        name: "root.journal.held",
        unit: Unit::Count,
        line: "Outputs the engine holds until their commit is durable, at least 3 times `root.journal.deliveries`.",
    },
    Name {
        name: "root.journal.writes",
        unit: Unit::Count,
        line: "Writes in one of the engine's commits, the deployment's record among them; the start checks it against the engine's worst case.",
    },
    Name {
        name: "root.journal.deliveries",
        unit: Unit::Count,
        line: "Outputs of one decision of the engine's; the start checks it against `tasks.tasks`, the people's waiters and the fleet's workers.",
    },
    Name {
        name: "root.journal.transcript_bytes",
        unit: Unit::Bytes,
        line: "A row of the engine's journal, a chat's turn among them; it holds the agent's largest turn, which the start checks.",
    },
    Name {
        name: "root.journal.result_bytes",
        unit: Unit::Bytes,
        line: "A task's result or a brief as the engine holds it, at least `root.brief.brief_bytes`, `tasks.result_bytes` and `people.words`.",
    },
    Name {
        name: "root.journal.run_bytes",
        unit: Unit::Bytes,
        line: "A run's charter as the engine holds it; a configured charter past it refuses the start.",
    },
    Name {
        name: "root.loads.loads",
        unit: Unit::Count,
        line: "Pages the engine loads from its store at once, at least 2; more wait.",
    },
    Name {
        name: "root.loads.rows",
        unit: Unit::Count,
        line: "Rows in one page the engine loads from its store, at least 2.",
    },
    Name {
        name: "root.loads.bytes",
        unit: Unit::Bytes,
        line: "A page the engine keeps of what it loads, cut before the first row past it; it holds the agent's largest turn.",
    },
    Name {
        name: "root.loads.read_bytes",
        unit: Unit::Bytes,
        line: "What one person's read of the store brings, its rows included; it holds the agent's largest turn with its row.",
    },
    Name {
        name: "root.loads.reply_bytes",
        unit: Unit::Bytes,
        line: "A page the store answers the engine, decoded, at least `root.loads.bytes`; a larger page fails.",
    },
    Name {
        name: "root.loads.transcript_bytes",
        unit: Unit::Bytes,
        line: "One row of a page the engine loads, a turn among them; it holds the agent's largest turn, which the start checks.",
    },
    Name {
        name: "root.brief.briefs",
        unit: Unit::Count,
        line: "Briefs the engine gathers at once, a brief being what a run reads when it starts.",
    },
    Name {
        name: "root.brief.sections",
        unit: Unit::Count,
        line: "Sections in one brief: the task, its dependencies, CI, reviews and the others.",
    },
    Name { name: "root.brief.parts", unit: Unit::Count, line: "Parts read for one section of a brief." },
    Name {
        name: "root.brief.read_bytes",
        unit: Unit::Bytes,
        line: "What one section of a brief may read; a larger section is missing from the brief.",
    },
    Name {
        name: "root.brief.brief_bytes",
        unit: Unit::Bytes,
        line: "A run's whole brief, which its sections share, each cut to fit, at most `root.journal.result_bytes`.",
    },
    Name {
        name: "root.brief.gather_ms",
        unit: Unit::Milliseconds,
        line: "How long the engine gathers a brief before it gives up on the sections still missing.",
    },
    Name {
        name: "root.brief.budgets.task",
        unit: Unit::Bytes,
        line: "The task's section of a brief, its spec, contract and requester; cut to fit.",
    },
    Name {
        name: "root.brief.budgets.dependencies",
        unit: Unit::Bytes,
        line: "The dependencies' section of a brief; cut to fit.",
    },
    Name { name: "root.brief.budgets.ci", unit: Unit::Bytes, line: "The CI section of a brief; cut to fit." },
    Name {
        name: "root.brief.budgets.reviews",
        unit: Unit::Bytes,
        line: "The reviews' section of a brief; cut to fit.",
    },
    Name {
        name: "root.brief.budgets.pull",
        unit: Unit::Bytes,
        line: "The pull request's section of a brief; cut to fit.",
    },
    Name {
        name: "root.brief.budgets.attempts",
        unit: Unit::Bytes,
        line: "The earlier attempts' section of a brief; cut to fit.",
    },
    Name { name: "root.brief.budgets.plan", unit: Unit::Bytes, line: "The plan's section of a brief; cut to fit." },
    Name {
        name: "root.brief.budgets.notes",
        unit: Unit::Bytes,
        line: "The notes' section of a brief, within `root.brief.read_bytes`; cut to fit.",
    },
    Name {
        name: "forge.repositories",
        unit: Unit::Count,
        line: "Repositories adopted across every forge, at most `forge.client.repositories`; an adoption past it is refused as too large.",
    },
    Name {
        name: "forge.tasks",
        unit: Unit::Count,
        line: "Tasks that hold forge resources or effects at once, at least `tasks.tasks`; a task past it is refused.",
    },
    Name {
        name: "forge.holds",
        unit: Unit::Count,
        line: "Forge resources held by tasks at once, at most `forge.client.resources`; a hold past it is refused.",
    },
    Name {
        name: "forge.subscriptions",
        unit: Unit::Count,
        line: "Topics of the forges followed at once, CI reads among them; a subscription past it is refused.",
    },
    Name {
        name: "forge.entries",
        unit: Unit::Count,
        line: "Writes to the forges awaiting them, at most `forge.client.entries`; a write past it is refused.",
    },
    Name {
        name: "forge.named_calls",
        unit: Unit::Count,
        line: "Runs' calls to the forges held until answered, at least `core.call_records`.",
    },
    Name {
        name: "forge.changes",
        unit: Unit::Count,
        line: "Changes on their way to landing at once; a change past it is refused.",
    },
    Name {
        name: "forge.issues",
        unit: Unit::Count,
        line: "Tracked goals shown as issues at once; past it, a goal's issue fails.",
    },
    Name {
        name: "forge.resources_per_task",
        unit: Unit::Count,
        line: "Repositories one task may name, equal to `checkout.repositories`; a task naming more is refused.",
    },
    Name {
        name: "forge.paths_per_subscription",
        unit: Unit::Count,
        line: "Paths one subscription follows, and files read of one pull request; past it, the files are unknown.",
    },
    Name {
        name: "forge.name_bytes",
        unit: Unit::Bytes,
        line: "A forge's name for an owner, a repository or a branch; a longer one is refused, and the longest remote must fit `checkout.name_bytes`.",
    },
    Name {
        name: "forge.output",
        unit: Unit::Count,
        line: "Outputs the forge reserves for one step, at least what its holds, subscriptions and calls need, which the start checks.",
    },
    Name {
        name: "forge.facts",
        unit: Unit::Count,
        line: "Counted in the forge's worst case only; nothing reads it at run time.",
    },
    Name { name: "forge.adoptions", unit: Unit::Count, line: "Adoptions under way at once; another is answered busy." },
    Name {
        name: "forge.applications",
        unit: Unit::Count,
        line: "Adoption answers kept for a request asked again, with those under way; past it, an adoption is answered busy.",
    },
    Name {
        name: "forge.collaborators",
        unit: Unit::Count,
        line: "Collaborators one adoption reads; a repository with more is refused as too large.",
    },
    Name {
        name: "forge.landings",
        unit: Unit::Count,
        line: "Landings remembered at once; past it, a landing's news names no files.",
    },
    Name {
        name: "forge.judge_projects",
        unit: Unit::Count,
        line: "Projects with landing rules of their own, at least the projects configured.",
    },
    Name {
        name: "forge.judge_criteria",
        unit: Unit::Count,
        line: "Landing rules in the deployment's table or one project's; more refuse the start.",
    },
    Name {
        name: "forge.brief_sections",
        unit: Unit::Count,
        line: "Brief sections the forges gather at once, at least `root.brief.briefs` times `root.brief.sections` times 2.",
    },
    Name {
        name: "forge.brief_bytes",
        unit: Unit::Bytes,
        line: "One brief section a forge gathers, at least `root.brief.read_bytes`; a larger section fails.",
    },
    Name {
        name: "forge.queue_window_ms",
        unit: Unit::Milliseconds,
        line: "How long a ready change waits before it goes ahead of every change that became ready after it, whatever their priority.",
    },
    Name {
        name: "forge.client.pending",
        unit: Unit::Count,
        line: "Calls to a forge queued and in flight, at least `forge.client.calls`; a read past it is answered busy.",
    },
    Name {
        name: "forge.client.calls",
        unit: Unit::Count,
        line: "Calls to a forge in flight at once, equal to `forge_component.connections`; more wait.",
    },
    Name {
        name: "forge.client.rate",
        unit: Unit::Count,
        line: "Requests to a forge in one rate window; past it, calls wait for the next window.",
    },
    Name {
        name: "forge.client.reserve",
        unit: Unit::Count,
        line: "Requests of each rate window reserved so that waiting work is still served, at most `forge.client.rate`.",
    },
    Name {
        name: "forge.client.window_ms",
        unit: Unit::Milliseconds,
        line: "A forge's rate window, after which `forge.client.rate` requests are allowed again.",
    },
    Name {
        name: "forge.client.op_bytes",
        unit: Unit::Bytes,
        line: "One forge call's own bytes, its rows among them; a larger call is refused as too large.",
    },
    Name {
        name: "forge.client.answer_bytes",
        unit: Unit::Bytes,
        line: "One answer from a forge, decoded, a file or a job log among them; a larger answer fails as too large.",
    },
    Name {
        name: "forge.client.rows",
        unit: Unit::Count,
        line: "Rows in one page of a forge's answer, and a listing's page size; a larger answer fails.",
    },
    Name {
        name: "forge.client.inbox",
        unit: Unit::Count,
        line: "Comments and reviews remembered for one pull request or issue; one with more fails to read as too large.",
    },
    Name {
        name: "forge.client.read_attempts",
        unit: Unit::Count,
        line: "Tries of a forge read before its error stands.",
    },
    Name {
        name: "forge.client.backoff_ms",
        unit: Unit::Milliseconds,
        line: "The first wait before a forge read or write is tried again, doubling up to `forge.client.backoff_max_ms`.",
    },
    Name {
        name: "forge.client.backoff_max_ms",
        unit: Unit::Milliseconds,
        line: "The longest wait between two tries of a forge read or write.",
    },
    Name {
        name: "forge.client.entries",
        unit: Unit::Count,
        line: "Writes to a forge kept until done, at least `forge.entries`; another is answered busy.",
    },
    Name {
        name: "forge.client.write_attempts",
        unit: Unit::Count,
        line: "Tries of a forge write before it fails as unavailable.",
    },
    Name {
        name: "forge.client.lifetime_ms",
        unit: Unit::Milliseconds,
        line: "The longest a sent write may take to reach a forge, at least `forge_component.deadline_ms`; a write whose fate is unknown waits it out.",
    },
    Name {
        name: "forge.client.clock_margin_ms",
        unit: Unit::Milliseconds,
        line: "An allowance for the clock's moves, added to `forge.client.lifetime_ms` before a write whose fate is unknown is settled.",
    },
    Name {
        name: "forge.client.resources",
        unit: Unit::Count,
        line: "Forge resources watched at once, pull requests and issues among them, at least `forge.holds`; past it, a watch is answered busy.",
    },
    Name {
        name: "forge.client.repositories",
        unit: Unit::Count,
        line: "Repositories watched across the forges, and a set-up forge's adopted list, at least `forge.repositories`; past it, busy.",
    },
    Name {
        name: "forge.client.poll_ms",
        unit: Unit::Milliseconds,
        line: "How often a watched resource is read again, the wait doubling up to `forge.client.poll_max_ms`.",
    },
    Name {
        name: "forge.client.poll_max_ms",
        unit: Unit::Milliseconds,
        line: "The longest wait between two reads of a watched resource.",
    },
    Name {
        name: "forge.client.hinted_ms",
        unit: Unit::Milliseconds,
        line: "The wait before a resource is read again once a webhook or a write hints that it changed.",
    },
    Name {
        name: "forge.client.slow_ms",
        unit: Unit::Milliseconds,
        line: "The wait before a slow read of a resource or a repository.",
    },
    Name {
        name: "forge.client.settle_ms",
        unit: Unit::Milliseconds,
        line: "After a pull request's head or base moves, how long a not mergeable answer or a hinted job's pending CI is read again and not believed.",
    },
    Name {
        name: "forge.client.facts",
        unit: Unit::Count,
        line: "A forge client's diagnostic facts awaiting the log; past it, a fact is dropped and counted.",
    },
    Name {
        name: "forge.change_policy.repairs",
        unit: Unit::Count,
        line: "Repairs one change may delegate; past it, the change is held.",
    },
    Name {
        name: "forge.change_policy.resolutions",
        unit: Unit::Count,
        line: "Conflict resolutions one change may delegate; past it, the change is held.",
    },
    Name {
        name: "forge.change_policy.updates",
        unit: Unit::Count,
        line: "Updates of one change's branch to its base; past it, the change is held.",
    },
    Name {
        name: "forge.change_policy.stall_ms",
        unit: Unit::Milliseconds,
        line: "How long a change waits on its forge before it is held as stalled.",
    },
    Name {
        name: "forge.change_policy.gates",
        unit: Unit::Count,
        line: "Gates one change keeps, and the landing rules one table holds; more rules refuse the start.",
    },
    Name {
        name: "forge.change_policy.clean_heads",
        unit: Unit::Count,
        line: "A change's earlier heads remembered across clean updates; past it, the change is held.",
    },
    Name {
        name: "forge.issue_policy.plan_items",
        unit: Unit::Count,
        line: "Items of a goal's plan shown in its issue.",
    },
    Name {
        name: "forge.issue_policy.milestones",
        unit: Unit::Count,
        line: "Milestones listed in a goal's issue; later ones are summed up.",
    },
    Name { name: "forge.issue_policy.title_bytes", unit: Unit::Bytes, line: "A goal's issue title; cut to fit." },
    Name {
        name: "forge.issue_policy.body_bytes",
        unit: Unit::Bytes,
        line: "A goal's issue body, its goal and plan; cut to fit.",
    },
    Name {
        name: "forge.issue_policy.comment_bytes",
        unit: Unit::Bytes,
        line: "A comment on a goal's issue, for a milestone or the end; cut to fit.",
    },
    Name {
        name: "forge.issue_policy.interval_ms",
        unit: Unit::Milliseconds,
        line: "The least time between two updates of a goal's issue.",
    },
    Name {
        name: "checkout.workspaces",
        unit: Unit::Count,
        line: "Workspaces a worker keeps on disk, at least its slots; past it, the least recent idle one goes, and with none idle a checkout is busy.",
    },
    Name {
        name: "checkout.repositories",
        unit: Unit::Count,
        line: "Repositories one checkout holds, equal to `forge.resources_per_task` and within the agent's directories.",
    },
    Name {
        name: "checkout.name_bytes",
        unit: Unit::Bytes,
        line: "A checkout's key, a repository's name or remote, or a branch, at most the agent's; a longer one is refused.",
    },
    Name {
        name: "checkout.message_bytes",
        unit: Unit::Bytes,
        line: "A commit's message, title and body, at least the agent's largest call; a longer one is refused.",
    },
    Name {
        name: "checkout.conflicts",
        unit: Unit::Count,
        line: "Conflicted paths one repository may hold, within the agent's; 0 starts no merge.",
    },
    Name {
        name: "checkout.path_bytes",
        unit: Unit::Bytes,
        line: "One conflicted path, within the agent's; a longer one fails the merge, and 0 starts none.",
    },
    Name {
        name: "checkout.remote_timeout_ms",
        unit: Unit::Milliseconds,
        line: "How long a clone, fetch, branch or push may take before it times out.",
    },
    Name {
        name: "checkout.local_timeout_ms",
        unit: Unit::Milliseconds,
        line: "How long a local checkout, merge or commit may take before it times out.",
    },
    Name {
        name: "checkout.facts",
        unit: Unit::Count,
        line: "A checkout's diagnostic facts awaiting the log; past it, a fact is dropped and counted.",
    },
    Name {
        name: "git_component.operations",
        unit: Unit::Count,
        line: "Git operations a worker runs at once, at least `checkout.workspaces`; another fails.",
    },
    Name {
        name: "git_component.stdout_bytes",
        unit: Unit::Bytes,
        line: "The output kept of one git operation, at least what the checkout parses; more fails the operation.",
    },
    Name {
        name: "git_component.stderr_bytes",
        unit: Unit::Bytes,
        line: "The tail of one git operation's errors kept for its diagnostic.",
    },
    Name {
        name: "git_component.read_bytes",
        unit: Unit::Bytes,
        line: "One read of a conflicted file while it is scanned.",
    },
    Name {
        name: "git_component.sweep_entries",
        unit: Unit::Count,
        line: "Entries read in one page while a worker clears its trash.",
    },
    Name {
        name: "git_component.sweep_bytes",
        unit: Unit::Bytes,
        line: "The names read in one page while a worker clears its trash.",
    },
    Name {
        name: "clients.bundle_files",
        unit: Unit::Count,
        line: "The web client's files, its page among them; a bundle with more refuses the start.",
    },
    Name {
        name: "clients.bundle_bytes",
        unit: Unit::Bytes,
        line: "The web client's files in all, held in memory while `temper serve` runs; a larger bundle refuses the start.",
    },
    Name {
        name: "records.value_bytes",
        unit: Unit::Bytes,
        line: "A forge's record or a value temper carries, such as a pasted token; a longer value is refused, named.",
    },
    Name {
        name: "forges.installations",
        unit: Unit::Count,
        line: "The installations a GitHub forge accepts, at most `forge_component.installations`.",
    },
    Name {
        name: "forges.log_origins",
        unit: Unit::Count,
        line: "Checked at the start alone, at most 4 and at most `forge_component.log_origins`; nothing reads it at run time.",
    },
    Name {
        name: "forge_component.log_origins",
        unit: Unit::Count,
        line: "The origins a GitHub job log's redirect may reach, per forge, at least `forges.log_origins`; a redirect elsewhere is refused.",
    },
    Name {
        name: "forge_component.log_poll_ms",
        unit: Unit::Milliseconds,
        line: "The wait between two reads of a GitHub App's delivery log.",
    },
    Name {
        name: "forge_component.log_catch_up_ms",
        unit: Unit::Milliseconds,
        line: "How far back a GitHub App's delivery log is read when a forge starts again in one process, at most a day.",
    },
    Name {
        name: "forge_component.log_pages",
        unit: Unit::Count,
        line: "Pages one read of a GitHub App's delivery log takes; the rest waits for the next read.",
    },
    Name {
        name: "forge_component.delivery_bytes",
        unit: Unit::Bytes,
        line: "One delivery read from a GitHub App's delivery log; a larger one hints nothing.",
    },
    Name {
        name: "forge_component.installations",
        unit: Unit::Count,
        line: "The installations whose tokens a GitHub forge keeps, at least `forges.installations`.",
    },
    Name {
        name: "forge_component.key_bytes",
        unit: Unit::Bytes,
        line: "A GitHub App's private key, at least 1196 with a GitHub forge; a larger key is refused.",
    },
    Name {
        name: "forge_component.renew_before_ms",
        unit: Unit::Milliseconds,
        line: "How long before an installation's token expires it is minted again, from twice `forge_component.deadline_ms` to 30 minutes.",
    },
    Name {
        name: "forge_component.forges",
        unit: Unit::Count,
        line: "The forges configured, in all; more refuse the start.",
    },
    Name {
        name: "forge_component.token_bytes",
        unit: Unit::Bytes,
        line: "A forge's token: an API token, a git token or a site admin's; a longer one is refused.",
    },
    Name {
        name: "forge_component.connections",
        unit: Unit::Count,
        line: "Connections to a forge for calls, equal to `forge.client.calls`; a call waits for one.",
    },
    Name {
        name: "forge_component.head_bytes",
        unit: Unit::Bytes,
        line: "The head of a forge's HTTP answer; a longer head fails the call.",
    },
    Name {
        name: "forge_component.headers",
        unit: Unit::Count,
        line: "The header fields of a forge's answer or a webhook's request; a webhook with more is answered 431.",
    },
    Name {
        name: "forge_component.body_bytes_in",
        unit: Unit::Count,
        line: "A forge's answer body as it streams in; a longer one fails to decode.",
    },
    Name {
        name: "forge_component.depth",
        unit: Unit::Count,
        line: "The nesting of a forge's JSON, at least 8; a deeper document fails.",
    },
    Name {
        name: "forge_component.chunk",
        unit: Unit::Bytes,
        line: "The piece in which a forge's answer is read and decoded; it bounds no whole.",
    },
    Name {
        name: "forge_component.string_bytes",
        unit: Unit::Bytes,
        line: "A text in a forge's answer, a name, title, path or link, at least `forge.name_bytes`; a longer one fails the answer.",
    },
    Name {
        name: "forge_component.body_bytes",
        unit: Unit::Bytes,
        line: "A body kept from a forge, as a pull request's or a comment's, and a write's body with its marker; a longer one fails.",
    },
    Name {
        name: "forge_component.inner",
        unit: Unit::Count,
        line: "The items of a list inside one document from a forge; more fail the document.",
    },
    Name {
        name: "forge_component.target_bytes",
        unit: Unit::Bytes,
        line: "The target of one HTTP request to a forge, its path and query; a longer one is refused.",
    },
    Name {
        name: "forge_component.repositories",
        unit: Unit::Count,
        line: "Repositories whose lookups a forge keeps, at most `forge.repositories`; past it, the least recent goes.",
    },
    Name {
        name: "forge_component.users",
        unit: Unit::Count,
        line: "Users whose logins a forge keeps; past it, the least recent goes.",
    },
    Name {
        name: "forge_component.tls.read",
        unit: Unit::Bytes,
        line: "The plaintext read at once from a forge's TLS connection.",
    },
    Name {
        name: "forge_component.tls.send",
        unit: Unit::Bytes,
        line: "The plaintext sent at once on a forge's TLS connection; a request head past it is refused.",
    },
    Name {
        name: "forge_component.tls.records",
        unit: Unit::Bytes,
        line: "The ciphertext kept of a TLS handshake with a forge, at least 18437; a longer handshake message fails.",
    },
    Name { name: "forge_component.hooks", unit: Unit::Count, line: "Webhook connections at once, in all; more wait." },
    Name {
        name: "forge_component.hook_head_bytes",
        unit: Unit::Bytes,
        line: "A webhook delivery's request head; a longer one is answered 431.",
    },
    Name {
        name: "forge_component.hook_body_bytes",
        unit: Unit::Bytes,
        line: "A webhook delivery's body, read as it streams; a longer one is answered 413 and hints nothing.",
    },
    Name {
        name: "forge_component.hook_secret_bytes",
        unit: Unit::Bytes,
        line: "A Forgejo forge's webhook secret, at least 32; a longer one is refused.",
    },
    Name {
        name: "forge_component.hook_deadline_ms",
        unit: Unit::Milliseconds,
        line: "A webhook delivery's read and answer, from its connection's accept or its last answer; past it, the connection closes.",
    },
    Name {
        name: "forge_component.deadline_ms",
        unit: Unit::Milliseconds,
        line: "A call to a forge, and each read as a forge starts, at most `forge.client.lifetime_ms`; past it, the call fails.",
    },
    Name {
        name: "forge_component.idle_ms",
        unit: Unit::Milliseconds,
        line: "How long an idle connection to a forge stays open.",
    },
    Name {
        name: "forge_component.start_backoff_ms",
        unit: Unit::Milliseconds,
        line: "The first wait before a forge's start is tried again, doubling up to `forge_component.start_backoff_max_ms`.",
    },
    Name {
        name: "forge_component.start_backoff_max_ms",
        unit: Unit::Milliseconds,
        line: "The longest wait between two tries of a forge's start.",
    },
];

#[expect(clippy::manual_find, reason = "step code uses a loop rather than a closure")]
fn find(name: &[u8]) -> Option<&'static Name> {
    for row in &NAMES {
        if row.name.as_bytes() == name {
            return Some(row);
        }
    }
    None
}

pub(super) fn apply(limits: &mut Limits, name: &[u8], value: u64) -> Result<(), Refused> {
    if name.starts_with(b"setup.") {
        return match temper_forge_setup::limits::apply_host(&mut limits.setup, name, value) {
            Ok(()) => Ok(()),
            Err(temper_forge_setup::limits::Refused::Unknown) => Err(Refused::Unknown),
            Err(temper_forge_setup::limits::Refused::Range) => Err(Refused::Range),
        };
    }
    let row = find(name).ok_or(Refused::Unknown)?;
    field(limits, row.name.as_bytes(), Some(value))?;
    Ok(())
}
/// A named limit's value, a duration's in milliseconds; none for a name
/// with no row.
#[must_use]
pub fn value(limits: &Limits, name: &[u8]) -> Option<u64> {
    let row = find(name)?;
    let mut copy = *limits;
    field(&mut copy, row.name.as_bytes(), None).ok()
}
#[expect(clippy::too_many_lines, reason = "one exhaustive table of scalar dotted field paths")]
fn field(limits: &mut Limits, name: &[u8], to: Option<u64>) -> Result<u64, Refused> {
    match name {
        b"sign_in.members" => count(&mut limits.sign_in.members, to),
        b"sign_in.pending" => count(&mut limits.sign_in.pending, to),
        b"sign_in.deadline_ms" => millis(&mut limits.sign_in.deadline, to),
        b"sign_in.fetches" => count(&mut limits.sign_in.fetches, to),
        b"sign_in.body_bytes" => count(&mut limits.sign_in.body_bytes, to),
        b"sign_in.url_bytes" => count(&mut limits.sign_in.url_bytes, to),
        b"sign_in.secret_bytes" => count(&mut limits.sign_in.secret_bytes, to),
        b"picker.page" => count(&mut limits.picker.page, to),
        b"routes.query_bytes" => count(&mut limits.routes.query_bytes, to),
        b"root.journal.commits" => count(&mut limits.root.journal.commits, to),
        b"root.journal.held" => count(&mut limits.root.journal.held, to),
        b"root.journal.writes" => count(&mut limits.root.journal.writes, to),
        b"root.journal.deliveries" => count(&mut limits.root.journal.deliveries, to),
        b"root.journal.transcript_bytes" => count(&mut limits.root.journal.transcript_bytes, to),
        b"root.journal.result_bytes" => count(&mut limits.root.journal.result_bytes, to),
        b"root.journal.run_bytes" => count(&mut limits.root.journal.run_bytes, to),
        b"root.loads.loads" => count(&mut limits.root.loads.loads, to),
        b"root.loads.rows" => count(&mut limits.root.loads.rows, to),
        b"root.loads.bytes" => count(&mut limits.root.loads.bytes, to),
        b"root.loads.read_bytes" => Ok(wide(&mut limits.root.loads.read_bytes, to)),
        b"root.loads.reply_bytes" => count(&mut limits.root.loads.reply_bytes, to),
        b"root.loads.transcript_bytes" => count(&mut limits.root.loads.transcript_bytes, to),
        b"root.brief.briefs" => count(&mut limits.root.brief.briefs, to),
        b"root.brief.sections" => count(&mut limits.root.brief.sections, to),
        b"root.brief.parts" => count(&mut limits.root.brief.parts, to),
        b"root.brief.read_bytes" => count(&mut limits.root.brief.read_bytes, to),
        b"root.brief.brief_bytes" => count(&mut limits.root.brief.brief_bytes, to),
        b"root.brief.gather_ms" => millis(&mut limits.root.brief.gather, to),
        b"root.brief.budgets.task" => count(&mut limits.root.brief.budgets.task, to),
        b"root.brief.budgets.dependencies" => count(&mut limits.root.brief.budgets.dependencies, to),
        b"root.brief.budgets.ci" => count(&mut limits.root.brief.budgets.ci, to),
        b"root.brief.budgets.reviews" => count(&mut limits.root.brief.budgets.reviews, to),
        b"root.brief.budgets.pull" => count(&mut limits.root.brief.budgets.pull, to),
        b"root.brief.budgets.attempts" => count(&mut limits.root.brief.budgets.attempts, to),
        b"root.brief.budgets.plan" => count(&mut limits.root.brief.budgets.plan, to),
        b"root.brief.budgets.notes" => count(&mut limits.root.brief.budgets.notes, to),
        b"forge.repositories" => count(&mut limits.forge.repositories, to),
        b"forge.tasks" => count(&mut limits.forge.tasks, to),
        b"forge.holds" => count(&mut limits.forge.holds, to),
        b"forge.subscriptions" => count(&mut limits.forge.subscriptions, to),
        b"forge.entries" => count(&mut limits.forge.entries, to),
        b"forge.named_calls" => count(&mut limits.forge.named_calls, to),
        b"forge.changes" => count(&mut limits.forge.changes, to),
        b"forge.issues" => count(&mut limits.forge.issues, to),
        b"forge.resources_per_task" => count(&mut limits.forge.resources_per_task, to),
        b"forge.paths_per_subscription" => count(&mut limits.forge.paths_per_subscription, to),
        b"forge.name_bytes" => count(&mut limits.forge.name_bytes, to),
        b"forge.output" => count(&mut limits.forge.output, to),
        b"forge.facts" => count(&mut limits.forge.facts, to),
        b"forge.adoptions" => count(&mut limits.forge.adoptions, to),
        b"forge.applications" => count(&mut limits.forge.applications, to),
        b"forge.collaborators" => count(&mut limits.forge.collaborators, to),
        b"forge.landings" => count(&mut limits.forge.landings, to),
        b"forge.judge_projects" => count(&mut limits.forge.judge_projects, to),
        b"forge.judge_criteria" => count(&mut limits.forge.judge_criteria, to),
        b"forge.brief_sections" => count(&mut limits.forge.brief_sections, to),
        b"forge.brief_bytes" => count(&mut limits.forge.brief_bytes, to),
        b"forge.queue_window_ms" => millis(&mut limits.forge.queue_window, to),
        b"forge.client.pending" => count(&mut limits.forge.client.pending, to),
        b"forge.client.calls" => count(&mut limits.forge.client.calls, to),
        b"forge.client.rate" => count(&mut limits.forge.client.rate, to),
        b"forge.client.reserve" => count(&mut limits.forge.client.reserve, to),
        b"forge.client.window_ms" => millis(&mut limits.forge.client.window, to),
        b"forge.client.op_bytes" => count(&mut limits.forge.client.op_bytes, to),
        b"forge.client.answer_bytes" => count(&mut limits.forge.client.answer_bytes, to),
        b"forge.client.rows" => count(&mut limits.forge.client.rows, to),
        b"forge.client.inbox" => count(&mut limits.forge.client.inbox, to),
        b"forge.client.read_attempts" => count(&mut limits.forge.client.read_attempts, to),
        b"forge.client.backoff_ms" => millis(&mut limits.forge.client.backoff, to),
        b"forge.client.backoff_max_ms" => millis(&mut limits.forge.client.backoff_max, to),
        b"forge.client.entries" => count(&mut limits.forge.client.entries, to),
        b"forge.client.write_attempts" => count(&mut limits.forge.client.write_attempts, to),
        b"forge.client.lifetime_ms" => millis(&mut limits.forge.client.lifetime, to),
        b"forge.client.clock_margin_ms" => millis(&mut limits.forge.client.clock_margin, to),
        b"forge.client.resources" => count(&mut limits.forge.client.resources, to),
        b"forge.client.repositories" => count(&mut limits.forge.client.repositories, to),
        b"forge.client.poll_ms" => millis(&mut limits.forge.client.poll, to),
        b"forge.client.poll_max_ms" => millis(&mut limits.forge.client.poll_max, to),
        b"forge.client.hinted_ms" => millis(&mut limits.forge.client.hinted, to),
        b"forge.client.slow_ms" => millis(&mut limits.forge.client.slow, to),
        b"forge.client.settle_ms" => millis(&mut limits.forge.client.settle, to),
        b"forge.client.facts" => count(&mut limits.forge.client.facts, to),
        b"forge.change_policy.repairs" => count(&mut limits.forge.change_policy.repairs, to),
        b"forge.change_policy.resolutions" => count(&mut limits.forge.change_policy.resolutions, to),
        b"forge.change_policy.updates" => count(&mut limits.forge.change_policy.updates, to),
        b"forge.change_policy.stall_ms" => millis(&mut limits.forge.change_policy.stall, to),
        b"forge.change_policy.gates" => count(&mut limits.forge.change_policy.gates, to),
        b"forge.change_policy.clean_heads" => count(&mut limits.forge.change_policy.clean_heads, to),
        b"forge.issue_policy.plan_items" => count(&mut limits.forge.issue_policy.plan_items, to),
        b"forge.issue_policy.milestones" => count(&mut limits.forge.issue_policy.milestones, to),
        b"forge.issue_policy.title_bytes" => count(&mut limits.forge.issue_policy.title_bytes, to),
        b"forge.issue_policy.body_bytes" => count(&mut limits.forge.issue_policy.body_bytes, to),
        b"forge.issue_policy.comment_bytes" => count(&mut limits.forge.issue_policy.comment_bytes, to),
        b"forge.issue_policy.interval_ms" => millis(&mut limits.forge.issue_policy.interval, to),
        b"checkout.workspaces" => count(&mut limits.checkout.workspaces, to),
        b"checkout.repositories" => count(&mut limits.checkout.repositories, to),
        b"checkout.name_bytes" => count(&mut limits.checkout.name_bytes, to),
        b"checkout.message_bytes" => count(&mut limits.checkout.message_bytes, to),
        b"checkout.conflicts" => count(&mut limits.checkout.conflicts, to),
        b"checkout.path_bytes" => count(&mut limits.checkout.path_bytes, to),
        b"checkout.remote_timeout_ms" => millis(&mut limits.checkout.remote_timeout, to),
        b"checkout.local_timeout_ms" => millis(&mut limits.checkout.local_timeout, to),
        b"checkout.facts" => count(&mut limits.checkout.facts, to),
        b"git_component.operations" => count(&mut limits.git_component.operations, to),
        b"git_component.stdout_bytes" => count(&mut limits.git_component.stdout_bytes, to),
        b"git_component.stderr_bytes" => count(&mut limits.git_component.stderr_bytes, to),
        b"git_component.read_bytes" => count(&mut limits.git_component.read_bytes, to),
        b"git_component.sweep_entries" => count(&mut limits.git_component.sweep_entries, to),
        b"git_component.sweep_bytes" => count(&mut limits.git_component.sweep_bytes, to),
        b"clients.bundle_files" => count(&mut limits.clients.bundle_files, to),
        b"clients.bundle_bytes" => count(&mut limits.clients.bundle_bytes, to),
        b"records.value_bytes" => count(&mut limits.records.value_bytes, to),
        b"forges.installations" => count(&mut limits.forges.installations, to),
        b"forges.log_origins" => count(&mut limits.forges.log_origins, to),
        b"forge_component.log_origins" => count(&mut limits.forge_component.log_origins, to),
        b"forge_component.log_poll_ms" => millis(&mut limits.forge_component.log_poll, to),
        b"forge_component.log_catch_up_ms" => millis(&mut limits.forge_component.log_catch_up, to),
        b"forge_component.log_pages" => count(&mut limits.forge_component.log_pages, to),
        b"forge_component.delivery_bytes" => count(&mut limits.forge_component.delivery_bytes, to),
        b"forge_component.installations" => count(&mut limits.forge_component.installations, to),
        b"forge_component.key_bytes" => count(&mut limits.forge_component.key_bytes, to),
        b"forge_component.renew_before_ms" => millis(&mut limits.forge_component.renew_before, to),
        b"forge_component.forges" => count(&mut limits.forge_component.forges, to),
        b"forge_component.token_bytes" => count(&mut limits.forge_component.token_bytes, to),
        b"forge_component.connections" => count(&mut limits.forge_component.connections, to),
        b"forge_component.head_bytes" => count(&mut limits.forge_component.head_bytes, to),
        b"forge_component.headers" => count(&mut limits.forge_component.headers, to),
        b"forge_component.body_bytes_in" => count(&mut limits.forge_component.body_bytes_in, to),
        b"forge_component.depth" => count(&mut limits.forge_component.depth, to),
        b"forge_component.chunk" => count(&mut limits.forge_component.chunk, to),
        b"forge_component.string_bytes" => count(&mut limits.forge_component.string_bytes, to),
        b"forge_component.body_bytes" => count(&mut limits.forge_component.body_bytes, to),
        b"forge_component.inner" => count(&mut limits.forge_component.inner, to),
        b"forge_component.target_bytes" => count(&mut limits.forge_component.target_bytes, to),
        b"forge_component.repositories" => count(&mut limits.forge_component.repositories, to),
        b"forge_component.users" => count(&mut limits.forge_component.users, to),
        b"forge_component.tls.read" => count(&mut limits.forge_component.tls.read, to),
        b"forge_component.tls.send" => count(&mut limits.forge_component.tls.send, to),
        b"forge_component.tls.records" => count(&mut limits.forge_component.tls.records, to),
        b"forge_component.hooks" => count(&mut limits.forge_component.hooks, to),
        b"forge_component.hook_head_bytes" => count(&mut limits.forge_component.hook_head_bytes, to),
        b"forge_component.hook_body_bytes" => count(&mut limits.forge_component.hook_body_bytes, to),
        b"forge_component.hook_secret_bytes" => count(&mut limits.forge_component.hook_secret_bytes, to),
        b"forge_component.hook_deadline_ms" => millis(&mut limits.forge_component.hook_deadline, to),
        b"forge_component.deadline_ms" => millis(&mut limits.forge_component.deadline, to),
        b"forge_component.idle_ms" => millis(&mut limits.forge_component.idle, to),
        b"forge_component.start_backoff_ms" => millis(&mut limits.forge_component.start_backoff, to),
        b"forge_component.start_backoff_max_ms" => millis(&mut limits.forge_component.start_backoff_max, to),
        _ => Err(Refused::Unknown),
    }
}

fn count(field: &mut u32, to: Option<u64>) -> Result<u64, Refused> {
    if let Some(value) = to {
        *field = u32::try_from(value).or(Err(Refused::Range))?;
    }
    Ok(u64::from(*field))
}
fn millis(field: &mut Duration, to: Option<u64>) -> Result<u64, Refused> {
    if let Some(value) = to {
        *field = duration(value)?;
    }
    Ok(field.as_nanos() / 1_000_000)
}

fn wide(field: &mut u64, to: Option<u64>) -> u64 {
    if let Some(value) = to {
        *field = value;
    }
    *field
}
fn duration(value: u64) -> Result<Duration, Refused> {
    match value.checked_mul(1_000_000) {
        Some(nanos) => Ok(Duration::from_nanos(nanos)),
        None => Err(Refused::Range),
    }
}
