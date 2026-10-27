//! Documents for the hand-recorded card examples; the production cards draw them.
use jig_client::contract as c;
use skein_lib::List;
use skein_ui::Builder;
const BUDGET: u64 = 40;
const LEFT: u64 = 120;
const HOLD_REVISION: u64 = 7;
const WINDOW: u32 = 2;
const TRIES: u32 = 2;
pub fn proposal(builder: &mut Builder) {
    let mut proposal = jig_client_world::stories::proposal();
    proposal.why = jig_client_world::stories::text(b"It touches sign-in.");
    proposal.action = c::ProposalAction::Batch { tasks: List::with_capacity(0), own: true };
    proposal.needs.budget = BUDGET;
    proposal.funding.left = LEFT;
    let page = c::ReadValue::Proposal(Box::new(proposal));
    let words = super::page_words();
    temper_client_view::cards::proposal_at(builder, &page, &words, WINDOW, temper_client_view::cards::Place::Inbox);
}
pub fn held(builder: &mut Builder) {
    let mut task = jig_client_world::stories::task();
    task.hold = Some(c::TaskHold::Owner(Box::new(c::OwnerChipHold {
        placed: None,
        circle: None,
        account: None,
        source: None,
        reason: c::HoldCategory::Failures { class: c::HoldFailure::Run },
        revision: HOLD_REVISION,
        before: c::OwnerPriorPhase::Active(c::RunStage::Idle),
        escalation: None,
        tries: c::OwnerFailureTries { transient: 0, permanent: 0, run: TRIES, agent: 0, lost: 0, invalid: 0 },
        hold_wait_since: None,
    })));
    task.acts = List::with_capacity(1);
    task.acts
        .push(c::Offered {
            act: c::ActKind::Release,
            admission: c::Admission::Allowed,
            effect: Some(c::Effect::Release(c::ReleaseEffect::RunAgain)),
        })
        .expect("release offer fits");
    let page = c::ReadValue::Task(Box::new(task));
    let words = super::page_words();
    temper_client_view::cards::proposal(builder, &page, &words, WINDOW);
    temper_client_view::cards::decision(builder, &words, Some(&page), c::ActKind::Release);
}
pub fn report(builder: &mut Builder, bytes: &[u8]) {
    let report = c::ResultValue::Report(c::DocumentText::complete(jig_client_world::stories::text(bytes)));
    temper_client_view::cards::value(builder, &report, &super::page_words());
}
