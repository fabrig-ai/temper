use super::{FITS, names, w};
use alloc::boxed::Box;
use skein_lib::{List, Wall};
use temper_engine_domain_forge as top;
use temper_engine_domain_forge_change as d;
fn encode_status(value: d::Status) -> w::ChangeStatus {
    match value {
        d::Status::Unknown => w::ChangeStatus::Unknown,
        d::Status::Pending => w::ChangeStatus::Pending,
        d::Status::Passed => w::ChangeStatus::Passed,
        d::Status::Failed => w::ChangeStatus::Failed,
    }
}
fn decode_status(value: w::ChangeStatus) -> d::Status {
    match value {
        w::ChangeStatus::Unknown => d::Status::Unknown,
        w::ChangeStatus::Pending => d::Status::Pending,
        w::ChangeStatus::Passed => d::Status::Passed,
        w::ChangeStatus::Failed => d::Status::Failed,
    }
}
fn encode_freshness(value: d::Freshness) -> w::Freshness {
    match value {
        d::Freshness::Exact => w::Freshness::Exact,
        d::Freshness::Clean => w::Freshness::Clean,
    }
}
fn decode_freshness(value: w::Freshness) -> d::Freshness {
    match value {
        w::Freshness::Exact => d::Freshness::Exact,
        w::Freshness::Clean => d::Freshness::Clean,
    }
}
fn encode_gate_kind(value: d::GateKind) -> w::GateKind {
    match value {
        d::GateKind::Agent => w::GateKind::Agent,
        d::GateKind::Person => w::GateKind::Person,
        d::GateKind::Check => w::GateKind::Check,
    }
}
fn decode_gate_kind(value: w::GateKind) -> d::GateKind {
    match value {
        w::GateKind::Agent => d::GateKind::Agent,
        w::GateKind::Person => d::GateKind::Person,
        w::GateKind::Check => d::GateKind::Check,
    }
}
fn encode_hold(value: d::Hold) -> w::ChangeHold {
    match value {
        d::Hold::Stalled => w::ChangeHold::Stalled,
        d::Hold::Repairs => w::ChangeHold::Repairs,
        d::Hold::Resolutions => w::ChangeHold::Resolutions,
        d::Hold::Updates => w::ChangeHold::Updates,
        d::Hold::BranchMoved => w::ChangeHold::BranchMoved,
        d::Hold::BranchMissing => w::ChangeHold::BranchMissing,
        d::Hold::PullClosed => w::ChangeHold::PullClosed,
        d::Hold::Retargeted => w::ChangeHold::Retargeted,
        d::Hold::Failed => w::ChangeHold::Failed,
        d::Hold::Rejected => w::ChangeHold::Rejected,
        d::Hold::Authority(cause) => w::ChangeHold::Authority(
            w::AuthorityHold::new(
                &w::CEILINGS,
                w::AuthorityHoldParts {
                    lack: match cause.lack {
                        d::AuthorityLack::Tasks => w::AuthorityLack::Tasks,
                        d::AuthorityLack::Depth => w::AuthorityLack::Depth,
                        d::AuthorityLack::Spend => w::AuthorityLack::Spend,
                    },
                    had: cause.had,
                    task: cause.recovery.task,
                    spec: cause.recovery.spec,
                    hold: cause.recovery.hold,
                    tasks: cause.recovery.tasks,
                    released: cause.released,
                },
            )
            .expect(FITS),
        ),
    }
}
fn decode_hold(value: w::ChangeHold) -> d::Hold {
    match value {
        w::ChangeHold::Stalled => d::Hold::Stalled,
        w::ChangeHold::Repairs => d::Hold::Repairs,
        w::ChangeHold::Resolutions => d::Hold::Resolutions,
        w::ChangeHold::Updates => d::Hold::Updates,
        w::ChangeHold::BranchMoved => d::Hold::BranchMoved,
        w::ChangeHold::BranchMissing => d::Hold::BranchMissing,
        w::ChangeHold::PullClosed => d::Hold::PullClosed,
        w::ChangeHold::Retargeted => d::Hold::Retargeted,
        w::ChangeHold::Failed => d::Hold::Failed,
        w::ChangeHold::Rejected => d::Hold::Rejected,
        w::ChangeHold::Authority(cause) => d::Hold::Authority(d::AuthorityHold {
            lack: match cause.lack() {
                w::AuthorityLack::Tasks => d::AuthorityLack::Tasks,
                w::AuthorityLack::Depth => d::AuthorityLack::Depth,
                w::AuthorityLack::Spend => d::AuthorityLack::Spend,
            },
            had: cause.had(),
            recovery: d::AuthorityRecovery {
                task: cause.task(),
                spec: cause.spec(),
                hold: cause.hold(),
                tasks: cause.tasks(),
            },
            released: cause.released(),
        }),
    }
}
fn encode_effect_result(value: d::EffectResult) -> w::EffectResult {
    match value {
        d::EffectResult::None => w::EffectResult::None,
        d::EffectResult::Pending => w::EffectResult::Pending,
        d::EffectResult::Made => w::EffectResult::Made,
        d::EffectResult::Conflict => w::EffectResult::Conflict,
        d::EffectResult::Failed => w::EffectResult::Failed,
    }
}
fn decode_effect_result(value: w::EffectResult) -> d::EffectResult {
    match value {
        w::EffectResult::None => d::EffectResult::None,
        w::EffectResult::Pending => d::EffectResult::Pending,
        w::EffectResult::Made => d::EffectResult::Made,
        w::EffectResult::Conflict => d::EffectResult::Conflict,
        w::EffectResult::Failed => d::EffectResult::Failed,
    }
}
fn encode_repair(value: d::Repair) -> w::Repair {
    match value {
        d::Repair::Ci => w::Repair::Ci,
        d::Repair::Gate(number) => {
            w::Repair::Gate(w::Number::new(&w::CEILINGS, w::NumberParts { number }).expect(FITS))
        }
        d::Repair::Semantic => w::Repair::Semantic,
    }
}
fn decode_repair(value: w::Repair) -> d::Repair {
    match value {
        w::Repair::Ci => d::Repair::Ci,
        w::Repair::Gate(value) => d::Repair::Gate(value.number()),
        w::Repair::Semantic => d::Repair::Semantic,
    }
}
fn encode_delegate(value: d::Delegate) -> w::Delegate {
    match value {
        d::Delegate::Produce => w::Delegate::Produce,
        d::Delegate::Repair(why) => w::Delegate::Repair(
            w::RepairValue::new(&w::CEILINGS, w::RepairValueParts { why: encode_repair(why) }).expect(FITS),
        ),
        d::Delegate::Resolve { base } => {
            w::Delegate::Resolve(w::Commit::new(&w::CEILINGS, w::CommitParts { commit: base }).expect(FITS))
        }
        d::Delegate::Gate { number, head } => {
            w::Delegate::Gate(w::DelegateGate::new(&w::CEILINGS, w::DelegateGateParts { number, head }).expect(FITS))
        }
    }
}
fn decode_delegate(value: w::Delegate) -> d::Delegate {
    match value {
        w::Delegate::Produce => d::Delegate::Produce,
        w::Delegate::Repair(value) => d::Delegate::Repair(decode_repair(value.into_parts().why)),
        w::Delegate::Resolve(value) => d::Delegate::Resolve { base: value.into_parts().commit },
        w::Delegate::Gate(value) => {
            let w::DelegateGateParts { number, head } = value.into_parts();
            d::Delegate::Gate { number, head }
        }
    }
}
fn encode_live(value: &d::State) -> w::ChangeLive {
    match value {
        d::State::Ended => w::ChangeLive::Ended,
        d::State::Producing { requested } => w::ChangeLive::Producing(
            w::Producing::new(&w::CEILINGS, w::ProducingParts { requested: *requested }).expect(FITS),
        ),
        d::State::Opening { requested } => w::ChangeLive::Opening(
            w::Producing::new(&w::CEILINGS, w::ProducingParts { requested: *requested }).expect(FITS),
        ),
        d::State::Recreating { head } => {
            w::ChangeLive::Recreating(w::ChangeHead::new(&w::CEILINGS, w::ChangeHeadParts { head: *head }).expect(FITS))
        }
        d::State::Reopening { requested, retarget } => w::ChangeLive::Reopening(
            w::Reopening::new(&w::CEILINGS, w::ReopeningParts { requested: *requested, retarget: *retarget })
                .expect(FITS),
        ),
        d::State::Checking { head } => {
            w::ChangeLive::Checking(w::ChangeHead::new(&w::CEILINGS, w::ChangeHeadParts { head: *head }).expect(FITS))
        }
        d::State::CancellingGates { head } => w::ChangeLive::CancellingGates(
            w::ChangeHead::new(&w::CEILINGS, w::ChangeHeadParts { head: *head }).expect(FITS),
        ),
        d::State::Gating { head, asked } => w::ChangeLive::Gating(
            w::Gating::new(&w::CEILINGS, w::GatingParts { head: *head, asked: encode_numbers(asked) }).expect(FITS),
        ),
        d::State::Queued { head, ready_since } => w::ChangeLive::Queued(
            w::ReadyHead::new(&w::CEILINGS, w::ReadyHeadParts { head: *head, ready_since: ready_since.as_nanos() })
                .expect(FITS),
        ),
        d::State::First { head, ready_since } => w::ChangeLive::First(
            w::ReadyHead::new(&w::CEILINGS, w::ReadyHeadParts { head: *head, ready_since: ready_since.as_nanos() })
                .expect(FITS),
        ),
        d::State::Updating { from, base, ready_since } => w::ChangeLive::Updating(
            w::Updating::new(
                &w::CEILINGS,
                w::UpdatingParts { from: *from, base: *base, ready_since: ready_since.as_nanos() },
            )
            .expect(FITS),
        ),
        d::State::Resolving { requested, base } => w::ChangeLive::Resolving(
            w::Resolving::new(&w::CEILINGS, w::ResolvingParts { requested: *requested, base: *base }).expect(FITS),
        ),
        d::State::Repairing { requested, why } => w::ChangeLive::Repairing(
            w::Repairing::new(&w::CEILINGS, w::RepairingParts { requested: *requested, why: encode_repair(*why) })
                .expect(FITS),
        ),
        d::State::Landing { head } => {
            w::ChangeLive::Landing(w::ChangeHead::new(&w::CEILINGS, w::ChangeHeadParts { head: *head }).expect(FITS))
        }
        d::State::Landed { merge } => w::ChangeLive::Landed(
            w::LandedChange::new(&w::CEILINGS, w::LandedChangeParts { merge: *merge }).expect(FITS),
        ),
        d::State::Held { .. } => unreachable!("the bounded loop unwrapped every hold"),
    }
}
fn decode_live(value: w::ChangeLive) -> d::State {
    match value {
        w::ChangeLive::Ended => d::State::Ended,
        w::ChangeLive::Producing(value) => {
            let w::ProducingParts { requested } = value.into_parts();
            d::State::Producing { requested }
        }
        w::ChangeLive::Opening(value) => {
            let w::ProducingParts { requested } = value.into_parts();
            d::State::Opening { requested }
        }
        w::ChangeLive::Recreating(value) => {
            let w::ChangeHeadParts { head } = value.into_parts();
            d::State::Recreating { head }
        }
        w::ChangeLive::Reopening(value) => {
            let w::ReopeningParts { requested, retarget } = value.into_parts();
            d::State::Reopening { requested, retarget }
        }
        w::ChangeLive::Checking(value) => {
            let w::ChangeHeadParts { head } = value.into_parts();
            d::State::Checking { head }
        }
        w::ChangeLive::CancellingGates(value) => {
            let w::ChangeHeadParts { head } = value.into_parts();
            d::State::CancellingGates { head }
        }
        w::ChangeLive::Gating(value) => {
            let w::GatingParts { head, asked } = value.into_parts();
            d::State::Gating { head, asked: asked.into_boxed() }
        }
        w::ChangeLive::Queued(value) => {
            let w::ReadyHeadParts { head, ready_since } = value.into_parts();
            d::State::Queued { head, ready_since: Wall::from_nanos(ready_since) }
        }
        w::ChangeLive::First(value) => {
            let w::ReadyHeadParts { head, ready_since } = value.into_parts();
            d::State::First { head, ready_since: Wall::from_nanos(ready_since) }
        }
        w::ChangeLive::Updating(value) => {
            let w::UpdatingParts { from, base, ready_since } = value.into_parts();
            d::State::Updating { from, base, ready_since: Wall::from_nanos(ready_since) }
        }
        w::ChangeLive::Resolving(value) => {
            let w::ResolvingParts { requested, base } = value.into_parts();
            d::State::Resolving { requested, base }
        }
        w::ChangeLive::Repairing(value) => {
            let w::RepairingParts { requested, why } = value.into_parts();
            d::State::Repairing { requested, why: decode_repair(why) }
        }
        w::ChangeLive::Landing(value) => {
            let w::ChangeHeadParts { head } = value.into_parts();
            d::State::Landing { head }
        }
        w::ChangeLive::Landed(value) => {
            let w::LandedChangeParts { merge } = value.into_parts();
            d::State::Landed { merge }
        }
    }
}
fn encode_change_state(state: &d::State) -> w::ChangeState {
    let mut holds = List::with_capacity(8);
    let mut current = state;
    for _ in 0..=8_u32 {
        let d::State::Held { was, why } = current else {
            break;
        };
        holds.push(encode_hold(*why)).expect("at most 8 holds (change_state_holds)");
        current = was;
    }
    w::ChangeState::new(&w::CEILINGS, w::ChangeStateParts { live: encode_live(current), holds }).expect(FITS)
}
fn decode_change_state(value: w::ChangeState) -> d::State {
    let w::ChangeStateParts { live, holds } = value.into_parts();
    let mut state = decode_live(live);
    for why in holds.into_boxed().into_iter().rev() {
        state = d::State::Held { was: Box::new(state), why: decode_hold(why) };
    }
    state
}
pub(super) fn encode_gate(value: &d::Gate) -> w::Gate {
    let d::Gate { number, kind, blocking, freshness, eager } = value;
    w::Gate::new(
        &w::CEILINGS,
        w::GateParts {
            number: *number,
            kind: encode_gate_kind(*kind),
            blocking: *blocking,
            freshness: encode_freshness(*freshness),
            eager: *eager,
        },
    )
    .expect(FITS)
}
pub(super) fn decode_gate(value: w::Gate) -> d::Gate {
    let w::GateParts { number, kind, blocking, freshness, eager } = value.into_parts();
    d::Gate { number, kind: decode_gate_kind(kind), blocking, freshness: decode_freshness(freshness), eager }
}
pub(super) fn encode_gate_report(value: &d::GateReport) -> w::GateReport {
    let d::GateReport { number, head, status } = value;
    w::GateReport::new(
        &w::CEILINGS,
        w::GateReportParts { number: *number, head: *head, status: encode_status(*status) },
    )
    .expect(FITS)
}
pub(super) fn decode_gate_report(value: w::GateReport) -> d::GateReport {
    let w::GateReportParts { number, head, status } = value.into_parts();
    d::GateReport { number, head, status: decode_status(status) }
}
pub(super) fn encode_gate_remark(value: &top::GateRemark) -> w::GateRemark {
    let top::GateRemark { number, head, words } = value;
    w::GateRemark::new(&w::CEILINGS, w::GateRemarkParts { number: *number, head: *head, words: words.clone() })
        .expect(FITS)
}
pub(super) fn decode_gate_remark(value: w::GateRemark) -> top::GateRemark {
    let w::GateRemarkParts { number, head, words } = value.into_parts();
    top::GateRemark { number, head, words }
}
#[expect(clippy::manual_map, reason = "step translations use explicit matches without closures")]
pub(super) fn encode_change(value: &d::Change) -> w::Change {
    let d::Change {
        task,
        state,
        gates,
        clean,
        repairs,
        resolutions,
        updates,
        since,
        ready_since,
        owns_turn,
        last_head,
        gate_failures,
        released,
    } = value;
    w::Change::new(
        &w::CEILINGS,
        w::ChangeParts {
            task: *task,
            state: encode_change_state(state),
            gates: encode_gates(gates),
            clean: encode_commits(clean),
            repairs: *repairs,
            resolutions: *resolutions,
            updates: *updates,
            since: since.as_nanos(),
            ready_since: match ready_since {
                Some(value) => Some(value.as_nanos()),
                None => None,
            },
            owns_turn: *owns_turn,
            last_head: *last_head,
            gate_failures: encode_gate_failures(gate_failures),
            released: match released {
                Some(why) => Some(encode_hold(*why)),
                None => None,
            },
        },
    )
    .expect(FITS)
}
#[expect(clippy::manual_map, reason = "step translations use explicit matches without closures")]
pub(super) fn decode_change(value: w::Change) -> d::Change {
    let w::ChangeParts {
        task,
        state,
        gates,
        clean,
        repairs,
        resolutions,
        updates,
        since,
        ready_since,
        owns_turn,
        last_head,
        gate_failures,
        released,
    } = value.into_parts();
    d::Change {
        task,
        state: decode_change_state(state),
        gates: decode_gates(gates),
        clean: decode_commits(clean),
        repairs,
        resolutions,
        updates,
        since: Wall::from_nanos(since),
        ready_since: match ready_since {
            Some(value) => Some(Wall::from_nanos(value)),
            None => None,
        },
        owns_turn,
        last_head,
        gate_failures: decode_gate_failures(gate_failures),
        released: match released {
            Some(why) => Some(decode_hold(why)),
            None => None,
        },
    }
}
#[expect(clippy::manual_map, reason = "step translations use explicit matches without closures")]
pub(super) fn encode_change_row(value: &top::ChangeRow) -> w::ChangeRow {
    let top::ChangeRow {
        task,
        repository,
        branch,
        base,
        title,
        body,
        priority,
        change,
        ci,
        pull,
        pending,
        effect,
        delegate,
        delegate_status,
        verdicts,
        gate_remarks,
        drift,
        base_repair,
        queue_repair,
        queue_repairs,
        producing_charter,
        review_charter,
    } = value;
    w::ChangeRow::new(
        &w::CEILINGS,
        w::ChangeRowParts {
            task: *task,
            repository: names::encode_repository(*repository),
            branch: branch.clone(),
            base: base.clone(),
            title: title.clone(),
            body: body.clone(),
            priority: u32::from_be_bytes(priority.to_be_bytes()),
            change: encode_change(change),
            ci: match ci {
                Some(read) => Some(
                    w::ChangeCi::new(
                        &w::CEILINGS,
                        w::ChangeCiParts { head: read.head, status: encode_status(read.status) },
                    )
                    .expect(FITS),
                ),
                None => None,
            },
            pull: *pull,
            pending: *pending,
            effect: encode_effect_result(*effect),
            delegate: match delegate {
                Some((task, request)) => Some(
                    w::DelegateRef::new(
                        &w::CEILINGS,
                        w::DelegateRefParts { task: *task, request: encode_delegate(*request) },
                    )
                    .expect(FITS),
                ),
                None => None,
            },
            delegate_status: encode_status(*delegate_status),
            verdicts: encode_gate_reports(verdicts),
            gate_remarks: encode_gate_remarks(gate_remarks),
            drift: match drift {
                Some(value) => Some(encode_hold(*value)),
                None => None,
            },
            base_repair: *base_repair,
            queue_repair: *queue_repair,
            queue_repairs: *queue_repairs,
            producing_charter: *producing_charter,
            review_charter: *review_charter,
        },
    )
    .expect(FITS)
}
#[expect(clippy::manual_map, reason = "step translations use explicit matches without closures")]
pub(super) fn decode_change_row(value: w::ChangeRow) -> top::ChangeRow {
    let w::ChangeRowParts {
        task,
        repository,
        branch,
        base,
        title,
        body,
        priority,
        change,
        ci,
        pull,
        pending,
        effect,
        delegate,
        delegate_status,
        verdicts,
        gate_remarks,
        drift,
        base_repair,
        queue_repair,
        queue_repairs,
        producing_charter,
        review_charter,
    } = value.into_parts();
    top::ChangeRow {
        task,
        repository: names::decode_repository(repository),
        branch,
        base,
        title,
        body,
        priority: i32::from_be_bytes(priority.to_be_bytes()),
        change: decode_change(change),
        ci: match ci {
            Some(read) => {
                let w::ChangeCiParts { head, status } = read.into_parts();
                Some(d::Ci { head, status: decode_status(status) })
            }
            None => None,
        },
        pull,
        pending,
        effect: decode_effect_result(effect),
        delegate: match delegate {
            Some(value) => {
                let w::DelegateRefParts { task, request } = value.into_parts();
                Some((task, decode_delegate(request)))
            }
            None => None,
        },
        delegate_status: decode_status(delegate_status),
        verdicts: decode_gate_reports(verdicts),
        gate_remarks: decode_gate_remarks(gate_remarks),
        drift: match drift {
            Some(value) => Some(decode_hold(value)),
            None => None,
        },
        base_repair,
        queue_repair,
        queue_repairs,
        producing_charter,
        review_charter,
    }
}
fn encode_numbers(values: &[u64]) -> List<u64> {
    let mut result = List::with_capacity(u32::try_from(values.len()).expect("admitted count"));
    for value in values {
        let item = *value;
        result.push(item).expect("same count");
    }
    result
}
fn encode_commits(values: &[[u8; 32]]) -> List<[u8; 32]> {
    let mut result = List::with_capacity(u32::try_from(values.len()).expect("admitted count"));
    for value in values {
        let item = *value;
        result.push(item).expect("same count");
    }
    result
}
fn decode_commits(values: List<[u8; 32]>) -> Box<[[u8; 32]]> {
    let mut result = List::with_capacity(values.len());
    for value in values.into_boxed() {
        let item = value;
        result.push(item).expect("same count");
    }
    result.into_boxed()
}
fn encode_gates(values: &[d::Gate]) -> List<w::Gate> {
    let mut result = List::with_capacity(u32::try_from(values.len()).expect("admitted count"));
    for value in values {
        let item = encode_gate(value);
        result.push(item).expect("same count");
    }
    result
}
fn decode_gates(values: List<w::Gate>) -> Box<[d::Gate]> {
    let mut result = List::with_capacity(values.len());
    for value in values.into_boxed() {
        let item = decode_gate(value);
        result.push(item).expect("same count");
    }
    result.into_boxed()
}
fn encode_gate_reports(values: &[d::GateReport]) -> List<w::GateReport> {
    let mut result = List::with_capacity(u32::try_from(values.len()).expect("admitted count"));
    for value in values {
        let item = encode_gate_report(value);
        result.push(item).expect("same count");
    }
    result
}
fn decode_gate_reports(values: List<w::GateReport>) -> Box<[d::GateReport]> {
    let mut result = List::with_capacity(values.len());
    for value in values.into_boxed() {
        let item = decode_gate_report(value);
        result.push(item).expect("same count");
    }
    result.into_boxed()
}
fn encode_gate_remarks(values: &[top::GateRemark]) -> List<w::GateRemark> {
    let mut result = List::with_capacity(u32::try_from(values.len()).expect("admitted count"));
    for value in values {
        let item = encode_gate_remark(value);
        result.push(item).expect("same count");
    }
    result
}
fn decode_gate_remarks(values: List<w::GateRemark>) -> Box<[top::GateRemark]> {
    let mut result = List::with_capacity(values.len());
    for value in values.into_boxed() {
        let item = decode_gate_remark(value);
        result.push(item).expect("same count");
    }
    result.into_boxed()
}

fn encode_gate_failures(values: &[d::GateFailure]) -> List<w::GateFailure> {
    let mut result = List::with_capacity(u32::try_from(values.len()).expect("admitted count"));
    for value in values {
        let d::GateFailure { number, count } = value;
        result
            .push(
                w::GateFailure::new(&w::CEILINGS, w::GateFailureParts { number: *number, count: *count }).expect(FITS),
            )
            .expect("same count");
    }
    result
}
fn decode_gate_failures(values: List<w::GateFailure>) -> Box<[d::GateFailure]> {
    let mut result = List::with_capacity(values.len());
    for value in values.into_boxed() {
        let w::GateFailureParts { number, count } = value.into_parts();
        result.push(d::GateFailure { number, count }).expect("same count");
    }
    result.into_boxed()
}
