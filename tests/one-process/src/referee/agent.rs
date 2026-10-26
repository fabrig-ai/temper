//! The worker's actual agent pipe bytes, decoded independently of its root.
use jig_conformance::referee::Observed;
use skein_io::kernel::{Complete, Done, Op};
use skein_lib::Reader;
use std::collections::BTreeMap;

pub(crate) enum Record {
    Start { task: u64, attempt: u64, workspace: Option<smith_channel::Workspace> },
    Answer { task: u64, attempt: u64, result: smith_channel::RunResult },
}
impl std::fmt::Debug for Record {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Start { task, attempt, workspace } => f
                .debug_struct("Start")
                .field("task", task)
                .field("attempt", attempt)
                .field("workspace", workspace)
                .finish(),
            Self::Answer { task, attempt, result } => {
                f.debug_struct("Answer").field("task", task).field("attempt", attempt).field("result", result).finish()
            }
        }
    }
}

pub(crate) struct Wire {
    pipes: BTreeMap<i32, (bool, i32, Vec<u8>)>,
    limits: smith_channel::Limits,
    store: std::rc::Rc<std::cell::RefCell<super::Store>>,
    runs: BTreeMap<i32, (u64, u64)>,
    pub events: Vec<Observed>,
}
impl Wire {
    pub(crate) fn new(store: std::rc::Rc<std::cell::RefCell<super::Store>>) -> Self {
        Self {
            pipes: BTreeMap::new(),
            limits: crate::fixtures::agent_limits().channel.bodies,
            store,
            runs: BTreeMap::new(),
            events: Vec::new(),
        }
    }
    pub fn complete(&mut self, complete: &Complete) {
        match (&complete.kind, &complete.result) {
            (Op::Spawn { spawn }, Ok(Done::Spawned { .. })) if spawn.program.as_ref() == b"/opt/temper/bin/temper" => {
                for entry in &spawn.env {
                    super::secrets::require(entry, "agent environment");
                }
                let identity = spawn
                    .pipes
                    .iter()
                    .find(|pipe| pipe.child == 0)
                    .expect("agent input pipe")
                    .parent
                    .expect("real agent input pipe")
                    .raw();
                self.runs.remove(&identity);
                for pipe in spawn.pipes.iter().filter(|pipe| pipe.child < 2) {
                    self.pipes
                        .insert(pipe.parent.expect("real agent pipe").raw(), (pipe.child == 0, identity, Vec::new()));
                }
            }
            (Op::Write { fd, bytes, from, .. } | Op::PipeWrite { fd, bytes, from }, Ok(Done::Count(count))) => {
                self.bytes(fd.raw(), &bytes[*from as usize..(*from + *count) as usize]);
            }
            (Op::Read { fd, buf, .. } | Op::PipeRead { fd, buf }, Ok(Done::Count(count))) => {
                self.bytes(fd.raw(), &buf[..*count as usize]);
            }
            (Op::Close { fd }, _) => {
                self.pipes.remove(&fd.raw());
            }
            _ => {}
        }
    }
    fn bytes(&mut self, fd: i32, bytes: &[u8]) {
        let Some((down, identity, buffer)) = self.pipes.get_mut(&fd) else { return };
        buffer.extend_from_slice(bytes);
        let mut frames = Vec::new();
        while buffer.len() >= 8 {
            let kind = u16::from_be_bytes(buffer[..2].try_into().expect("frame kind"));
            assert_eq!(&buffer[2..4], &[0, 0], "agent frame flags");
            let length = u32::from_be_bytes(buffer[4..8].try_into().expect("frame length")) as usize;
            if buffer.len() < 8 + length {
                break;
            }
            frames.push((*down, *identity, kind, buffer[8..8 + length].to_vec()));
            buffer.drain(..8 + length);
        }
        for (down, identity, kind, body) in frames {
            self.frame(down, identity, kind, &body);
        }
    }
    fn frame(&mut self, down: bool, identity_fd: i32, kind: u16, bytes: &[u8]) {
        let limits = &self.limits;
        if down && kind == 0x0100 {
            let start = smith_channel::Start::decode(limits, &mut Reader::new(bytes)).expect("agent Start bytes");
            let store = self.store.borrow();
            let proof = store
                .rows
                .values()
                .find_map(|row| match row {
                    temper_engine_domain::Record::Core(jig_domain_core::Record::Core(
                        jig_domain_core::CoreRecord::RunProof(proof),
                    )) if proof.attempt == start.activation()
                        && Some(identity(proof.task, proof.conversation)) == *start.identity() =>
                    {
                        Some(proof)
                    }
                    temper_engine_domain::Record::Core(_)
                    | temper_engine_domain::Record::Forge { .. }
                    | temper_engine_domain::Record::Delivery { .. } => None,
                })
                .expect("agent identity names a durable claim");
            self.runs.insert(identity_fd, (proof.task, start.activation()));
            let charter = smith_charter::Charter::decode(&smith_charter::CEILINGS, &mut Reader::new(start.charter()))
                .expect("agent charter bytes");
            self.events.push(Observed::Assigned {
                task: proof.task,
                attempt: start.activation(),
                conversation: proof.conversation,
                resumed: !start.transcript().is_empty(),
                budget: charter.budget().spend(),
                // Only the drain/restart story changes revision 1 through ctl.
                // Other stories start work on their own defaults at revision 1.
                // Smith's Start carries no settings-revision metadata.
                settings: crate::stories::settings_revision(),
            });
            let task = proof.task;
            drop(store);
            self.store.borrow_mut().agent_records.push(Record::Start {
                task,
                attempt: start.activation(),
                workspace: start.workspace().clone(),
            });
        } else if !down && kind == 0x0109 {
            let (task, attempt) = *self.runs.get(&identity_fd).expect("turn after agent Start");
            let turn = smith_channel::Turn::decode(limits, &mut Reader::new(bytes)).expect("agent Turn bytes");
            self.events.push(Observed::Completion { task, attempt, cumulative: turn.spent() });
        } else if !down && kind == 0x0110 {
            let answer = smith_channel::Answer::decode(limits, &mut Reader::new(bytes)).expect("agent Answer bytes");
            if matches!(answer.result(), smith_channel::RunResult::Failed(_) | smith_channel::RunResult::Refused(_)) {
                let (task, attempt) = *self.runs.get(&identity_fd).expect("answer after agent Start");
                self.store.borrow_mut().agent_records.push(Record::Answer {
                    task,
                    attempt,
                    result: answer.result().clone(),
                });
            }
        }
        // The worker's agent ACK grants retention credit, not durable engine commitment.
        // Commit prerequisites are checked on the client's actual released documents.
    }
}

fn identity(task: u64, conversation: u64) -> [u8; 16] {
    let bytes =
        [crate::fixtures::seeds().deployment.as_slice(), &task.to_be_bytes(), &conversation.to_be_bytes()].concat();
    let digest = skein_io::digest::digest(&bytes);
    [digest.0[0].to_be_bytes(), digest.0[1].to_be_bytes()].concat().try_into().expect("sixteen identity bytes")
}
