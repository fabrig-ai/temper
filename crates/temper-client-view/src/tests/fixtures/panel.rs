//! Whole change panels at their receiving bounds.
use super::limits;
use skein_lib::List;
use temper_client_domain::forge;
use temper_client_protocol_wire::{self as wire, v1 as w};
/// Every heap field at its receiving bound, with optional facts populated.
pub fn panel(task: u64, step: w::Step) -> w::ChangePanel {
    let limits = forge::wire_limits(&limits().forge).expect("wire limits");
    let mut gates = List::with_capacity(wire::GATES);
    for number in 1..=wire::GATES {
        gates
            .push(
                w::Gate::new(
                    &limits,
                    w::GateParts {
                        number: u64::from(number),
                        kind: w::GateKind::Review,
                        blocks: true,
                        verdict: w::Verdict::Approved,
                    },
                )
                .expect("gate"),
            )
            .expect("bounded gates");
    }
    w::ChangePanel::new(
        &limits,
        w::ChangePanelParts {
            way: w::Way::Queue,
            in_landing: None,
            task,
            forge: 13,
            owner: field(b'o', limits.change_panel_owner),
            name: field(b'n', limits.change_panel_name),
            branch: field(b'b', limits.change_panel_branch),
            landing: field(b'l', limits.change_panel_landing),
            head: Some([0x39; 32]),
            pull: Some(
                w::Pull::new(&limits, w::PullParts { number: 23, address: field(b'a', limits.pull_address) })
                    .expect("pull"),
            ),
            step,
            hold: Some(w::Hold::Moved),
            runs: Some(29),
            ci: w::Ci::Passed,
            gates,
            repairs: w::Used::new(&limits, w::UsedParts { used: 1, limit: 2 }).expect("repairs"),
            resolutions: w::Used::new(&limits, w::UsedParts { used: 3, limit: 4 }).expect("resolutions"),
            updates: w::Used::new(&limits, w::UsedParts { used: 5, limit: 6 }).expect("updates"),
        },
    )
    .expect("panel")
}
pub fn queued() -> w::Step {
    let limits = forge::wire_limits(&limits().forge).expect("wire limits");
    let mut ahead = List::with_capacity(wire::TASKS_AHEAD);
    for task in 1..=wire::TASKS_AHEAD {
        ahead.push(u64::from(task)).expect("tasks ahead");
    }
    w::Step::Queued(w::Queued::new(&limits, w::QueuedParts { place: 65, ahead }).expect("queued"))
}

fn field(byte: u8, count: u32) -> Box<[u8]> {
    let mut bytes = skein_lib::bytes::zeroed(usize::try_from(count).unwrap());
    bytes.fill(byte);
    bytes
}
