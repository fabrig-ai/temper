//! Reference reconciliation before Switch (domain/root.md, 9.1; protocol/store.md, 2).
use crate::{Domain, Limits, Name, Stored, Topic};
use skein_forge::api::Repository;
use skein_lib::List;

/// The first unresolved record's family, in restoration order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RecordKind {
    Call,
    ProposedEffect,
    Repository,
    Hold,
    Names,
    Subscription,
    BranchHead,
    PullState,
    Ci,
    Landed,
    Entry,
    Client,
    Change,
    Issue,
    Release,
    Application,
    Writer,
    Demo,
    Binding,
    DemoAct,
    Flight,
}
/// A refused restore reports each unresolved row once.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Unresolved {
    pub records: u32,
    pub first: RecordKind,
}
#[derive(Clone, Copy, Debug)]
pub(crate) enum Need {
    Forge(u16),
    Demo(u16),
    Repository(Repository),
    Task { task: u64, retired: Option<u16> },
    Entry { task: u64, entry: u64 },
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Reference {
    row: u32,
    kind: RecordKind,
    need: Need,
}
#[derive(Debug)]
pub(crate) struct Reconciliation {
    references: List<Reference>,
    row_start: u32,
    pub(crate) verdict: Result<(), Unresolved>,
}
// Retain the opaque client's references too: families can arrive in any order.
pub(crate) fn capacity(l: &Limits) -> Option<u32> {
    l.named_calls // Call: task.
        .checked_add(l.tasks.checked_mul(2)?)? // ProposedEffect: repository and task.
        .checked_add(l.repositories)? // Repository: forge.
        .checked_add(l.holds.checked_mul(3)?)? // Hold: name, task, writer task.
        .checked_add(l.tasks.checked_mul(l.resources_per_task.checked_add(1)?)?)? // Names.
        .checked_add(l.subscriptions.checked_mul(2)?)? // Subscription: repository and owner.
        .checked_add(l.client.resources.checked_mul(3)?)? // BranchHead, PullState, Client::Live.
        .checked_add(l.subscriptions)? // Ci: repository.
        .checked_add(l.entries.checked_mul(3)?)? // Entry: repository/task; Flight: entry/task.
        .checked_add(l.client.repositories)? // Client::Repository.
        .checked_add(l.changes)? // Change: repository; task is historical.
        .checked_add(l.issues.checked_mul(2)?)? // Issue: repository and live goal.
        .checked_add(l.tasks.checked_mul(3)?)? // Release: task, root, pending name.
        .checked_add(l.applications)? // Application: adopted repository, if any.
        .checked_add(l.client.repositories)? // Writer: configured forge.
        .checked_add(1) // Demo: configured demo's forge.
}
impl Reconciliation {
    pub(crate) fn new(l: &Limits) -> Self {
        Self {
            references: List::with_capacity(capacity(l).expect("checked reconciliation capacity")),
            row_start: 0,
            verdict: Ok(()),
        }
    }
    fn need(&mut self, kind: RecordKind, need: Need) {
        self.references
            .push(Reference { row: self.row_start, kind, need })
            .expect("restored references fit row bounds");
    }
    fn repository(&mut self, kind: RecordKind, repository: Repository) {
        self.need(kind, Need::Repository(repository));
    }
    fn task(&mut self, kind: RecordKind, task: u64, retired: Option<u16>) {
        self.need(kind, Need::Task { task, retired });
    }
    fn name(&mut self, kind: RecordKind, name: &Name) {
        self.repository(kind, Repository { forge: name.forge, repository: name.repository });
    }
    fn entry(&mut self, kind: RecordKind, entry: &skein_forge_client::Entry) {
        self.repository(kind, entry.repository);
        self.task(kind, entry.task, Some(entry.repository.forge));
    }
    pub(crate) fn restore(&mut self, row: &Stored) {
        // Only rows with references need an identity. The bounded list's
        // offset avoids counting arbitrarily long reference-free history.
        self.row_start = self.references.len();
        match row {
            Stored::Call { key, .. } => self.task(RecordKind::Call, key.task, None),
            Stored::ProposedEffect(row) => self.entry(RecordKind::ProposedEffect, &row.entry),
            Stored::Repository(row) => self.need(RecordKind::Repository, Need::Forge(row.provider.forge)),
            Stored::Hold(row) => {
                self.name(RecordKind::Hold, &row.name);
                self.task(RecordKind::Hold, row.task, Some(row.name.forge));
                if let Some(crate::Writer::Run { task, .. } | crate::Writer::Lost { task, .. }) = row.writer {
                    self.task(RecordKind::Hold, task, Some(row.name.forge));
                }
            }
            Stored::Names { task, resources } => {
                let mut retired = None;
                if let Some(name) = resources.first() {
                    retired = Some(name.forge);
                }
                for name in resources {
                    if retired != Some(name.forge) {
                        retired = None;
                    }
                }
                self.task(RecordKind::Names, *task, retired);
                for name in resources {
                    self.name(RecordKind::Names, name);
                }
            }
            Stored::Subscription(row) => {
                let repository = match row.topic {
                    Topic::Landings { repository, .. }
                    | Topic::Ci { repository, .. }
                    | Topic::Pull { repository, .. }
                    | Topic::Participation { repository, .. } => repository,
                };
                self.repository(RecordKind::Subscription, repository);
                self.task(RecordKind::Subscription, row.task, Some(repository.forge));
                // own_change and goal_tasks classify historical landings.
            }
            Stored::BranchHead(row) => self.name(RecordKind::BranchHead, &row.name),
            Stored::PullState(row) => self.name(RecordKind::PullState, &row.name),
            Stored::Ci(row) => self.repository(RecordKind::Ci, row.repository),
            Stored::Entry { entry, .. } => self.entry(RecordKind::Entry, entry),
            Stored::Client(row) => match row {
                skein_forge_client::Stored::Live(row) => {
                    self.repository(RecordKind::Client, row.watch.resource.repository);
                }
                skein_forge_client::Stored::Repository(row) => self.repository(RecordKind::Client, row.repository),
                skein_forge_client::Stored::Writer(row) => self.need(RecordKind::Client, Need::Forge(row.forge)),
            },
            // Change panels outlive tasks (domain/forge.md, 8.8).
            Stored::Change(row) => self.repository(RecordKind::Change, row.repository),
            Stored::Issue(row) => {
                self.repository(RecordKind::Issue, row.repository);
                let ending = match &row.desired {
                    Some(view) => view.finished.is_some(),
                    None => false,
                };
                if !row.state.closed && !ending {
                    self.task(RecordKind::Issue, row.goal, Some(row.repository.forge));
                }
            }
            Stored::Release(row) => {
                self.task(RecordKind::Release, row.task, None);
                self.task(RecordKind::Release, row.root, None);
                if let Some((_, Some(name))) = &row.pending {
                    self.name(RecordKind::Release, name);
                }
            }
            Stored::Application { outcome, .. } => match outcome {
                crate::ApplicationOutcome::Adopted(row) => self.repository(RecordKind::Application, row.repository),
                crate::ApplicationOutcome::CoreRefused(_) | crate::ApplicationOutcome::ForgeRefused(_) => {}
            },
            Stored::Writer(row) => self.need(RecordKind::Writer, Need::Forge(row.forge)),
            Stored::Demo(row) => self.need(RecordKind::Demo, Need::Demo(row.forge)),
            // Landing history outlives tasks (domain/forge.md, 7.2).
            // Bindings outlive members. End acts retry until the saved answer,
            // including after their target ended or was removed.
            Stored::Landed { .. }
            | Stored::Binding { .. }
            | Stored::DemoAct(_)
            | Stored::Verification(_)
            | Stored::Tier(_) => {}
            Stored::Flight { task, entry } => self.need(RecordKind::Flight, Need::Entry { task: *task, entry: *entry }),
        }
    }
}
impl Domain {
    /// Check the restored rows against the root's restored task facts.
    pub fn reconcile(&mut self, tasks: &jig_domain_tasks::Domain) {
        let mut verdict = Ok(());
        let mut failed = None;
        for reference in &self.reconciliation.references {
            let live = match reference.need {
                Need::Task { task, .. } | Need::Entry { task, .. } => tasks.task(task).is_some(),
                Need::Forge(_) | Need::Demo(_) | Need::Repository(_) => false,
            };
            if !self.reference_resolves(reference.need, live) && failed != Some(reference.row) {
                failed = Some(reference.row);
                add_failure(&mut verdict, reference.kind);
            }
        }
        self.reconciliation.verdict = verdict;
    }
    /// The verdict survives refusal; no unresolved row is erased.
    pub const fn reconciliation(&self) -> Result<(), Unresolved> {
        self.reconciliation.verdict
    }
    fn configured_forge(&self, forge: u16) -> bool {
        self.configured.contains_key(&forge) || self.identities.contains_key(&forge)
    }
    fn reference_resolves(&self, need: Need, live: bool) -> bool {
        match need {
            // Identities are constructor facts too; a restored writer is never one.
            Need::Forge(forge) => self.configured_forge(forge),
            Need::Demo(forge) => match &self.demo {
                Some(demo) => demo.adoption.forge == forge,
                None => false,
            },
            Need::Repository(repository) => {
                self.configured_forge(repository.forge) && self.repositories.contains_key(&repository)
            }
            Need::Task { retired, .. } => {
                live || match retired {
                    Some(forge) => self.forge_ended(forge),
                    None => false,
                }
            }
            Need::Entry { task, entry } => match self.entries.get(&entry) {
                Some(row) => row.entry.task == task && (live || self.forge_ended(row.entry.repository.forge)),
                None => false,
            },
        }
    }
}
fn add_failure(verdict: &mut Result<(), Unresolved>, kind: RecordKind) {
    match verdict {
        Ok(()) => *verdict = Err(Unresolved { records: 1, first: kind }),
        Err(why) => why.records = why.records.checked_add(1).expect("bounded unresolved rows"),
    }
}

#[cfg(test)]
pub(crate) fn references_resolve(d: &Domain, limits: &Limits, row: &Stored, live_tasks: &[u64]) -> bool {
    let mut reconciliation = Reconciliation::new(limits);
    reconciliation.restore(row);
    for reference in &reconciliation.references {
        let live = match reference.need {
            Need::Task { task, .. } | Need::Entry { task, .. } => live_tasks.contains(&task),
            Need::Forge(_) | Need::Demo(_) | Need::Repository(_) => false,
        };
        if !d.reference_resolves(reference.need, live) {
            return false;
        }
    }
    true
}
