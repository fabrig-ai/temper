//! Landing and receipt records (protocol/store.md, families 23 and 24).
use super::{FITS, names, w};
use alloc::boxed::Box;
use skein_lib::{List, Wall};
use temper_engine_domain_forge as top;
use temper_engine_domain_forge_change as change;
use temper_engine_domain_forge_landing as d;

fn encode_place(phase: &d::Phase) -> w::LandingPlace {
    match phase {
        d::Phase::Waiting => w::LandingPlace::Waiting,
        d::Phase::Preparing => w::LandingPlace::Preparing,
        d::Phase::Proving => w::LandingPlace::Proving,
        d::Phase::Proved { at } => w::LandingPlace::Proved(
            w::LandingAt::new(&w::CEILINGS, w::LandingAtParts { at: at.as_nanos() }).expect(FITS),
        ),
        d::Phase::Turn { at } => {
            w::LandingPlace::Turn(w::LandingAt::new(&w::CEILINGS, w::LandingAtParts { at: at.as_nanos() }).expect(FITS))
        }
        d::Phase::Landed => w::LandingPlace::Landed,
        d::Phase::Cancelled => w::LandingPlace::Cancelled,
        d::Phase::Held { .. } => unreachable!("one held wrapper"),
    }
}
fn decode_place(phase: w::LandingPlace) -> d::Phase {
    match phase {
        w::LandingPlace::Waiting => d::Phase::Waiting,
        w::LandingPlace::Preparing => d::Phase::Preparing,
        w::LandingPlace::Proving => d::Phase::Proving,
        w::LandingPlace::Proved(at) => d::Phase::Proved { at: Wall::from_nanos(at.at()) },
        w::LandingPlace::Turn(at) => d::Phase::Turn { at: Wall::from_nanos(at.at()) },
        w::LandingPlace::Landed => d::Phase::Landed,
        w::LandingPlace::Cancelled => d::Phase::Cancelled,
    }
}
fn encode_phase(phase: &d::Phase) -> w::LandingPhase {
    let (place, held) = match phase {
        d::Phase::Held { was, why: d::Hold::Faults { member, gate } } => (
            encode_place(was),
            Some(
                w::LandingFaults::new(&w::CEILINGS, w::LandingFaultsParts { member: *member, gate: *gate })
                    .expect(FITS),
            ),
        ),
        d::Phase::Waiting
        | d::Phase::Preparing
        | d::Phase::Proving
        | d::Phase::Proved { .. }
        | d::Phase::Turn { .. }
        | d::Phase::Landed
        | d::Phase::Cancelled => (encode_place(phase), None),
    };
    w::LandingPhase::new(&w::CEILINGS, w::LandingPhaseParts { place, held }).expect(FITS)
}
fn decode_phase(phase: w::LandingPhase) -> d::Phase {
    let w::LandingPhaseParts { place, held } = phase.into_parts();
    let phase = decode_place(place);
    match held {
        Some(held) => {
            let w::LandingFaultsParts { member, gate } = held.into_parts();
            d::Phase::Held { was: Box::new(phase), why: d::Hold::Faults { member, gate } }
        }
        None => phase,
    }
}
fn encode_proof(proof: &d::GateProof) -> w::GateProof {
    let state = match proof.proof {
        d::Proof::Unkeyed => w::GateProofState::Unkeyed,
        d::Proof::Dispatched { key, try_ } => w::GateProofState::Dispatched(
            w::DispatchedKey::new(&w::CEILINGS, w::DispatchedKeyParts { key, try_ }).expect(FITS),
        ),
        d::Proof::Keyed { key } => {
            w::GateProofState::Keyed(w::GateKey::new(&w::CEILINGS, w::GateKeyParts { key }).expect(FITS))
        }
        d::Proof::Passed { key } => {
            w::GateProofState::Passed(w::GateKey::new(&w::CEILINGS, w::GateKeyParts { key }).expect(FITS))
        }
        d::Proof::Failed { key } => {
            w::GateProofState::Failed(w::GateKey::new(&w::CEILINGS, w::GateKeyParts { key }).expect(FITS))
        }
    };
    w::GateProof::new(
        &w::CEILINGS,
        w::GateProofParts {
            proof: state,
            posted: match proof.posted {
                d::Posted::Nothing => w::GatePosted::Nothing,
                d::Posted::Pending => w::GatePosted::Pending,
                d::Posted::Verdict => w::GatePosted::Verdict,
            },
            faults: proof.faults,
        },
    )
    .expect(FITS)
}
fn decode_proof(proof: w::GateProof) -> d::GateProof {
    let w::GateProofParts { proof, posted, faults } = proof.into_parts();
    let proof = match proof {
        w::GateProofState::Unkeyed => d::Proof::Unkeyed,
        w::GateProofState::Dispatched(value) => {
            let w::DispatchedKeyParts { key, try_ } = value.into_parts();
            d::Proof::Dispatched { key, try_ }
        }
        w::GateProofState::Keyed(value) => d::Proof::Keyed { key: *value.key() },
        w::GateProofState::Passed(value) => d::Proof::Passed { key: *value.key() },
        w::GateProofState::Failed(value) => d::Proof::Failed { key: *value.key() },
    };
    d::GateProof {
        proof,
        posted: match posted {
            w::GatePosted::Nothing => d::Posted::Nothing,
            w::GatePosted::Pending => d::Posted::Pending,
            w::GatePosted::Verdict => d::Posted::Verdict,
        },
        faults,
    }
}
fn encode_member(member: &d::Member) -> w::LandingMember {
    let mut gates = List::with_capacity(u32::try_from(member.gates.len()).expect(FITS));
    for gate in &member.gates {
        gates.push(encode_proof(gate)).expect(FITS);
    }
    w::LandingMember::new(
        &w::CEILINGS,
        w::LandingMemberParts {
            change: member.change,
            repository: names::encode_repository(skein_forge::api::Repository {
                forge: member.repository.forge,
                repository: member.repository.repository,
            }),
            pull: member.pull,
            head: member.head,
            base: member.base,
            candidate: member.candidate,
            gates,
            asked: member.asked,
            landed: member.landed,
        },
    )
    .expect(FITS)
}
fn decode_member(member: w::LandingMember) -> d::Member {
    let w::LandingMemberParts { change, repository, pull, head, base, candidate, gates, asked, landed } =
        member.into_parts();
    let repository = names::decode_repository(repository);
    let mut proofs = List::with_capacity(gates.len());
    for gate in gates.into_boxed() {
        proofs.push(decode_proof(gate)).expect(FITS);
    }
    d::Member {
        change,
        repository: d::Repository { forge: repository.forge, repository: repository.repository },
        pull,
        head,
        base,
        candidate,
        gates: proofs.into_boxed(),
        asked,
        landed,
    }
}
fn encode_operation(op: top::Operation) -> w::LandingOperation {
    match op {
        top::Operation::Dispatch => w::LandingOperation::Dispatch,
        top::Operation::Rerun => w::LandingOperation::Rerun,
        top::Operation::Pending => w::LandingOperation::Pending,
        top::Operation::Verdict => w::LandingOperation::Verdict,
        top::Operation::Merge => w::LandingOperation::Merge,
    }
}
fn decode_operation(op: w::LandingOperation) -> top::Operation {
    match op {
        w::LandingOperation::Dispatch => top::Operation::Dispatch,
        w::LandingOperation::Rerun => top::Operation::Rerun,
        w::LandingOperation::Pending => top::Operation::Pending,
        w::LandingOperation::Verdict => top::Operation::Verdict,
        w::LandingOperation::Merge => top::Operation::Merge,
    }
}
fn encode_ask(ask: change::Ask) -> w::LandingAsk {
    match ask {
        change::Ask::Prepare { head, main } => w::LandingAsk::Prepare(
            w::LandingPrepare::new(&w::CEILINGS, w::LandingPrepareParts { head, main }).expect(FITS),
        ),
        change::Ask::Repair { gate, candidate } => w::LandingAsk::Repair(
            w::LandingRepairAsk::new(&w::CEILINGS, w::LandingRepairAskParts { gate, candidate }).expect(FITS),
        ),
    }
}
fn decode_ask(ask: w::LandingAsk) -> change::Ask {
    match ask {
        w::LandingAsk::Prepare(value) => {
            let w::LandingPrepareParts { head, main } = value.into_parts();
            change::Ask::Prepare { head, main }
        }
        w::LandingAsk::Repair(value) => {
            let w::LandingRepairAskParts { gate, candidate } = value.into_parts();
            change::Ask::Repair { gate, candidate }
        }
    }
}
#[expect(clippy::manual_map, reason = "the foundation uses explicit option matches without closures")]
pub(super) fn encode(row: &top::LandingRow) -> w::LandingRow {
    let mut members = List::with_capacity(u32::try_from(row.landing.members.len()).expect(FITS));
    for member in &row.landing.members {
        members.push(encode_member(member)).expect(FITS);
    }
    let landing = w::LandingState::new(
        &w::CEILINGS,
        w::LandingStateParts {
            task: row.landing.task,
            project: row.landing.project,
            phase: encode_phase(&row.landing.phase),
            members,
            preparation: row.landing.preparation,
            stale_proofs: row.landing.stale_proofs,
        },
    )
    .expect(FITS);
    let mut asks = List::with_capacity(u32::try_from(row.asks.len()).expect(FITS));
    for ask in &row.asks {
        asks.push(
            w::LandingAskSlot::new(
                &w::CEILINGS,
                w::LandingAskSlotParts {
                    ask: match ask {
                        Some(ask) => Some(encode_ask(*ask)),
                        None => None,
                    },
                },
            )
            .expect(FITS),
        )
        .expect(FITS);
    }
    let mut entries = List::with_capacity(u32::try_from(row.entries.len()).expect(FITS));
    for entry in &row.entries {
        entries
            .push(
                w::LandingEntry::new(
                    &w::CEILINGS,
                    w::LandingEntryParts {
                        entry: entry.entry,
                        member: entry.member,
                        gate: entry.gate,
                        operation: encode_operation(entry.operation),
                    },
                )
                .expect(FITS),
            )
            .expect(FITS);
    }
    w::LandingRow::new(&w::CEILINGS, w::LandingRowParts { landing, asks, entries }).expect(FITS)
}
#[expect(clippy::manual_map, reason = "the foundation uses explicit option matches without closures")]
pub(super) fn decode(row: w::LandingRow) -> top::LandingRow {
    let w::LandingRowParts { landing, asks, entries } = row.into_parts();
    let w::LandingStateParts { task, project, phase, members, preparation, stale_proofs } = landing.into_parts();
    let mut decoded_members = List::with_capacity(members.len());
    for member in members.into_boxed() {
        decoded_members.push(decode_member(member)).expect(FITS);
    }
    let mut decoded_asks = List::with_capacity(asks.len());
    for ask in asks.into_boxed() {
        decoded_asks
            .push(match ask.into_parts().ask {
                Some(ask) => Some(decode_ask(ask)),
                None => None,
            })
            .expect(FITS);
    }
    let mut decoded_entries = List::with_capacity(entries.len());
    for entry in entries.into_boxed() {
        let w::LandingEntryParts { entry, member, gate, operation } = entry.into_parts();
        decoded_entries
            .push(top::LandingEntry { entry, member, gate, operation: decode_operation(operation) })
            .expect(FITS);
    }
    top::LandingRow {
        landing: d::Landing {
            task,
            project,
            phase: decode_phase(phase),
            members: decoded_members.into_boxed(),
            preparation,
            stale_proofs,
        },
        asks: decoded_asks.into_boxed(),
        entries: decoded_entries.into_boxed(),
    }
}
#[expect(clippy::manual_map, reason = "the foundation uses explicit options without callbacks")]
pub(super) fn encode_receipt(key: &top::ReceiptKey, receipt: &top::Receipt) -> w::ReceiptRow {
    let (name, tier) = match &key.check {
        top::Check::Gate(name) => (name.clone(), false),
        top::Check::Tier(name) => (name.clone(), true),
    };
    let name = w::ReceiptName::new(&w::CEILINGS, w::ReceiptNameParts { name }).expect(FITS);
    let key = w::ReceiptKey::new(
        &w::CEILINGS,
        w::ReceiptKeyParts {
            project: key.project,
            repository: names::encode_repository(key.repository),
            check_name: if tier { w::ReceiptCheck::Tier(name) } else { w::ReceiptCheck::Gate(name) },
            key: key.key,
        },
    )
    .expect(FITS);
    let run = w::ReceiptRun::new(
        &w::CEILINGS,
        w::ReceiptRunParts { id: receipt.run.id, number: receipt.run.number, url: receipt.run.url.clone() },
    )
    .expect(FITS);
    let receipt = w::Receipt::new(
        &w::CEILINGS,
        w::ReceiptParts {
            failures: match &receipt.failures {
                Some(f) => Some(super::verification::encode_failures(f)),
                None => None,
            },
            passed: receipt.passed,
            run,
            candidate: receipt.candidate,
            ended: receipt.ended.as_nanos(),
        },
    )
    .expect(FITS);
    w::ReceiptRow::new(&w::CEILINGS, w::ReceiptRowParts { key, receipt }).expect(FITS)
}
pub(super) fn decode_receipt(row: w::ReceiptRow) -> Result<top::Stored, super::Undecodable> {
    let w::ReceiptRowParts { key, receipt } = row.into_parts();
    let w::ReceiptKeyParts { project, repository, check_name, key } = key.into_parts();
    let check = match check_name {
        w::ReceiptCheck::Gate(name) => top::Check::Gate(name.into_parts().name),
        w::ReceiptCheck::Tier(name) => top::Check::Tier(name.into_parts().name),
    };
    let w::ReceiptParts { failures, passed, run, candidate, ended } = receipt.into_parts();
    let w::ReceiptRunParts { id, number, url } = run.into_parts();
    Ok(top::Stored::Receipt {
        key: top::ReceiptKey { project, repository: names::decode_repository(repository), check, key },
        receipt: top::Receipt {
            passed,
            failures: match failures {
                Some(f) => Some(super::verification::decode_failures(f)?),
                None => None,
            },
            run: top::ReceiptRun { id, number, url },
            candidate,
            ended: Wall::from_nanos(ended),
        },
    })
}
