//! The root cold-loads the exact records written while finding and repairing a red.
use super::*;
use temper_engine_domain_forge_tiers::{self as policy, Blame, Part, Suspects};
use temper_engine_domain_world::forge_routes::produce_path_content;

const TIER_TICKS: u32 = 1800; // the existing root tier story's 90 s workflow and poll allowance.
const RED_TICKS: u32 = 3 * TIER_TICKS; // whole, rerun, and one oldest-landing probe.
const FAILED: &[u8] = b"suite.first\nsuite.second\n";
const WITH_FLAKE: &[u8] = b"suite.first\nsuite.flaky\nsuite.second\n";
#[derive(Clone, Copy, Debug)]
enum Cut {
    Report,
    Rerun,
    ProbeBranch,
    Probe,
    ProbeRelease,
    RunRelease,
    Red,
    Repaired,
}
fn tier(world: &World) -> Option<&forge_top::TierRow> {
    world.store.rows.values().find_map(|record| match record {
        Record::Forge { row } => match row.as_ref() {
            forge_top::Stored::Tier(row) => Some(row),
            _ => None,
        },
        Record::Core(_) => None,
    })
}
fn verifications(world: &World) -> impl Iterator<Item = &forge_top::Verification> {
    world.store.rows.values().filter_map(|record| match record {
        Record::Forge { row } => match row.as_ref() {
            forge_top::Stored::Verification(row) => Some(row),
            _ => None,
        },
        Record::Core(_) => None,
    })
}
fn release_entry(world: &World, number: u64) -> bool {
    matches!(world.store.rows.get(&Key::Forge(forge_top::Key::Entry(number))),
        Some(Record::Forge { row }) if matches!(row.as_ref(), forge_top::Stored::Entry { entry, .. }
            if matches!(entry.effect.write, skein_forge::api::Write::DeleteBranch { .. })))
}
fn reached(world: &World, cut: Cut) -> bool {
    let Some(row) = tier(world) else { return false };
    let run = row.record.run.as_ref();
    match cut {
        Cut::Report => row.flight.as_ref().is_some_and(|flight| flight.report.is_some()),
        Cut::Rerun => run.is_some_and(|run| matches!(run.part, Part::Rerun { .. })) && row.flight.as_ref().is_some_and(|flight| flight.start.is_some() && flight.report.is_none()),
        Cut::ProbeBranch => row.probe.as_ref().is_some_and(|probe| matches!(probe.branch, forge_top::TierBranch::Pending { .. })),
        Cut::Probe => run.is_some_and(|run| matches!(run.part, Part::Probe { .. })) && row.flight.as_ref().is_some_and(|flight| flight.start.is_some() && flight.report.is_none()),
        Cut::ProbeRelease => row.probe.as_ref().is_some_and(|probe| match probe.branch {
            forge_top::TierBranch::Pending { entry } => release_entry(world, entry),
            forge_top::TierBranch::Absent | forge_top::TierBranch::Made => false,
        }),
        Cut::RunRelease => match row.branch {
            forge_top::TierBranch::Pending { entry } => release_entry(world, entry),
            forge_top::TierBranch::Absent | forge_top::TierBranch::Made => false,
        },
        Cut::Red => !row.record.flakes.is_empty() && verifications(world).any(|verification| verification.tiers.iter().any(|tier| matches!(tier.verdict, Some(forge_top::TierVerdict { blame: Blame::Owns(_) | Blame::Suspect(_), .. })))) && world.store.rows.values().any(|record| matches!(record, Record::Forge { row } if matches!(row.as_ref(), forge_top::Stored::Change(row) if matches!(row.change.state, temper_engine_domain_forge_change::State::Red { .. })))),
        Cut::Repaired => verifications(world).any(|verification| verification.tiers.iter().any(|tier| tier.repaired.is_some())),
    }
}
fn cold_at(world: &mut World, cut: Cut) {
    world.hold_artifacts = matches!(cut, Cut::Report);
    world.hold_write_answers = match cut {
        Cut::ProbeBranch => Some(HoldWrite::All),
        Cut::ProbeRelease => Some(HoldWrite::DeleteBranch(b"temper/tier/browser.2.0")),
        Cut::RunRelease => Some(HoldWrite::DeleteBranch(b"temper/tier/browser.2")),
        Cut::Report | Cut::Rerun | Cut::Probe | Cut::Red | Cut::Repaired => None,
    };
    for _ in 0..RED_TICKS {
        if reached(world, cut) && world.pending.is_empty() && world.store.pending.is_empty() {
            world.hold_artifacts = false;
            world.held_artifacts.clear(); // the old process's unsent read; the durable report state reissues it.
            world.restart(true, true);
            for _ in 0..TIER_TICKS {
                world.tick();
                if world.root.ready() {
                    return;
                }
            }
            panic!("restart at {cut:?} opens, step={:?}", world.root.restart_step());
        }
        world.tick();
    }
    panic!("cold cut {cut:?} reached; tier={:?}", tier(world));
}
fn produced_red_range(suspects: Suspects) -> World {
    let world = World::configured_limits(
        true,
        true,
        false,
        1000,
        Gate::default(),
        None,
        StoryLimits { tier: true, tier_reports: true, tier_suspects: Some(suspects), ..StoryLimits::default() },
    );
    let (mut world, owner, first, branch) =
        change_world_in(world, Change { no_ci: false, owner_gate: false, tracked: true });
    temper_engine_domain_world::tier_runs::park_owner(&mut world, &owner);
    produce_path_content(&mut world, &first, &branch, b"failures", FAILED);
    for _ in 0..TIER_TICKS {
        world.tick();
        if world.merges == 1 {
            break;
        }
    }
    let (mut world, _owner, second, branch) =
        change_world_in(world, Change { no_ci: false, owner_gate: false, tracked: false });
    produce_path_content(&mut world, &second, &branch, b"file", b"another read path");
    for _ in 0..TIER_TICKS {
        world.tick();
        if world.merges == 2 {
            break;
        }
    }
    // The root's scripted artifact boundary adds one failure that passes alone;
    // the fake still supplies conclusions, selected stable failures and probes.
    world.artifact_answers = [Box::from(WITH_FLAKE), Box::from(FAILED)].into();
    world
}
#[test]
fn a_cold_restart_reads_back_every_red_record() {
    let cases: &[(Suspects, &[Cut])] = &[
        (
            Suspects::Bisect,
            &[Cut::Report, Cut::Rerun, Cut::ProbeBranch, Cut::Probe, Cut::ProbeRelease, Cut::RunRelease, Cut::Red],
        ),
        (Suspects::TellAll, &[Cut::Report, Cut::Rerun, Cut::RunRelease, Cut::Red]),
    ];
    for (suspects, cuts) in cases {
        let mut world = produced_red_range(*suspects);
        for cut in *cuts {
            cold_at(&mut world, *cut);
        }
        let record = &tier(&world).expect("red tier restored").record;
        assert_eq!(record.flakes.as_ref(), &[policy::Flake { test: b"suite.flaky".as_slice().into(), count: 1 }]);
        let blamed: Vec<_> = verifications(&world)
            .flat_map(|row| row.tiers.iter())
            .filter_map(|tier| tier.verdict.as_ref())
            .map(|verdict| &verdict.blame)
            .collect();
        assert!(blamed.iter().any(|blame| match suspects {
            Suspects::Bisect => matches!(blame, Blame::Owns(_)),
            Suspects::TellAll => matches!(blame, Blame::Suspect(_)),
        }));
        fake::advance(&mut world.fake, &world.fake_env, b"org/repo", b"main", b"failures", b"", 1)
            .expect("outside repair removes the failing tests");
        cold_at(&mut world, Cut::Repaired);
        for _ in 0..TIER_TICKS {
            world.tick();
            if tier(&world).is_some_and(|row| {
                row.record.run.is_none()
                    && matches!(&row.record.failing, policy::Failures::Tests(tests) if tests.is_empty())
            }) {
                break;
            }
        }
        assert!(world.root.ready());
        assert_eq!(tier(&world).expect("repaired tier").record.failing, policy::Failures::Tests(Box::new([])));
        assert!(world.store.rows.values().all(|record| !matches!(record, Record::Forge { row } if matches!(row.as_ref(), forge_top::Stored::Change(row) if matches!(row.change.state, temper_engine_domain_forge_change::State::Red { .. })))));
    }
}
