//! A task's controls as read, and whole replacements for its edits (client/README.md, 3.7).
use jig_client::contract as c;
use skein_lib::{List, Wall};

#[derive(Debug)]
pub struct Draft {
    revision: c::Revision,
    tracked: c::Tracking,
    spec: c::SpecificationDocument,
    authority: Option<c::OwnerAuthority>,
    instructions: Option<c::DocumentText>,
    dependencies: List<u64>,
    llm: Option<c::LlmChoice>,
    change: c::Amendment,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cut {
    Authority,
    Spec,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drafted {
    pub revision: Option<c::Revision>,
    pub change: c::Amendment,
}
impl Draft {
    #[must_use]
    pub fn of(task: &c::Task) -> Self {
        let tracked = match task.priority {
            Some(priority) => c::Tracking::Tracked { priority },
            None => c::Tracking::Untracked,
        };
        let mut dependencies = List::with_capacity(task.dependencies.len());
        for link in &task.dependencies {
            dependencies.push(link.number()).expect("same supplied dependency length");
        }
        Self {
            revision: task.spec_revision,
            tracked,
            spec: task.spec.clone(),
            authority: task.authority.whole(),
            instructions: task.instructions.clone(),
            dependencies,
            llm: task.llm.clone(),
            change: c::Amendment {
                addressed: None,
                spec: None,
                wake: None,
                dependencies: None,
                authority: None,
                instructions: None,
                tracked: None,
                llm: None,
            },
        }
    }
    #[must_use]
    pub fn revision(&self) -> c::Revision {
        self.revision
    }
    #[must_use]
    pub fn moved(&self, task: &c::Task) -> bool {
        task.spec_revision != self.revision
    }
    pub fn track(&mut self, tracking: c::Tracking) {
        self.change.tracked = if tracking == self.tracked { None } else { Some(tracking) };
    }
    pub fn deadline(&mut self, deadline: Option<Wall>) -> Result<(), Cut> {
        let base = self.authority.as_ref().ok_or(Cut::Authority)?;
        self.change.authority = if deadline == base.deadline {
            None
        } else {
            let mut authority = base.clone();
            authority.deadline = deadline;
            Some(authority)
        };
        Ok(())
    }
    pub fn instructions(&mut self, text: c::Text) {
        let same = match &self.instructions {
            Some(base) => !base.cut && base.bytes == text,
            None => text.bytes().is_empty(),
        };
        self.change.instructions = if same { None } else { Some(text) };
    }
    pub fn words(&mut self, words: c::Text) -> Result<(), Cut> {
        // Old words may be cut: they are replaced whole at the shown revision.
        // Preserve parameters and inputs only when those were supplied whole.
        let mut replacement = self.spec.clone();
        replacement.specification.words = c::DocumentText::complete(words.clone());
        let replacement = replacement.whole().ok_or(Cut::Spec)?;
        self.change.spec = if !self.spec.specification.words.cut && words == self.spec.specification.words.bytes {
            None
        } else {
            Some(replacement)
        };
        Ok(())
    }
    pub fn dependencies(&mut self, numbers: List<u64>) {
        self.change.dependencies = if numbers == self.dependencies { None } else { Some(numbers) };
    }
    pub fn llm(&mut self, choice: c::LlmChoice) {
        self.change.llm = if Some(&choice) == self.llm.as_ref() { None } else { Some(choice) };
    }
    #[must_use]
    pub fn amendment(&self) -> Option<Drafted> {
        let other = self.change.spec.is_some()
            || self.change.authority.is_some()
            || self.change.instructions.is_some()
            || self.change.dependencies.is_some()
            || self.change.llm.is_some();
        if !other && self.change.tracked.is_none() {
            return None;
        }
        let priority_alone = match self.tracked {
            c::Tracking::Tracked { .. } => match self.change.tracked {
                Some(c::Tracking::Tracked { .. }) => !other,
                Some(c::Tracking::Untracked) | None => false,
            },
            c::Tracking::Untracked => false,
        };
        Some(Drafted { revision: if priority_alone { None } else { Some(self.revision) }, change: self.change.clone() })
    }
}
