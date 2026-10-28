//! Next-run drafts borrow the catalogue; they never choose an account (client/README.md, 3.5).
use super::{Pages, Request, accounts};
use crate::owner::Owner;
use alloc::boxed::Box;
use jig_client::{Client, Key, Outcome, Told, contract as c};
use skein_lib::{List, Queue};

/// Catalogue ceilings enforced by the received client wire.
pub const MODELS: u32 = jig_client_protocol_wire::v19::CEILINGS.me_project_models;
pub const EFFORTS: u32 = jig_client_protocol_wire::v19::CEILINGS.me_model_efforts;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Context {
    Template { project: u32, executor: c::TemplateExecutor, model_index: Option<u8> },
    Task { project: u32, task: u64, revision: c::Revision, model_index: Option<u8> },
}
impl Context {
    // The row fits the Task variant's existing alignment space: no extra draft bytes.
    // None resolves a supplied choice by name until a person picks a row.
    // A byte index covers the wire catalogue ceiling (64 rows).
    fn model_index(self) -> Option<u32> {
        let index = match self {
            Self::Template { model_index, .. } | Self::Task { model_index, .. } => model_index,
        };
        Some(u32::from(index?))
    }
    fn select(&mut self, index: Option<u8>) {
        match self {
            Self::Template { model_index, .. } | Self::Task { model_index, .. } => *model_index = index,
        }
    }
    fn unselected(context: Option<Self>) -> Option<Self> {
        match context {
            Some(mut context) => {
                context.select(None);
                Some(context)
            }
            None => None,
        }
    }
}
#[derive(Debug)]
pub struct Draft {
    pub context: Option<Context>,
    pub original: Option<c::LlmChoice>,
    pub requested: Option<c::LlmChoice>,
    pub pending: Option<Key>,
    pub outcome: Option<Outcome>,
}
#[expect(
    clippy::derivable_impls,
    reason = "the programming model permits explicit Default implementations, not its derive"
)]
impl Default for Draft {
    fn default() -> Self {
        Self { context: None, original: None, requested: None, pending: None, outcome: None }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    Model { index: u32 },
    Effort { index: u32 },
    Default,
    Apply,
}
#[must_use]
pub fn models(me: &c::Me, project: u32) -> Option<&List<c::MeModel>> {
    for row in &me.projects {
        if row.project == project {
            return Some(&row.models);
        }
    }
    None
}
impl Draft {
    pub(crate) fn reset(&mut self, context: Option<Context>, original: Option<&c::LlmChoice>) {
        let context = Context::unselected(context);
        if Context::unselected(self.context) != context || self.original.as_ref() != original {
            self.context = context;
            self.original = original.cloned();
            self.requested = original.cloned();
            self.outcome = None;
        }
    }
    #[must_use]
    pub fn project(&self) -> Option<u32> {
        match self.context? {
            Context::Template { project, .. } | Context::Task { project, .. } => Some(project),
        }
    }
    /// Exactly one offered row supplies the draft's effort controls and validation.
    #[must_use]
    pub fn selected(&self, me: &c::Me) -> Option<u32> {
        let choice = self.requested.as_ref()?;
        let models = models(me, self.project()?)?;
        if let Some(index) = self.context?.model_index() {
            let model = models.get(index)?;
            return if model.name == choice.main.model { Some(index) } else { None };
        }
        for (model, index) in models.iter().zip(0u32..) {
            if model.name == choice.main.model {
                return Some(index);
            }
        }
        None
    }
    #[must_use]
    pub fn valid(&self, me: &c::Me) -> bool {
        let Some(choice) = &self.requested else { return true };
        let Some(index) = self.selected(me) else { return false };
        let Some(project) = self.project() else { return false };
        let Some(models) = models(me, project) else { return false };
        let Some(model) = models.get(index) else { return false };
        if model.efforts.is_empty() {
            choice.main.effort.is_none()
        } else if let Some(effort) = &choice.main.effort {
            model.efforts.as_slice().contains(effort)
        } else {
            false
        }
    }
    pub(crate) fn edit(&mut self, me: &c::Me, act: Act) {
        if self.pending.is_some() {
            return;
        }
        let Some(project) = self.project() else { return };
        let Some(models) = models(me, project) else { return };
        match act {
            Act::Model { index } => {
                let Some(model) = models.get(index) else { return };
                let Some(context) = &mut self.context else { return };
                let Ok(index) = u8::try_from(index) else { return };
                context.select(Some(index));
                if let Some(choice) = &mut self.requested {
                    choice.main = c::ModelEffort { model: model.name.clone(), effort: None };
                } else {
                    self.requested = Some(c::LlmChoice {
                        account: None,
                        main: c::ModelEffort { model: model.name.clone(), effort: None },
                        subagents: List::with_capacity(0),
                    });
                }
            }
            Act::Effort { index } => {
                let Some(selected) = self.selected(me) else { return };
                let Some(model) = models.get(selected) else { return };
                let Some(effort) = model.efforts.get(index) else { return };
                let Some(choice) = &mut self.requested else { return };
                choice.main.effort = Some(effort.clone());
            }
            Act::Default => {
                self.context = Context::unselected(self.context);
                self.requested = self.original.clone();
            }
            Act::Apply => {}
        }
    }
    pub(crate) fn amend(&mut self, me: &c::Me, task: &c::Task, serial: &mut u64, out: &mut Queue<Request>) {
        let Some(Context::Task { project, task: number, revision, .. }) = self.context else { return };
        if self.pending.is_some() || self.requested == self.original || !self.valid(me) || number != task.chip.task {
            return;
        }
        let admission = jig_client::facts::offer(task.acts.as_slice(), c::ActKind::Amend);
        let change = Box::new(c::Amendment {
            addressed: None,
            spec: None,
            wake: None,
            dependencies: None,
            authority: None,
            instructions: None,
            tracked: None,
            llm: self.requested.clone(),
        });
        let Ok(reason) = c::Text::new(Box::from(*b"Change next run"), u32::MAX) else { return };
        let ask = match admission {
            Some(c::Admission::Allowed | c::Admission::Bounded { .. }) => {
                c::Ask::Amend { project, task: number, revision: Some(revision), change, reason }
            }
            Some(c::Admission::Proposed { .. }) => c::Ask::Propose {
                project,
                action: c::ProposeAction::Amendment { task: number, revision, change },
                reason,
            },
            Some(c::Admission::Refused(_)) | None => return,
        };
        *serial = serial.checked_add(1).expect("page keys fit");
        let key = Key { asker: Owner::Pages.asker(), number: *serial };
        self.pending = Some(key);
        self.outcome = None;
        out.push(Request::Client(jig_client::Event::Send {
            key,
            name: None,
            request: jig_client::Ask::Jig(Box::new(ask)),
        }));
    }
    pub(crate) fn told(&mut self, told: &Told) -> bool {
        if let Told::Answered { key, outcome } = told
            && self.pending == Some(*key)
        {
            self.pending = None;
            self.outcome = Some(outcome.clone());
            return true;
        }
        false
    }
}
pub(crate) fn refresh(pages: &mut Pages, client: &Client) {
    let Some(me) = accounts::current_me(client) else {
        pages.choice.reset(None, None);
        return;
    };
    let task = if let Some(chat) = &pages.chat {
        chat.task(client)
    } else if let Some(task) = &pages.task {
        task.document(client)
    } else {
        None
    };
    if let Some(task) = task {
        pages.choice.reset(
            Some(Context::Task {
                project: task.project,
                task: task.chip.task,
                revision: task.spec_revision,
                model_index: None,
            }),
            task.llm.as_ref(),
        );
    } else if pages.chats.is_some()
        && let Some(project) = pages.project
        && let Some(template) = jig_client::shapes::template(me, project, jig_client::shapes::CHAT)
    {
        pages.choice.reset(
            Some(Context::Template { project, executor: template.executor, model_index: None }),
            template.llm.as_ref(),
        );
    } else {
        pages.choice.reset(None, None);
    }
}
pub(crate) fn act(pages: &mut Pages, client: &Client, act: Act, out: &mut Queue<Request>) {
    let Some(me) = accounts::current_me(client) else { return };
    if act == Act::Apply {
        let task = if let Some(chat) = &pages.chat {
            chat.task(client)
        } else if let Some(task) = &pages.task {
            task.document(client)
        } else {
            None
        };
        if let Some(task) = task {
            pages.choice.amend(me, task, &mut pages.serial, out);
        }
    } else {
        pages.choice.edit(me, act);
    }
}
