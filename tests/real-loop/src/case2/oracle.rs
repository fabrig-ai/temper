//! One oracle per hour-0 judgement, over ctl reads and fake-peer observations.
use super::{COORDINATORS, COORDINATORS_CHARTER, Cast, MemberKind, PROJECT, acts::Read};
use crate::{Exit, Ran, calls::ReadCondition};
use serde_json::Value;
use skein_fake_forge_domain::Observation;

pub struct Query {
    pub system: Box<[u8]>,
    pub model: Box<[u8]>,
}
pub struct Observed {
    pub documents: Vec<(Read, Value)>,
    pub queries: Vec<Query>,
    pub forge: Vec<Observation>,
}
impl Observed {
    #[must_use]
    pub fn from_ran(ran: &Ran, reads: &[(Read, usize)]) -> Self {
        let mut documents = Vec::with_capacity(reads.len());
        for (read, at) in reads {
            if let Some(call) = ran.calls().get(*at)
                && call.exit == Some(Exit::Code(0))
                && let Ok(document) = serde_json::from_slice(&call.stdout)
            {
                documents.push((*read, document));
            }
        }
        let mut queries = Vec::with_capacity(ran.llm().len());
        for observation in ran.llm() {
            if let skein_fake_peers::llm::Observation::Query { query, .. } = observation {
                queries.push(Query { system: query.system.clone(), model: query.model.clone() });
            }
        }
        Self { documents, queries, forge: ran.forge().observations().to_vec() }
    }
    #[must_use]
    pub fn document(&self, read: Read) -> &Value {
        for (name, document) in &self.documents {
            if *name == read {
                return document;
            }
        }
        &Value::Null
    }
    #[must_use]
    pub fn task(&self, name: &'static str) -> &Value {
        let document = self.document(Read::Task(name));
        document.get("task").unwrap_or(document)
    }
}

#[must_use]
pub fn hour_0_p_and_its_implementers_run(cast: &Cast, observed: &Observed) -> bool {
    for member in cast.members {
        let task = cast.number(member.name);
        let writing = observed.document(Read::Writing(member.name));
        if !(ReadCondition::WritingTask { task }).matches(&serde_json::to_vec(writing).expect("read JSON")) {
            return false;
        }
        let document = observed.task(member.name);
        if !(ReadCondition::WritingTask { task }).matches(&serde_json::to_vec(document).expect("task JSON"))
            || document["project"] != PROJECT
            || document.pointer("/chip/executor/agent/charter").and_then(Value::as_u64)
                != Some(u64::from(member.charter))
            || document.pointer("/spec/specification/words/bytes").and_then(Value::as_str) != Some(member.spec)
        {
            return false;
        }
        match member.under {
            Some(parent)
                if document.pointer("/requester/task").and_then(Value::as_u64) != Some(cast.number(parent)) =>
            {
                return false;
            }
            None if document.pointer("/requester/person").and_then(Value::as_u64) != Some(1) => return false,
            Some(_) | None => {}
        }
        if let MemberKind::Goal { priority } = member.kind
            && (document["goal"] != task || document["priority"] != priority)
        {
            return false;
        }
        let mut queries = 0;
        for query in &observed.queries {
            if skein_lib::bytes::find(&query.system, member.cue.as_bytes()).is_some() {
                queries += 1;
                if (member.charter == COORDINATORS_CHARTER
                    && skein_lib::bytes::find(&query.system, COORDINATORS.as_bytes()).is_none())
                    || query.model.as_ref() != member.model.name().as_bytes()
                {
                    return false;
                }
            }
        }
        if queries == 0 {
            return false;
        }
    }
    true
}

#[must_use]
pub fn hour_0_nothing_held(cast: &Cast, observed: &Observed) -> bool {
    for member in cast.members {
        let task = observed.task(member.name);
        // Every task in this fresh cast is live and non-exempt: a subtree
        // placed hold would appear in its owner's hold and phase as well.
        if task.get("hold") != Some(&Value::Null)
            || task.pointer("/chip/phase").and_then(Value::as_str) != Some("running")
            || task.get("result") != Some(&Value::Null)
        {
            return false;
        }
    }
    // The fake setups contain no issues. Goals may project their own issue;
    // a finding is an issue carrying the finding label (draft section 5.2).
    for record in &observed.forge {
        if let Observation::Opened { labels, .. } | Observation::Labelled { labels, .. } = record {
            for label in labels {
                if label.as_ref() == b"finding" {
                    return false;
                }
            }
        }
    }
    true
}
