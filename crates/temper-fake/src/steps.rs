//! The demo and fake processes around forge-setup's commands (protocol/setup.md, 7.3).
use crate::demo::Demo;
use alloc::boxed::Box;
use skein_lib::{Decimal, List, Writer, bytes};
use temper_forge_setup::{
    Group, Kind, Stage, Step, Steps,
    setup::{Mode, Setup},
};

/// The demo's group, the fake LLM, then the demo forge's preparation
/// and its fake forge (`application.demo.forge`, 8.4), is always asked,
/// and writes no record: the demo's state is the engine's alone (8.4),
/// so a box started again runs it only when `temper serve` asks, which
/// serve does at the person's choice and at each of its starts while the
/// demo runs. Its `started` follows its long steps' start, as the steps'
/// order takes them (7.2). A fake forge other than the demo's runs in its
/// place, unasked.
#[must_use]
#[expect(clippy::manual_map, reason = "step code uses explicit matches, never closures")]
pub fn steps(
    setup: &Setup,
    demo: Option<&Demo>,
    flags: &[Box<[u8]>],
    listeners: Box<[core::net::SocketAddr]>,
) -> Steps {
    let given = temper_forge_setup::steps(setup, flags, listeners);
    let demo_forge = match demo {
        Some(demo) => Some(demo.forge()),
        None => None,
    };
    let mut prefix = 0_u32;
    for entry in &setup.entries {
        if let Mode::Fake(_) = &entry.mode
            && demo_forge != Some(entry.forge)
        {
            prefix = prefix.checked_add(2).expect("bounded fake steps");
        }
    }
    let capacity =
        u32::try_from(given.always.len()).expect("bounded steps").checked_add(prefix).expect("bounded prefix");
    let mut always = List::with_capacity(capacity);
    for entry in &setup.entries {
        if let Mode::Fake(fake) = &entry.mode
            && demo_forge != Some(entry.forge)
        {
            always
                .push(command(b"prepare", &[b"forge", b"prepare"], flags, Some(entry.forge), Kind::Once))
                .expect("fake preparation");
            always
                .push(
                    command(b"fake forge", &[b"fake", b"forge"], flags, Some(entry.forge), Kind::Long)
                        .listening(Box::new([fake.listen])),
                )
                .expect("fake forge");
        }
    }
    for step in given.always {
        always.push(step).expect("forge-setup steps counted");
    }
    let mut asked = List::with_capacity(
        u32::try_from(given.asked.len())
            .expect("bounded groups")
            .checked_add(u32::from(demo.is_some()))
            .expect("demo group"),
    );
    if let Some(demo) = demo {
        let Mode::Fake(fake) = &setup.entry(demo.forge()).expect("checked demo forge").mode else {
            unreachable!("demo has a fake forge")
        };
        asked
            .push(Group {
                forge: demo.forge(),
                recorded: false,
                before: 0,
                steps: Box::new([
                    (
                        Stage::Run,
                        command(b"fake llm", &[b"fake", b"llm"], flags, None, Kind::Long)
                            .listening(Box::new([demo.llm().listen()])),
                    ),
                    (
                        Stage::Prepare,
                        command(b"prepare", &[b"forge", b"prepare"], flags, Some(demo.forge()), Kind::Once),
                    ),
                    (
                        Stage::Run,
                        command(b"fake forge", &[b"fake", b"forge"], flags, Some(demo.forge()), Kind::Long)
                            .listening(Box::new([fake.listen])),
                    ),
                ]),
            })
            .expect("demo group counted");
    }
    for mut group in given.asked {
        group.before = group.before.checked_add(prefix).expect("bounded group position");
        asked.push(group).expect("owned groups counted");
    }
    Steps { always: always.into_boxed(), asked: asked.into_boxed() }
}

fn command(name: &[u8], words: &[&[u8]], flags: &[Box<[u8]>], forge: Option<u16>, kind: Kind) -> Step {
    let capacity = u32::try_from(words.len())
        .expect("command words")
        .checked_add(u32::try_from(flags.len()).expect("bounded flags"))
        .expect("bounded arguments")
        .checked_add(u32::from(forge.is_some()))
        .expect("forge number");
    let mut arguments = List::with_capacity(capacity);
    for word in words {
        arguments.push(bytes::copy_of(word)).expect("command words counted");
    }
    for flag in flags {
        arguments.push(flag.clone()).expect("flags counted");
    }
    let name = match forge {
        Some(number) => {
            let digits = Decimal::of(u64::from(number));
            let number = digits.as_bytes();
            arguments.push(bytes::copy_of(number)).expect("forge number counted");
            let length =
                name.len().checked_add(1).expect("name and space").checked_add(number.len()).expect("bounded name");
            let mut writer = Writer::new(length);
            writer.put(name).expect("name counted");
            writer.put(b" ").expect("space counted");
            writer.put(number).expect("number counted");
            writer.finish()
        }
        None => bytes::copy_of(name),
    };
    Step::new(name, arguments.into_boxed(), kind)
}
