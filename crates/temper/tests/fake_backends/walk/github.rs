use super::*;

fn offered(seen: &[Ended]) {
    let [before_model, before_forge] = seen else { panic!("welcome preconditions read") };
    assert!(!usable(before_model), "the welcome offers its Model step");
    let row = forge_number(before_forge, 2);
    assert_eq!(row["state"], "not_connected", "the welcome offers GitHub: {row}");
    assert!(row["first"].is_null());
}
fn finish(walk: &Walk, mut acts: Vec<Act>) -> Seen {
    acts.push(adopt_github());
    acts.push(walk.add_key());
    acts.extend(first_chat());
    let seen = walk.run(acts);
    assert_adopted(&seen, 2, "widget");
    assert_reply(&seen);
    seen
}
pub(super) fn one_click() {
    let walk = Walk::new("walk-github-key");
    let seen = finish(&walk, manifest_acts());
    let [visit] = seen.visits.as_slice() else { panic!("one departing tab") };
    assert_eq!(visit.iter().map(|hop| hop.status).collect::<Vec<_>>(), [200, 200, 302, 302, 200]);
    assert_eq!(visit.iter().map(|hop| hop.method.as_str()).collect::<Vec<_>>(), ["GET", "POST", "GET", "GET", "GET"]);
    let [manifest, create, convert, install, setup] = visit.as_slice() else { panic!("five GitHub hops") };
    assert!(manifest.path.starts_with("/forges/2/github/manifest?"));
    assert!(create.path.starts_with("/settings/apps/new?"));
    assert!(convert.path.starts_with("/forges/2/github/redirect?"));
    assert!(install.path.starts_with("/apps/"));
    assert!(setup.path.starts_with("/forges/2/github/setup?"));
    walk.scratch.remove();
}
pub(super) fn own_app() {
    let walk = Walk::new("walk-own-app-key");
    let pem = walk.sandbox().join("data/github-app.pem");
    let seen = finish(
        &walk,
        vec![
            accounts(),
            forges(),
            Act::After(Box::new(move |seen| {
                offered(seen);
                Call::new(&["forge-connect", "2", "github-app", "1", path(&pem), "--key", "walk-own-app", "--json"])
            })),
            wait_forges(),
            Act::After(Box::new(|seen| {
                let row = forge_number(seen.last().expect("own App connected"), 2);
                let [installation] = row["installations"].as_array().expect("seeded installation").as_slice() else {
                    panic!("one seeded installation")
                };
                assert_eq!(installation["stray"], true, "Connecting still offers Install until the tab comes back");
                Call::new(&["forge-install", "2", "--key", "walk-install"])
            })),
            Act::Visit,
            wait_forges(),
        ],
    );
    let [visit] = seen.visits.as_slice() else { panic!("own-App departing tab") };
    assert_eq!(visit.iter().map(|hop| hop.status).collect::<Vec<_>>(), [302, 200]);
    let [install, setup] = visit.as_slice() else { panic!("install and setup") };
    assert!(install.path.starts_with("/apps/temper-fake/installations/new?"));
    // Reconcile may already have pruned the unaccepted seed installation;
    // the return accepts the installation GitHub made or updated for this tab.
    assert!(setup.path.starts_with("/forges/2/github/setup?installation_id="));
    walk.scratch.remove();
}

pub(super) fn manifest_acts() -> Vec<Act> {
    vec![
        accounts(),
        forges(),
        Act::After(Box::new(|seen| {
            offered(seen);
            Call::new(&["forge-connect", "2", "github", "--key", "walk-github"])
        })),
        Act::Visit,
        wait_forges(),
    ]
}
