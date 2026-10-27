use super::fixtures;
use crate::{
    Kind,
    setup::{Mode, Setup},
    steps,
};
use alloc::boxed::Box;
fn names(setup: &Setup, expected: &[(&[u8], Kind)]) {
    let steps = steps(setup, &flags(), Box::new([]), false).always;
    assert_eq!(steps.len(), expected.len());
    for (step, (name, kind)) in steps.iter().zip(expected) {
        assert_eq!(step.name(), *name);
        assert_eq!(step.kind(), *kind);
    }
}
#[test]
fn a_managed_and_an_owned_forge_run_the_design_tree() {
    names(
        &fixtures::setup(),
        &[
            (b"prepare 2", Kind::Once),
            (b"forgejo 2", Kind::Long),
            (b"runner 2", Kind::Long),
            (b"reconcile 1", Kind::Once),
            (b"serve", Kind::Long),
        ],
    );
    let commands = steps(&fixtures::setup(), &flags(), Box::new([]), false).always;
    let Mode::Owned(owned) = &fixtures::setup().entries[1].mode else { panic!("owned") };
    assert_eq!(commands[1].ready(), Some(owned.listen));
    assert!(commands[1].listeners().is_empty());
    for (index, step) in commands.iter().enumerate() {
        if index != 1 {
            assert_eq!(step.ready(), None);
        }
    }
    let expected: &[&[&[u8]]] = &[
        &[b"forge", b"prepare", b"--config", b"/config", b"--data", b"/data", b"--cache", b"/cache", b"2"],
        &[b"forge", b"run", b"--config", b"/config", b"--data", b"/data", b"--cache", b"/cache", b"2", b"forgejo"],
        &[b"forge", b"run", b"--config", b"/config", b"--data", b"/data", b"--cache", b"/cache", b"2", b"runner"],
        &[b"forge", b"reconcile", b"--config", b"/config", b"--data", b"/data", b"--cache", b"/cache", b"1"],
        &[b"internal", b"serve", b"--config", b"/config", b"--data", b"/data", b"--cache", b"/cache", b"--box"],
    ];
    for (step, expected) in commands.iter().zip(expected) {
        assert_eq!(step.arguments().len(), expected.len());
        for (arg, expected) in step.arguments().iter().zip(*expected) {
            assert_eq!(arg.as_ref(), *expected);
        }
    }
}
#[test]
fn a_fake_forge_reconciles_in_serve_without_external_steps() {
    names(
        &fixtures::fake_setup(),
        &[
            (b"prepare 2", Kind::Once),
            (b"forgejo 2", Kind::Long),
            (b"runner 2", Kind::Long),
            (b"reconcile 1", Kind::Once),
            (b"serve", Kind::Long),
        ],
    );
    assert!(steps(&fixtures::fake_setup(), &flags(), Box::new([]), false).asked.is_empty());
}
#[test]
fn an_owned_forge_without_a_runner_has_no_runner_step() {
    let mut setup = fixtures::setup();
    match &mut setup.entries[1].mode {
        Mode::Owned(owned) => owned.runner = None,
        Mode::Managed { .. } | Mode::Fake(_) => panic!("owned"),
    }
    names(
        &setup,
        &[(b"prepare 2", Kind::Once), (b"forgejo 2", Kind::Long), (b"reconcile 1", Kind::Once), (b"serve", Kind::Long)],
    );
}
#[test]
fn a_managed_forge_alone_runs_reconcile_then_serve() {
    let setup = Setup { entries: Box::new([fixtures::setup().entries[0].clone()]) };
    names(&setup, &[(b"reconcile 1", Kind::Once), (b"serve", Kind::Long)]);
}
#[test]
fn an_empty_setup_runs_serve_alone() {
    names(&Setup { entries: Box::new([]) }, &[(b"serve", Kind::Long)]);
}

#[test]
fn serve_holds_only_fixed_document_ports_in_their_original_order() {
    let ephemeral = ([127, 0, 0, 1], 0).into();
    let clients = ([127, 0, 0, 1], 8367).into();
    let webhooks = ([127, 0, 0, 1], 8371).into();
    let setup = Setup { entries: Box::new([]) };
    for (document, held) in
        [(&[ephemeral][..], &[][..]), (&[ephemeral, clients, ephemeral, webhooks], &[clients, webhooks][..])]
    {
        let commands = steps(&setup, &flags(), Box::from(document), false);
        assert_eq!(commands.always[0].listeners(), held);
    }
}

fn flags() -> Box<[Box<[u8]>]> {
    Box::new([
        Box::from(b"--config".as_slice()),
        Box::from(b"/config".as_slice()),
        Box::from(b"--data".as_slice()),
        Box::from(b"/data".as_slice()),
        Box::from(b"--cache".as_slice()),
        Box::from(b"/cache".as_slice()),
    ])
}

#[test]
fn asked_groups_keep_their_place_and_run_only_their_processes() {
    for (runner, number, label) in [
        (false, 2, b"Forge 2 (Forgejo)".as_slice()),
        (true, 2, b"Forge 2 (Forgejo)".as_slice()),
        (true, 13, b"Forge 13 (Forgejo)".as_slice()),
        (true, u16::MAX, b"Forge 65535 (Forgejo)".as_slice()),
    ] {
        let mut setup = fixtures::setup();
        setup.entries[1].forge = number;
        setup.entries[1].asked = true;
        if !runner {
            let Mode::Owned(owned) = &mut setup.entries[1].mode else { panic!("owned") };
            owned.runner = None;
        }
        let commands = steps(&setup, &flags(), Box::new([]), false);
        assert_eq!(commands.always.len(), 2);
        for (step, name) in commands.always.iter().zip([b"reconcile 1".as_slice(), b"serve"]) {
            assert_eq!(step.name(), name);
        }
        assert_eq!(commands.asked.len(), 1);
        let group = &commands.asked[0];
        assert_eq!((group.forge, group.before), (number, 0));
        assert_eq!(group.label.as_ref(), label);
        assert_eq!(group.steps.len(), if runner { 3 } else { 2 });
        assert_eq!(group.steps[0].0, crate::Stage::Prepare);
        let Mode::Owned(owned) = &setup.entries[1].mode else { panic!("owned") };
        assert_eq!(group.steps[1].1.ready(), Some(owned.listen));
        assert!(group.steps[1].1.listeners().is_empty());
        for (stage, step) in &group.steps[1..] {
            assert_eq!(*stage, crate::Stage::Run);
            assert_eq!(step.kind(), Kind::Long);
        }
    }
    for (stage, word) in
        [(crate::Stage::Fetch, b"fetch".as_slice()), (crate::Stage::Prepare, b"prepare"), (crate::Stage::Run, b"run")]
    {
        assert_eq!(stage.word(), word);
    }
    let mut setup = fixtures::setup();
    let mut other = setup.entries[1].clone();
    other.forge = 3;
    other.asked = true;
    setup.entries = Box::new([setup.entries[1].clone(), other]);
    let commands = steps(&setup, &flags(), Box::new([]), false);
    assert_eq!(commands.asked[0].before, 3);
    assert_eq!(commands.asked[0].label.as_ref(), b"Forge 3 (Forgejo)");
}

#[test]
fn forgejo_takes_its_listener_only_when_the_shell_can_identify_signal_senders() {
    let mut setup = fixtures::setup();
    let Mode::Owned(owned) = &setup.entries[1].mode else { panic!("owned") };
    let address = owned.listen;
    for asked in [false, true] {
        setup.entries[1].asked = asked;
        for listens in [false, true] {
            let commands = steps(&setup, &flags(), Box::new([]), listens);
            let step = if asked { &commands.asked[0].steps[1].1 } else { &commands.always[1] };
            assert_eq!(step.ready(), Some(address));
            let held = [address];
            assert_eq!(step.listeners(), if listens { &held[..] } else { &[] });
        }
    }
}
