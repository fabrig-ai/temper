//! Installation HTTP traffic, with the root's wake-driven procedure scheduler.
//!
//! Two-minute warmup, then a 1200-second window scaled by three. The shipped
//! 1800-second stall stays outside the window. CI takes 7200 virtual seconds.
//! Run with `TEMPER_RATE=1` to print the installation's HTTP routes and costs.
//! This is QA because the three protocol worlds together take over 0.5 s.
//! Conservative phase allowance: two passes per endpoint/resource (fast and
//! slow), scaled by three: at most 6*(7*N+1) requests/hour either way.
//! The chosen 1200 s contains whole steady periods of the shipped cadences.
//!
//! Busy stories keep changes unlanded, report staggered worker pushes every
//! 300 s, accept each reported new head, and let CI turn green after 120 s.
//! They warm for 300 s, measure 600 s, and scale by six. Main moves every
//! 600 s in the second shape. Totals allow two fast/slow passes per endpoint
//! at the boundary: +/-12*(7*N+1) requests/hour. Fake replies take no time.
use std::{collections::BTreeMap, fmt::Write};

use skein_fake_forge_domain::{Observation, api as raw};
use skein_forge_machine_world::joint::{self, REPO};
use skein_lib::{Duration, Wall};
use temper_engine_domain_forge as forge;
use temper_engine_domain_forge_change as change;
use temper_engine_forge_world::{changes::change_row, translate::commit};
use temper_engine_protocol_forge_world::protocol::Protocol;
use temper_service::limits::{Profile, profile};

const WINDOW: u64 = 1200;
const SCALE: u64 = 3600 / WINDOW;
const KINDS: [&str; 16] = [
    "Base branch tip",
    "Base combined status",
    "Base check-runs",
    "Head branch",
    "Head combined status",
    "Head check-runs",
    "Pull item",
    "Pull comments",
    "Pull details",
    "Pull GraphQL",
    "Repository listing",
    "Pull lookup",
    "Compare",
    "Pull files",
    "Pull reviews",
    "File contents",
];

fn branch_name(branch: &[u8]) -> forge::Name {
    forge::Name {
        forge: REPO.forge,
        repository: REPO.repository,
        what: forge::What::Branch(branch.split(|byte| *byte == b'/').map(Box::from).collect()),
    }
}

struct Scheduler {
    due: Vec<bool>,
    until: Vec<Option<Wall>>,
    entry: u64,
    iterating: bool,
    steps: Vec<(Wall, u64)>,
}
impl Scheduler {
    fn run(&mut self, world: &mut Protocol, seconds: u64) {
        let end = world.wall().as_nanos() + Duration::from_secs(seconds).as_nanos();
        loop {
            for request in world.take_seen() {
                if let forge::Request::WakeProcedure { task } = request {
                    self.due[usize::try_from(task - 51).expect("task index")] = true;
                } else if let forge::Request::ChangeDecision { task, decision, entry, .. } = request {
                    let index = usize::try_from(task - 51).expect("task index");
                    if let Some(entry) = entry {
                        world.event(forge::Event::Committed { entry });
                    }
                    match decision {
                        change::Decision::Wait { until } => self.until[index] = until,
                        change::Decision::Hold(change::Hold::BranchMoved) if self.iterating => {
                            world.event(forge::Event::ReleaseChange { task });
                            self.due[index] = true;
                        }
                        other @ (change::Decision::None
                        | change::Decision::Delegate(_)
                        | change::Decision::CancelGates { .. }
                        | change::Decision::Effect(_)
                        | change::Decision::Ready
                        | change::Decision::QueueRepair
                        | change::Decision::Finish { .. }
                        | change::Decision::Cancel
                        | change::Decision::Hold(_)) => {
                            panic!("kept change must wait or acknowledge its reported head: {other:?}")
                        }
                    }
                } else if let forge::Request::Refused { task } = request {
                    panic!("fixture capacity must admit task {task}");
                }
            }
            for index in 0..self.due.len() {
                if self.until[index].is_some_and(|at| world.wall() >= at) {
                    self.due[index] = true;
                }
                if self.due[index] {
                    self.due[index] = false;
                    self.until[index] = None;
                    if self.iterating {
                        self.steps.push((world.wall(), 51 + u64::try_from(index).expect("task")));
                    }
                    world.event(forge::Event::StepChange {
                        task: 51 + u64::try_from(index).expect("task index"),
                        entry: self.entry,
                        heard: change::Heard {
                            lacking: None,
                            spend: None,
                            delegate: change::Status::Unknown,
                            effect: change::EffectResult::None,
                            released: self.iterating,
                            cancelled: false,
                        },
                        gates: Box::new([]),
                        queue_repair_active: false,
                        kept: self.iterating,
                    });
                    self.entry += 1;
                }
            }
            world.run_for(0);
            if !world.seen().is_empty() {
                continue;
            }
            if world.wall().as_nanos() >= end {
                return;
            }
            world.run_for(1);
        }
    }
}

fn pending(count: usize) -> (Protocol, Scheduler) {
    configured(count, 7200)
}

fn configured(count: usize, latency: u64) -> (Protocol, Scheduler) {
    let shipped = profile(Profile::Small).forge;
    let mut limits = temper_engine_forge_world::LIMITS;
    limits.client = shipped.client;
    limits.resources_per_task = shipped.resources_per_task;
    limits.change_policy.stall = shipped.change_policy.stall;
    let mut checks = joint::checks(1000);
    checks.latency_min = Duration::from_secs(latency);
    checks.latency_max = checks.latency_min;
    let mut world = Protocol::github_with_limits(71, checks, &limits);
    world.adopt();
    for index in 0..count {
        let task = 51 + u64::try_from(index).expect("task index");
        let branch = format!("temper/{task}").into_bytes().into_boxed_slice();
        world.produce_file(&branch, format!("file{task}").as_bytes(), b"change");
        let pull = world.open_pull(&branch, b"main");
        let head = commit(world.branch(&branch));
        let mut row = change_row();
        row.task = task;
        row.branch.clone_from(&branch);
        row.pull = Some(pull);
        row.change.task = task;
        row.change.since = world.wall();
        row.change.gates = Box::new([]);
        row.change.last_head = Some(head);
        row.change.state = change::State::Checking { head };
        world.event(forge::Event::Change { row });
        let name = branch_name(&branch);
        world.event(forge::Event::Names { task, resources: Box::new([name.clone(), branch_name(b"main")]) });
        world.event(forge::Event::Hold { task, resource: name.clone(), from: None });
        world.event(forge::Event::Rename {
            task,
            resources: Box::new([
                name,
                forge::Name { forge: REPO.forge, repository: REPO.repository, what: forge::What::Pull(pull) },
            ]),
        });
        world.run_for(0);
    }
    (
        world,
        Scheduler { due: vec![true; count], until: vec![None; count], entry: 1, iterating: false, steps: Vec::new() },
    )
}

fn commit_hex(number: u64) -> String {
    let mut hex = String::new();
    for byte in &commit(number)[..20] {
        write!(hex, "{byte:02x}").expect("write to string");
    }
    hex
}

// Snapshot after draining, then count newly completed HTTP lines. A timestamp
// filter including the start instant would count requests already in costs().
fn histogram(world: &Protocol, start: usize) -> BTreeMap<String, u64> {
    let bases: Vec<_> = world
        .base_reads()
        .iter()
        .filter_map(|(_, _, commit)| *commit)
        .map(|commit| {
            let mut hex = String::new();
            for byte in &commit[..20] {
                write!(hex, "{byte:02x}").expect("write to string");
            }
            hex
        })
        .collect();
    let base = commit_hex(world.branch(b"main"));
    let mut table: BTreeMap<_, _> = KINDS.map(|kind| (kind.to_owned(), 0)).into_iter().collect();
    for (_, line) in world.requests().skip(start) {
        let text = std::str::from_utf8(line).expect("HTTP request line");
        let route = text.split(" HTTP/").next().expect("request target");
        if route.starts_with("GET /app/") || route.starts_with("POST /app/") {
            continue; // App JWT, outside the installation's budget.
        }
        let path = route.split_ascii_whitespace().nth(1).expect("HTTP target");
        let path = path.split('?').next().expect("path without query");
        let kind = if route == "POST /graphql" {
            "Pull GraphQL"
        } else {
            assert!(route.starts_with("GET "), "unexpected installation write: {route}");
            match path {
                "/repos/acme/widgets/branches/main" => "Base branch tip",
                "/repos/acme/widgets/issues" => "Repository listing",
                path if path.starts_with("/repos/acme/widgets/branches/temper/") => "Head branch",
                path if path.contains("/commits/") && path.ends_with("/status") => {
                    if path.contains(&base) || bases.iter().any(|base| path.contains(base)) {
                        "Base combined status"
                    } else {
                        "Head combined status"
                    }
                }
                path if path.contains("/commits/") && path.ends_with("/check-runs") => {
                    if path.contains(&base) || bases.iter().any(|base| path.contains(base)) {
                        "Base check-runs"
                    } else {
                        "Head check-runs"
                    }
                }
                path if path.contains("/issues/") && path.ends_with("/comments") => "Pull comments",
                path if path.starts_with("/repos/acme/widgets/issues/") => "Pull item",
                "/repos/acme/widgets/pulls" => "Pull lookup",
                path if path.contains("/compare/") => "Compare",
                path if path.ends_with("/files") => "Pull files",
                path if path.ends_with("/reviews") => "Pull reviews",
                path if path.contains("/contents/") => "File contents",
                path if path.starts_with("/repos/acme/widgets/pulls/") => "Pull details",
                _ => panic!("unexpected installation route: {route}"),
            }
        };
        *table.get_mut(kind).expect("named HTTP kind") += 1;
    }
    table
}

#[test]
fn pending_changes_installation_requests_per_hour() {
    let mut measured = Vec::new();
    for count in 1..=3 {
        let (mut world, mut scheduler) = pending(count);
        scheduler.run(&mut world, 120); // Past the shipped 30-second settle.
        let start = world.requests().count();
        let costs = world.costs();
        scheduler.run(&mut world, WINDOW);
        let table = histogram(&world, start);
        let total: u64 = table.values().sum();
        let changes = u64::try_from(count).expect("small change count");
        // Recorded 1200-second counts: 30 branch reads and 60 of each of the
        // pull's six HTTP endpoints per change, plus 240 repository listings.
        let expected: BTreeMap<_, _> = KINDS
            .into_iter()
            .zip([
                0,
                0,
                0,
                30 * changes,
                60 * changes,
                60 * changes,
                60 * changes,
                60 * changes,
                60 * changes,
                60 * changes,
                240,
                0,
                0,
                0,
                0,
                0,
            ])
            .map(|(kind, count)| (kind.to_owned(), count))
            .collect();
        assert_eq!(table, expected, "recorded per-kind baseline for N={count}");
        assert_eq!(total * SCALE, [1890, 3060, 4230][count - 1]);
        assert_eq!(total, world.costs() - costs, "all installation HTTP requests charged exactly once");
        assert_eq!(world.delivery_hints(), 0, "no public webhook or delivery hints in this shape");
        assert_eq!(world.writes(), 0, "all fixture pulls stay open; pending CI admits no writes");
        for index in 0..count {
            let task = 51 + u64::try_from(index).expect("task index");
            let Some(forge::Stored::Change(row)) = world.stored().get(&forge::Key::Change(task)) else {
                panic!("active change retained")
            };
            assert!(matches!(row.change.state, change::State::Checking { .. }));
            let head = world.branch(&row.branch);
            assert!(world.observations().iter().any(|o| matches!(o,
                Observation::Reported { commit, state: raw::Check::Pending, .. } if *commit == head
            )));
            assert!(!world.observations().iter().any(|o| matches!(o,
                Observation::Reported { commit, state: raw::Check::Passed, .. } if *commit == head
            )));
        }
        measured.push(table);
        world.finish();
    }
    if std::env::var_os("TEMPER_RATE").is_some() {
        eprintln!("requests/hour (equal to charged answer costs):     N=1     N=2     N=3");
        for kind in KINDS {
            eprintln!(
                "{kind:40} {:7} {:7} {:7}",
                measured[0][kind] * SCALE,
                measured[1][kind] * SCALE,
                measured[2][kind] * SCALE
            );
        }
        eprintln!("TOTAL                                       1890    3060    4230");
    }
}

// A kept procedure still decides on CI changes, but never queues a merge.
// The worker reports each push; BranchMoved acknowledgement accepts that head.
fn busy_run(world: &mut Protocol, scheduler: &mut Scheduler, count: usize, moving: bool, from: u64, to: u64) {
    for second in from..to {
        if moving && second > 0 && second % 600 == 0 {
            world.push_file(b"main", b"other", &second.to_be_bytes());
        }
        for index in 0..count {
            let offset = u64::try_from(index * 300 / count).expect("stagger");
            if second > 0 && second % 300 == offset {
                let task = 51 + u64::try_from(index).expect("task");
                let branch = format!("temper/{task}");
                let name = branch_name(branch.as_bytes());
                world.event(forge::Event::Claim {
                    task,
                    attempt: second,
                    writes: Box::new([name.clone()]),
                    holders: Box::new([]),
                });
                world.push_file(branch.as_bytes(), format!("file{task}").as_bytes(), &second.to_be_bytes());
                world.event(forge::Event::Answered {
                    task,
                    attempt: second,
                    pushed: Box::new([(name, commit(world.branch(branch.as_bytes())))]),
                });
            }
        }
        scheduler.run(world, 1);
    }
}

fn sharing(world: &Protocol, start: usize, bucket: u64) -> (u64, u64, u64) {
    let bases: Vec<_> = world.base_reads().iter().filter_map(|(_, _, commit)| *commit).collect();
    let mut groups = BTreeMap::new();
    let mut cohorts = BTreeMap::new();
    for (at, read, returned) in &world.base_reads()[start..] {
        let (commit, checks) = if let skein_forge::api::Read::Branch { .. } = read {
            (returned.expect("base tip answered"), false)
        } else if let skein_forge::api::Read::Checks { commit } = read {
            if !bases.contains(commit) {
                continue;
            }
            (*commit, true)
        } else {
            panic!("only tips and checks are sampled")
        };
        let first = cohorts.entry((commit, checks)).or_insert(at.as_nanos());
        if at.as_nanos() - *first >= bucket {
            *first = at.as_nanos();
        }
        *groups.entry((*first, commit, checks)).or_insert(0_u64) += 1;
    }
    let reads = groups.iter().map(|((_, _, checks), n)| n * if *checks { 2 } else { 1 }).sum();
    let save = groups.iter().map(|((_, _, checks), n)| (n - 1) * if *checks { 2 } else { 1 }).sum();
    let chains = groups.iter().filter(|((_, _, checks), _)| !checks).map(|(_, n)| n).sum();
    (reads, save, chains)
}

fn busy(moving: bool) {
    for count in 1..=3 {
        let (mut world, mut scheduler) = configured(count, 120);
        scheduler.iterating = true;
        let epoch = world.wall().as_nanos();
        busy_run(&mut world, &mut scheduler, count, moving, 0, 300);
        let start = world.requests().count();
        let read_start = world.base_reads().len();
        let step_start = scheduler.steps.len();
        let costs = world.costs();
        busy_run(&mut world, &mut scheduler, count, moving, 300, 900);
        let table = histogram(&world, start);
        let total: u64 = table.values().sum();
        assert_eq!(total, world.costs() - costs);
        assert_eq!(world.writes(), 0);
        assert_eq!(world.delivery_hints(), 0);
        let nominal = if moving { [2508_u64, 4332, 6060] } else { [2508_u64, 4260, 6024] };
        let band = 12 * (7 * u64::try_from(count).expect("count") + 1);
        assert!((total * 6).abs_diff(nominal[count - 1]) <= band, "N={count}: {table:?}");
        let exact = sharing(&world, read_start, 1);
        let second = sharing(&world, read_start, 1_000_000_000);
        assert_eq!(exact.0, table["Base branch tip"] + table["Base combined status"] + table["Base check-runs"]);
        let offered: Vec<_> = scheduler.steps[step_start..].iter().map(|(at, _)| *at).collect();
        let issued: Vec<_> = world.base_reads()[read_start..]
            .iter()
            .filter_map(|(at, read, _)| matches!(read, skein_forge::api::Read::Branch { .. }).then_some(*at))
            .collect();
        assert_eq!(offered, issued, "one base-tip read per offered chain at its actual instant");
        assert_eq!(exact.2, u64::try_from(offered.len()).expect("steps"));
        assert_eq!(exact.1 * 6, [36, 72, 162][count - 1]);
        assert_eq!(second.1, exact.1, "no additional cohort within one second");
        for (_, stored) in world.stored() {
            if let forge::Stored::Change(row) = stored {
                assert!(matches!(row.change.state, change::State::Checking { .. } | change::State::Gating { .. }));
            }
        }
        if std::env::var_os("TEMPER_RATE").is_some() {
            eprintln!(
                "moving={moving} N={count} total/h={} base/h={} save exact/1s={}/{} chains={} (600s)",
                total * 6,
                exact.0 * 6,
                exact.1 * 6,
                second.1 * 6,
                exact.2
            );
            for (kind, n) in &table {
                eprintln!("{kind}: {}", n * 6);
            }
            for (at, task) in &scheduler.steps[step_start..] {
                eprintln!("task={task} chain +{}ms", (at.as_nanos() - epoch) / 1_000_000);
            }
        }
        world.finish();
    }
}

#[test]
fn iterating_changes_installation_requests_per_hour() {
    busy(false);
}
#[test]
fn moving_main_installation_requests_per_hour() {
    busy(true);
}
