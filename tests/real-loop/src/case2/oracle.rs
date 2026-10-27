//! One oracle per hour-0 judgement, over ctl reads and fake-peer observations.
use super::{COORDINATORS, COORDINATORS_CHARTER, Cast, MemberKind, PROJECT, acts::Read};
use crate::{Exit, Ran, calls::ReadCondition};
use serde_json::Value;
use skein_fake_forge_domain::Observation;

pub struct Query {
    pub system: Box<[u8]>,
    pub model: Box<[u8]>,
    pub history: Box<[skein_fake_llm_domain::api::Message]>,
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
                queries.push(Query {
                    system: query.system.clone(),
                    model: query.model.clone(),
                    history: query.messages.clone(),
                });
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

fn task_at(observed: &Observed, read: Read) -> &Value {
    let doc = observed.document(read);
    doc.get("task").unwrap_or(doc)
}
#[must_use]
pub fn step_8_1_p_and_its_implementers_held_by_the_persons_stop(cast: &Cast, observed: &Observed) -> bool {
    use super::scripts::{HOLD_REVISION, STOP_HOLD};
    let placed = &task_at(observed, Read::Held("P"))["hold"]["placed"];
    if placed["hold"] != STOP_HOLD
        || placed["revision"] != HOLD_REVISION
        || placed["owner"]["person"] != 1
        || placed["lifted"] != false
    {
        return false;
    }
    for member in &cast.members[1..] {
        let held = task_at(observed, Read::Held(member.name));
        let waiting = task_at(observed, Read::Waiting(member.name));
        if held["chip"]["phase"] != "held"
            || held["hold"]["placed"] != *placed
            || waiting["hold"]["placed"] != *placed
            || waiting["chip"]["phase"] != "held"
        {
            return false;
        }
        let Some(rows) = observed.document(Read::Held(member.name)).pointer("/activity/rows").and_then(Value::as_array)
        else {
            return false;
        };
        let initial = observed.task(member.name).pointer("/chip/details/owner/details/attempt");
        let mut cancelled = false;
        for row in rows {
            let end = &row["line"]["run_ended"];
            if Some(&end["attempt"]) == initial && end["end"] == "cancelled" {
                if end["spent"] != 0 {
                    return false;
                }
                cancelled = true;
            }
        }
        if !cancelled {
            return false;
        }
    }
    true
}
#[must_use]
pub fn step_8_1_the_directors_release_refused_naming_the_person(cast: &Cast, observed: &Observed) -> bool {
    use skein_fake_llm_domain::api::Part;
    let placed = &task_at(observed, Read::Held("P"))["hold"]["placed"];
    let (Some(hold), Some(revision), Some(owner)) =
        (placed["hold"].as_u64(), placed["revision"].as_u64(), placed["owner"]["person"].as_u64())
    else {
        return false;
    };
    let p = cast.number("P");
    let expected = format!(
        "Error: Refused: task {p} is held by hold {hold} (revision {revision}), placed by person {owner}; only they may release it. Propose it: {{\"action\": \"release\", \"task\": {p}, \"hold\": {hold}, \"revision\": {revision}, \"reason\": \"...\"}}"
    );
    for query in member_queries(observed, &cast.members[0]) {
        for message in &query.history {
            for part in &message.parts {
                if let Part::ToolOutput { output, .. } = part
                    && output.as_ref() == expected.as_bytes()
                {
                    return true;
                }
            }
        }
    }
    false
}
#[must_use]
pub fn step_8_1_the_proposal_waits_in_the_persons_inbox(cast: &Cast, observed: &Observed) -> bool {
    let proposal = observed.document(Read::Proposal);
    let held = task_at(observed, Read::Held("P"));
    if proposal.pointer("/waiting/holder/person/value").and_then(Value::as_u64) != Some(1)
        || proposal.pointer("/action/release/task").and_then(Value::as_u64) != Some(cast.number("P"))
        || proposal.pointer("/action/release/hold/reason/placed/hold") != held.pointer("/hold/owner/reason/placed/hold")
        || proposal.pointer("/action/release/hold/revision") != held.pointer("/hold/owner/revision")
    {
        return false;
    }
    let Some(rows) = observed.document(Read::Inbox)["inbox"]["rows"].as_array() else { return false };
    let mut found = 0;
    for row in rows {
        if row.pointer("/object/proposal") == proposal.get("key") {
            found += 1;
        }
    }
    if found != 1 {
        return false;
    }
    // This fresh deployment has one person. No cast task gets this proposal;
    // the supplied waiting holder names that person alone, never a role.
    for member in cast.members {
        let Some(proposals) = task_at(observed, Read::Waiting(member.name))["proposals"].as_array() else {
            return false;
        };
        if !proposals.is_empty() {
            return false;
        }
    }
    true
}
fn member_queries<'a>(observed: &'a Observed, member: &super::Member) -> Vec<&'a Query> {
    let mut queries = Vec::with_capacity(observed.queries.len());
    for query in &observed.queries {
        if skein_lib::bytes::find(&query.system, member.cue.as_bytes()).is_some() {
            queries.push(query);
        }
    }
    queries
}
fn transcript_kept(before: &Query, resumed: &Query) -> bool {
    use skein_fake_llm_domain::api::{Part, Role};
    let mut remaining = resumed.history.iter();
    let mut saved = 0;
    for message in &before.history {
        let mut committed = message.role == Role::Assistant;
        for part in &message.parts {
            committed |= matches!(part, Part::ToolOutput { .. });
        }
        if !committed {
            continue;
        }
        let mut found = false;
        for next in remaining.by_ref() {
            if next == message {
                found = true;
                break;
            }
        }
        if !found {
            return false;
        }
        saved += 1;
    }
    saved >= 2
}
#[must_use]
pub fn step_8_1_p_resumes_from_its_transcript(cast: &Cast, observed: &Observed) -> bool {
    for member in &cast.members[1..] {
        let resumed = task_at(observed, Read::Resumed(member.name));
        let parked = task_at(observed, Read::Parked(member.name));
        if resumed.get("hold") != Some(&Value::Null)
            || parked.get("hold") != Some(&Value::Null)
            || parked["chip"]["phase"] != "idle"
            || parked.get("result") != Some(&Value::Null)
        {
            return false;
        }
        let mut initial = 2; // documentation read, then writing
        for child in cast.members {
            initial += usize::from(child.under == Some(member.name));
        }
        let queries = member_queries(observed, member);
        if queries.len() <= initial || !transcript_kept(queries[initial - 1], queries[initial]) {
            return false;
        }
        if resumed.pointer("/chip/details/owner/details/attempt").and_then(Value::as_u64)
            <= observed.task(member.name).pointer("/chip/details/owner/details/attempt").and_then(Value::as_u64)
        {
            return false;
        }
    }
    true
}
