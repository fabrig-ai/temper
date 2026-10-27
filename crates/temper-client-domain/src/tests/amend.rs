use super::fixtures;
use crate::amend::{Cut, Draft, Drafted};
use fixtures::acts::text;
use jig_client::contract as c;
use skein_lib::{List, Wall};

const REVISION: c::Revision = c::Revision { value: 7 };
const LATER_REVISION: c::Revision = c::Revision { value: 8 };
const PRIORITY: u32 = 2;
const NEXT_PRIORITY: u32 = 3;
const BUILD: u64 = 11;
const REVIEW: u64 = 13;
const INPUT: u64 = 5;
const DEADLINE: Wall = Wall::from_nanos(100);
const LATER_DEADLINE: Wall = Wall::from_nanos(200);
// Each fixture list has one row, except the two dependencies.
const ONE: u32 = 1;
const DEPENDENCIES: u32 = 2;
const EMPTY: u32 = 0;

fn pattern() -> c::OwnerPattern {
    let mut segments = List::with_capacity(ONE);
    segments.push(Box::from(b"project".as_slice())).unwrap();
    c::OwnerPattern { segments, last: c::OwnerPatternEnd::Open(Box::from(b"notes".as_slice())) }
}
fn authority(deadline: Option<Wall>) -> c::OwnerAuthority {
    let mut models = List::with_capacity(ONE);
    models.push(Box::from(b"first".as_slice())).unwrap();
    let mut grants = List::with_capacity(ONE);
    grants.push(c::OwnerGrant { connector: fixtures::FORGE, kind: 1, pattern: pattern() }).unwrap();
    let mut kinds = List::with_capacity(ONE);
    kinds.push(c::OwnerAuthorityExecutor::Charter(1)).unwrap();
    let mut note_resources = List::with_capacity(ONE);
    note_resources.push(c::OwnerResourceScope { connector: fixtures::FORGE, pattern: pattern() }).unwrap();
    c::OwnerAuthority {
        models: Some(c::AuthorityModels::Named(models)),
        tools: 3,
        grants,
        delegation: c::OwnerDelegation { kinds, tasks: 2, depth: 3 },
        spend: 50,
        deadline,
        notes: 1,
        note_resources,
    }
}
fn spec() -> c::Specification {
    let mut parameters = List::with_capacity(ONE);
    parameters.push(c::Parameter::Bytes { name: 1, value: text(b"debug") }).unwrap();
    let mut inputs = List::with_capacity(ONE);
    inputs.push(INPUT).unwrap();
    c::Specification { words: text(b"Build the project"), parameters, inputs }
}
fn dependencies() -> List<u64> {
    let mut numbers = List::with_capacity(DEPENDENCIES);
    for number in [BUILD, REVIEW] {
        numbers.push(number).unwrap();
    }
    numbers
}
fn choice(model: &[u8]) -> c::LlmChoice {
    let mut subagents = List::with_capacity(ONE);
    subagents.push(c::ModelEffort { model: text(b"helper"), effort: Some(text(b"low")) }).unwrap();
    c::LlmChoice {
        account: Some(7),
        main: c::ModelEffort { model: text(model), effort: Some(text(b"high")) },
        subagents,
    }
}
fn next_choice() -> c::LlmChoice {
    let mut choice = choice(b"second");
    choice.account = Some(8);
    choice.main.effort = None;
    choice.subagents.clear();
    choice.subagents.push(c::ModelEffort { model: text(b"worker"), effort: Some(text(b"medium")) }).unwrap();
    choice
}
fn goal() -> c::Task {
    let mut task = fixtures::chat::task();
    task.priority = Some(PRIORITY);
    task.spec_revision = REVISION;
    task.authority = c::OwnerAuthorityDocument::complete(authority(Some(DEADLINE)));
    task.spec = c::SpecificationDocument::complete(spec());
    task.instructions = Some(c::DocumentText::complete(text(b"Run the checks")));
    let mut build = fixtures::acts::chip();
    build.task = BUILD;
    task.dependencies = List::with_capacity(DEPENDENCIES);
    task.dependencies.push(c::OwnerTaskLink::Live(build)).unwrap();
    task.dependencies.push(c::OwnerTaskLink::Stored { number: REVIEW, status: None }).unwrap();
    task.llm = Some(choice(b"first"));
    task
}
fn cut_goal() -> c::Task {
    let mut task = goal();
    task.authority.cut = true;
    task.spec.cut = true;
    task
}
fn no_change() -> c::Amendment {
    c::Amendment {
        addressed: None,
        spec: None,
        wake: None,
        dependencies: None,
        authority: None,
        instructions: None,
        tracked: None,
        llm: None,
    }
}
fn priority_change() -> c::Amendment {
    c::Amendment { tracked: Some(c::Tracking::Tracked { priority: NEXT_PRIORITY }), ..no_change() }
}
fn reprioritized(task: &c::Task) -> Draft {
    let mut draft = Draft::of(task);
    draft.track(c::Tracking::Tracked { priority: NEXT_PRIORITY });
    draft
}
fn edited(task: &c::Task) -> Draft {
    let mut draft = reprioritized(task);
    draft.deadline(Some(LATER_DEADLINE)).unwrap();
    draft.words(text(b"Build and test")).unwrap();
    draft.instructions(text(b""));
    draft.dependencies(List::with_capacity(EMPTY));
    draft.llm(next_choice());
    draft
}
fn restore_controls(draft: &mut Draft) {
    draft.track(c::Tracking::Tracked { priority: PRIORITY });
    draft.deadline(Some(DEADLINE)).unwrap();
    draft.words(text(b"Build the project")).unwrap();
    draft.instructions(text(b"Run the checks"));
    draft.dependencies(dependencies());
    draft.llm(choice(b"first"));
}

#[test]
fn a_priority_alone_names_no_revision() {
    let draft = reprioritized(&goal());
    assert_eq!(draft.amendment(), Some(Drafted { revision: None, change: priority_change() }));
}
#[test]
fn tracking_an_untracked_task_names_the_revision() {
    let mut task = goal();
    task.priority = None;
    let draft = reprioritized(&task);
    assert_eq!(draft.amendment(), Some(Drafted { revision: Some(REVISION), change: priority_change() }));
}
#[test]
fn untracking_names_the_revision() {
    let mut draft = Draft::of(&goal());
    draft.track(c::Tracking::Untracked);
    let change = c::Amendment { tracked: Some(c::Tracking::Untracked), ..no_change() };
    assert_eq!(draft.amendment(), Some(Drafted { revision: Some(REVISION), change }));
}
#[test]
fn a_deadline_restates_the_rest_of_the_authority() {
    let mut draft = Draft::of(&goal());
    draft.deadline(Some(LATER_DEADLINE)).unwrap();
    let change = c::Amendment { authority: Some(authority(Some(LATER_DEADLINE))), ..no_change() };
    assert_eq!(draft.amendment(), Some(Drafted { revision: Some(REVISION), change }));
}
#[test]
fn a_deadline_on_a_cut_authority_is_refused() {
    let mut draft = Draft::of(&cut_goal());
    assert_eq!(draft.deadline(Some(LATER_DEADLINE)), Err(Cut::Authority));
    assert_eq!(draft.amendment(), None);
}
#[test]
fn words_keep_the_specs_parameters_and_inputs() {
    let mut draft = Draft::of(&goal());
    draft.words(text(b"Build and test")).unwrap();
    let mut expected = spec();
    expected.words = text(b"Build and test");
    let change = c::Amendment { spec: Some(expected), ..no_change() };
    assert_eq!(draft.amendment(), Some(Drafted { revision: Some(REVISION), change }));
}
#[test]
fn words_on_a_cut_spec_are_refused() {
    let mut draft = Draft::of(&cut_goal());
    assert_eq!(draft.words(text(b"Build and test")), Err(Cut::Spec));
    assert_eq!(draft.amendment(), None);
}
#[test]
fn empty_instructions_on_a_task_without_any_are_no_change() {
    let mut task = goal();
    task.instructions = None;
    let mut draft = Draft::of(&task);
    draft.instructions(text(b""));
    assert_eq!(draft.amendment(), None);
}
#[test]
fn empty_instructions_are_sent_explicitly() {
    let mut draft = Draft::of(&goal());
    draft.instructions(text(b""));
    let change = c::Amendment { instructions: Some(text(b"")), ..no_change() };
    assert_eq!(draft.amendment(), Some(Drafted { revision: Some(REVISION), change }));
}
#[test]
fn an_empty_dependency_list_is_sent_explicitly() {
    let mut draft = Draft::of(&goal());
    draft.dependencies(List::with_capacity(EMPTY));
    let change = c::Amendment { dependencies: Some(List::with_capacity(EMPTY)), ..no_change() };
    assert_eq!(draft.amendment(), Some(Drafted { revision: Some(REVISION), change }));
}
#[test]
fn an_edit_equal_to_the_base_is_no_change() {
    let mut draft = Draft::of(&goal());
    restore_controls(&mut draft);
    assert_eq!(draft.amendment(), None);
}
#[test]
fn an_edit_back_to_the_base_undoes_it() {
    let mut draft = edited(&goal());
    restore_controls(&mut draft);
    assert_eq!(draft.amendment(), None);
}
#[test]
fn a_priority_with_a_deadline_names_the_revision() {
    let mut draft = reprioritized(&goal());
    draft.deadline(Some(LATER_DEADLINE)).unwrap();
    let change = c::Amendment { authority: Some(authority(Some(LATER_DEADLINE))), ..priority_change() };
    assert_eq!(draft.amendment(), Some(Drafted { revision: Some(REVISION), change }));
}
#[test]
fn a_later_document_with_another_revision_has_moved() {
    let mut task = goal();
    let draft = Draft::of(&task);
    task.spec_revision = LATER_REVISION;
    assert!(draft.moved(&task));
    assert_eq!(draft.revision(), REVISION);
}
#[test]
fn the_llm_choice_is_sent_whole() {
    let mut draft = Draft::of(&goal());
    draft.llm(next_choice());
    let change = c::Amendment { llm: Some(next_choice()), ..no_change() };
    assert_eq!(draft.amendment(), Some(Drafted { revision: Some(REVISION), change }));
}
#[test]
fn a_new_draft_has_no_change() {
    assert_eq!(Draft::of(&goal()).amendment(), None);
}
#[test]
fn the_document_as_read_has_not_moved() {
    let task = goal();
    assert!(!Draft::of(&task).moved(&task));
}
#[test]
fn a_refused_edit_keeps_other_edits() {
    let mut draft = reprioritized(&cut_goal());
    assert_eq!(draft.deadline(None), Err(Cut::Authority));
    assert_eq!(draft.words(text(b"Build and test")), Err(Cut::Spec));
    assert_eq!(draft.amendment(), Some(Drafted { revision: None, change: priority_change() }));
}
#[test]
fn a_deadline_can_be_cleared() {
    let mut draft = Draft::of(&goal());
    draft.deadline(None).unwrap();
    let change = c::Amendment { authority: Some(authority(None)), ..no_change() };
    assert_eq!(draft.amendment(), Some(Drafted { revision: Some(REVISION), change }));
}
#[test]
fn undoing_the_deadline_leaves_a_priority_alone() {
    let mut draft = reprioritized(&goal());
    draft.deadline(Some(LATER_DEADLINE)).unwrap();
    draft.deadline(Some(DEADLINE)).unwrap();
    assert_eq!(draft.amendment(), Some(Drafted { revision: None, change: priority_change() }));
}

#[test]
fn words_replace_a_cut_read_while_preserving_parameters_inputs_and_revision() {
    let mut task = goal();
    task.spec.specification.words.cut = true;
    let mut draft = Draft::of(&task);
    draft.words(text(b"Replacement")).unwrap();
    let amended = draft.amendment().unwrap();
    assert_eq!(amended.revision, Some(REVISION));
    let mut expected = spec();
    expected.words = text(b"Replacement");
    assert_eq!(amended.change.spec, Some(expected));
}
