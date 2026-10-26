use jig_protocol_store::codec::component::Levels;
use temper_one_process_world::{
    deployment::{Deployment, Held},
    fixtures,
    world::{self, World},
};

// Chat's stored settings, identities, account and terminal, plus the
// component's format/layout/installation/place/attempt rows.
const STORE_ROWS: usize = 52;
const JIG_ROWS: usize = 46;
// One forge Names row; two credential digests and the account's key.
const APPLICATION_ROWS: usize = 1;
const SECRET_ROWS: usize = 3;

fn chat() -> Deployment {
    World::run(fixtures::calm(fixtures::SEED), world::Story::Chat { words: b"Hello" }).deployment()
}

#[test]
fn a_captured_deployment_starts_again_on_a_fresh_machine() {
    let deployment = chat();
    // A copied deployment may have any active generation, and stale retained
    // generations must not enter its offline read.
    for kept in [deployment.clone(), generation(&deployment, 9)] {
        let held = Held::read(&kept);
        let mut restarted = world::over(&kept, fixtures::calm(fixtures::SEED));
        let after = restarted.deployment();
        assert_eq!(after.configuration, kept.configuration);
        assert_eq!(after.startup, kept.startup);
        assert_eq!(Held::read(&after), held);
    }
}

#[test]
fn a_deployment_reads_its_format_rows_manifest_and_turns() {
    let held = Held::read(&chat());
    assert_eq!(held.generation, 1);
    assert_eq!(held.manifest.generation, 1);
    let unkept = Levels { skein: None, smith: None, jig: None, application: None };
    assert_eq!(held.format.levels, unkept);
    assert_eq!(held.manifest.before, unkept);
    assert_eq!(held.manifest.after, unkept);
    assert_eq!(held.store.len(), STORE_ROWS);
    assert_eq!(held.jig.len(), JIG_ROWS);
    assert_eq!(held.application.len(), APPLICATION_ROWS);
    assert_eq!(held.secrets.len(), SECRET_ROWS);
    assert_eq!(held.jig_secrets.len(), SECRET_ROWS);
    // The chat has one task, with one accepted turn of its first attempt.
    let turns: Vec<_> = held
        .turns
        .iter()
        .map(|(task, turns)| {
            (
                *task,
                turns.iter().map(|turn| (turn.attempt, turn.turn, turn.spent, turn.opens_window)).collect::<Vec<_>>(),
            )
        })
        .collect();
    assert_eq!(turns, [(1, vec![(1, 1, 1, true)])]);
}

fn generation(deployment: &Deployment, number: u64) -> Deployment {
    let mut kept = deployment.clone();
    let prefix = format!("generations/{number}");
    for (path, entry) in &deployment.files {
        if let Some(suffix) = path.strip_prefix(b"generations/1") {
            kept.files.insert([prefix.as_bytes(), suffix].concat().into(), entry.clone());
        }
    }
    kept.files.get_mut(b"active".as_slice()).expect("active").bytes = format!("{number}\n").into_bytes().into();
    let mut manifest = Held::read(deployment).manifest;
    manifest.generation = number;
    kept.files.get_mut(format!("{prefix}/manifest").as_bytes()).expect("manifest").bytes = manifest.encode();
    kept
}
