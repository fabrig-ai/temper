//! Dotted paths mirror every scalar in its owner's limits.
use super::{Limits, Refused};
use skein_lib::Duration;
fn duration(value: u64) -> Result<Duration, Refused> {
    match value.checked_mul(1_000_000) {
        Some(nanos) => Ok(Duration::from_nanos(nanos)),
        None => Err(Refused::Range),
    }
}
#[expect(clippy::too_many_lines, reason = "one exhaustive table of scalar dotted field paths")]
pub(super) fn apply(limits: &mut Limits, name: &[u8], value: u64) -> Result<(), Refused> {
    if name.starts_with(b"setup.") {
        return match temper_forge_setup::limits::apply_host(&mut limits.setup, name, value) {
            Ok(()) => Ok(()),
            Err(temper_forge_setup::limits::Refused::Unknown) => Err(Refused::Unknown),
            Err(temper_forge_setup::limits::Refused::Range) => Err(Refused::Range),
        };
    }
    match name {
        b"sign_in.members" => limits.sign_in.members = u32::try_from(value).or(Err(Refused::Range))?,
        b"sign_in.pending" => limits.sign_in.pending = u32::try_from(value).or(Err(Refused::Range))?,
        b"sign_in.deadline_ms" => limits.sign_in.deadline = duration(value)?,
        b"sign_in.fetches" => limits.sign_in.fetches = u32::try_from(value).or(Err(Refused::Range))?,
        b"sign_in.body_bytes" => limits.sign_in.body_bytes = u32::try_from(value).or(Err(Refused::Range))?,
        b"sign_in.url_bytes" => limits.sign_in.url_bytes = u32::try_from(value).or(Err(Refused::Range))?,
        b"sign_in.secret_bytes" => limits.sign_in.secret_bytes = u32::try_from(value).or(Err(Refused::Range))?,
        b"picker.page" => limits.picker.page = u32::try_from(value).or(Err(Refused::Range))?,
        b"routes.query_bytes" => limits.routes.query_bytes = u32::try_from(value).or(Err(Refused::Range))?,
        b"root.journal.commits" => limits.root.journal.commits = u32::try_from(value).or(Err(Refused::Range))?,
        b"root.journal.held" => limits.root.journal.held = u32::try_from(value).or(Err(Refused::Range))?,
        b"root.journal.writes" => limits.root.journal.writes = u32::try_from(value).or(Err(Refused::Range))?,
        b"root.journal.deliveries" => limits.root.journal.deliveries = u32::try_from(value).or(Err(Refused::Range))?,
        b"root.journal.transcript_bytes" => {
            limits.root.journal.transcript_bytes = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"root.journal.result_bytes" => {
            limits.root.journal.result_bytes = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"root.journal.run_bytes" => limits.root.journal.run_bytes = u32::try_from(value).or(Err(Refused::Range))?,
        b"root.loads.loads" => limits.root.loads.loads = u32::try_from(value).or(Err(Refused::Range))?,
        b"root.loads.rows" => limits.root.loads.rows = u32::try_from(value).or(Err(Refused::Range))?,
        b"root.loads.bytes" => limits.root.loads.bytes = u32::try_from(value).or(Err(Refused::Range))?,
        b"root.loads.read_bytes" => limits.root.loads.read_bytes = value,
        b"root.loads.reply_bytes" => limits.root.loads.reply_bytes = u32::try_from(value).or(Err(Refused::Range))?,
        b"root.loads.transcript_bytes" => {
            limits.root.loads.transcript_bytes = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"root.brief.briefs" => limits.root.brief.briefs = u32::try_from(value).or(Err(Refused::Range))?,
        b"root.brief.sections" => limits.root.brief.sections = u32::try_from(value).or(Err(Refused::Range))?,
        b"root.brief.parts" => limits.root.brief.parts = u32::try_from(value).or(Err(Refused::Range))?,
        b"root.brief.read_bytes" => limits.root.brief.read_bytes = u32::try_from(value).or(Err(Refused::Range))?,
        b"root.brief.brief_bytes" => limits.root.brief.brief_bytes = u32::try_from(value).or(Err(Refused::Range))?,
        b"root.brief.gather_ms" => limits.root.brief.gather = duration(value)?,
        b"root.brief.budgets.task" => limits.root.brief.budgets.task = u32::try_from(value).or(Err(Refused::Range))?,
        b"root.brief.budgets.dependencies" => {
            limits.root.brief.budgets.dependencies = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"root.brief.budgets.ci" => limits.root.brief.budgets.ci = u32::try_from(value).or(Err(Refused::Range))?,
        b"root.brief.budgets.reviews" => {
            limits.root.brief.budgets.reviews = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"root.brief.budgets.pull" => limits.root.brief.budgets.pull = u32::try_from(value).or(Err(Refused::Range))?,
        b"root.brief.budgets.attempts" => {
            limits.root.brief.budgets.attempts = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"root.brief.budgets.plan" => limits.root.brief.budgets.plan = u32::try_from(value).or(Err(Refused::Range))?,
        b"root.brief.budgets.notes" => {
            limits.root.brief.budgets.notes = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge.repositories" => limits.forge.repositories = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.tasks" => limits.forge.tasks = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.holds" => limits.forge.holds = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.subscriptions" => limits.forge.subscriptions = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.entries" => limits.forge.entries = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.named_calls" => limits.forge.named_calls = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.changes" => limits.forge.changes = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.issues" => limits.forge.issues = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.resources_per_task" => {
            limits.forge.resources_per_task = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge.paths_per_subscription" => {
            limits.forge.paths_per_subscription = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge.name_bytes" => limits.forge.name_bytes = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.output" => limits.forge.output = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.facts" => limits.forge.facts = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.adoptions" => limits.forge.adoptions = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.applications" => limits.forge.applications = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.collaborators" => limits.forge.collaborators = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.tiers" => limits.forge.tiers = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.failing_tests" => limits.forge.failing_tests = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.test_id_bytes" => limits.forge.test_id_bytes = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.landings" => limits.forge.landings = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.judge_projects" => limits.forge.judge_projects = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.judge_criteria" => limits.forge.judge_criteria = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.brief_sections" => limits.forge.brief_sections = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.brief_bytes" => limits.forge.brief_bytes = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.queue_window_ms" => limits.forge.queue_window = duration(value)?,
        b"forge.client.pending" => limits.forge.client.pending = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.client.calls" => limits.forge.client.calls = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.client.rate" => limits.forge.client.rate = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.client.reserve" => limits.forge.client.reserve = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.client.window_ms" => limits.forge.client.window = duration(value)?,
        b"forge.client.op_bytes" => limits.forge.client.op_bytes = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.client.answer_bytes" => {
            limits.forge.client.answer_bytes = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge.client.rows" => limits.forge.client.rows = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.client.inbox" => limits.forge.client.inbox = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.client.read_attempts" => {
            limits.forge.client.read_attempts = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge.client.backoff_ms" => limits.forge.client.backoff = duration(value)?,
        b"forge.client.backoff_max_ms" => limits.forge.client.backoff_max = duration(value)?,
        b"forge.client.entries" => limits.forge.client.entries = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.client.write_attempts" => {
            limits.forge.client.write_attempts = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge.client.lifetime_ms" => limits.forge.client.lifetime = duration(value)?,
        b"forge.client.clock_margin_ms" => limits.forge.client.clock_margin = duration(value)?,
        b"forge.client.resources" => limits.forge.client.resources = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.client.repositories" => {
            limits.forge.client.repositories = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge.client.poll_ms" => limits.forge.client.poll = duration(value)?,
        b"forge.client.poll_max_ms" => limits.forge.client.poll_max = duration(value)?,
        b"forge.client.hinted_ms" => limits.forge.client.hinted = duration(value)?,
        b"forge.client.slow_ms" => limits.forge.client.slow = duration(value)?,
        b"forge.client.settle_ms" => limits.forge.client.settle = duration(value)?,
        b"forge.client.facts" => limits.forge.client.facts = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge.change_policy.repairs" => {
            limits.forge.change_policy.repairs = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge.change_policy.resolutions" => {
            limits.forge.change_policy.resolutions = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge.change_policy.updates" => {
            limits.forge.change_policy.updates = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge.change_policy.stall_ms" => limits.forge.change_policy.stall = duration(value)?,
        b"forge.change_policy.gates" => {
            limits.forge.change_policy.gates = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge.change_policy.clean_heads" => {
            limits.forge.change_policy.clean_heads = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge.issue_policy.plan_items" => {
            limits.forge.issue_policy.plan_items = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge.issue_policy.milestones" => {
            limits.forge.issue_policy.milestones = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge.issue_policy.title_bytes" => {
            limits.forge.issue_policy.title_bytes = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge.issue_policy.body_bytes" => {
            limits.forge.issue_policy.body_bytes = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge.issue_policy.comment_bytes" => {
            limits.forge.issue_policy.comment_bytes = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge.issue_policy.interval_ms" => limits.forge.issue_policy.interval = duration(value)?,
        b"checkout.workspaces" => limits.checkout.workspaces = u32::try_from(value).or(Err(Refused::Range))?,
        b"checkout.repositories" => limits.checkout.repositories = u32::try_from(value).or(Err(Refused::Range))?,
        b"checkout.name_bytes" => limits.checkout.name_bytes = u32::try_from(value).or(Err(Refused::Range))?,
        b"checkout.message_bytes" => limits.checkout.message_bytes = u32::try_from(value).or(Err(Refused::Range))?,
        b"checkout.conflicts" => limits.checkout.conflicts = u32::try_from(value).or(Err(Refused::Range))?,
        b"checkout.path_bytes" => limits.checkout.path_bytes = u32::try_from(value).or(Err(Refused::Range))?,
        b"checkout.remote_timeout_ms" => limits.checkout.remote_timeout = duration(value)?,
        b"checkout.local_timeout_ms" => limits.checkout.local_timeout = duration(value)?,
        b"checkout.facts" => limits.checkout.facts = u32::try_from(value).or(Err(Refused::Range))?,
        b"git_component.operations" => {
            limits.git_component.operations = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"git_component.stdout_bytes" => {
            limits.git_component.stdout_bytes = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"git_component.stderr_bytes" => {
            limits.git_component.stderr_bytes = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"git_component.read_bytes" => {
            limits.git_component.read_bytes = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"git_component.sweep_entries" => {
            limits.git_component.sweep_entries = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"git_component.sweep_bytes" => {
            limits.git_component.sweep_bytes = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"clients.bundle_files" => limits.clients.bundle_files = u32::try_from(value).or(Err(Refused::Range))?,
        b"clients.bundle_bytes" => limits.clients.bundle_bytes = u32::try_from(value).or(Err(Refused::Range))?,
        b"records.value_bytes" => limits.records.value_bytes = u32::try_from(value).or(Err(Refused::Range))?,
        b"forges.installations" => limits.forges.installations = u32::try_from(value).or(Err(Refused::Range))?,
        b"forges.log_origins" => limits.forges.log_origins = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge_component.log_origins" => {
            limits.forge_component.log_origins = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge_component.log_poll_ms" => limits.forge_component.log_poll = duration(value)?,
        b"forge_component.log_catch_up_ms" => limits.forge_component.log_catch_up = duration(value)?,
        b"forge_component.log_pages" => {
            limits.forge_component.log_pages = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge_component.delivery_bytes" => {
            limits.forge_component.delivery_bytes = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge_component.installations" => {
            limits.forge_component.installations = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge_component.key_bytes" => {
            limits.forge_component.key_bytes = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge_component.renew_before_ms" => limits.forge_component.renew_before = duration(value)?,
        b"forge_component.forges" => limits.forge_component.forges = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge_component.token_bytes" => {
            limits.forge_component.token_bytes = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge_component.connections" => {
            limits.forge_component.connections = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge_component.head_bytes" => {
            limits.forge_component.head_bytes = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge_component.headers" => limits.forge_component.headers = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge_component.body_bytes_in" => {
            limits.forge_component.body_bytes_in = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge_component.depth" => limits.forge_component.depth = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge_component.chunk" => limits.forge_component.chunk = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge_component.string_bytes" => {
            limits.forge_component.string_bytes = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge_component.body_bytes" => {
            limits.forge_component.body_bytes = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge_component.inner" => limits.forge_component.inner = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge_component.target_bytes" => {
            limits.forge_component.target_bytes = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge_component.repositories" => {
            limits.forge_component.repositories = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge_component.users" => limits.forge_component.users = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge_component.tls.read" => {
            limits.forge_component.tls.read = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge_component.tls.send" => {
            limits.forge_component.tls.send = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge_component.tls.records" => {
            limits.forge_component.tls.records = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge_component.hooks" => limits.forge_component.hooks = u32::try_from(value).or(Err(Refused::Range))?,
        b"forge_component.hook_head_bytes" => {
            limits.forge_component.hook_head_bytes = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge_component.hook_body_bytes" => {
            limits.forge_component.hook_body_bytes = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge_component.hook_secret_bytes" => {
            limits.forge_component.hook_secret_bytes = u32::try_from(value).or(Err(Refused::Range))?;
        }
        b"forge_component.hook_deadline_ms" => limits.forge_component.hook_deadline = duration(value)?,
        b"forge_component.deadline_ms" => limits.forge_component.deadline = duration(value)?,
        b"forge_component.idle_ms" => limits.forge_component.idle = duration(value)?,
        b"forge_component.start_backoff_ms" => limits.forge_component.start_backoff = duration(value)?,
        b"forge_component.start_backoff_max_ms" => limits.forge_component.start_backoff_max = duration(value)?,
        _ => return Err(Refused::Unknown),
    }
    Ok(())
}
