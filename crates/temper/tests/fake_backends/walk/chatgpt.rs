use super::*;

pub(super) fn run() {
    let walk = Walk::new("walk-chatgpt-github");
    let mut acts = github::manifest_acts();
    acts.push(adopt_github());
    acts.push(accounts());
    acts.push(Act::After(Box::new(|seen| {
        let before = seen.last().expect("Model before sign-in");
        assert!(!usable(before), "Model still offers ChatGPT sign-in");
        let document = ctl::json(before);
        let provider = document["accounts"]["deployment_facts"]["providers"]
            .as_array()
            .expect("provider offers")
            .iter()
            .find(|provider| provider["provider"] == 1)
            .expect("OpenAI offered");
        assert_eq!(provider["sign_in"], true);
        Call::new(&["account", "connect", "1", "--key", "walk-sign-in"])
    })));
    acts.push(Act::Visit);
    acts.extend(first_chat());
    let seen = walk.run(acts);
    assert_adopted(&seen, 2, "widget");
    assert_reply(&seen);
    let [github, sign_in] = seen.visits.as_slice() else { panic!("GitHub and ChatGPT departing tabs") };
    assert_eq!(github.iter().map(|hop| hop.status).collect::<Vec<_>>(), [200, 200, 302, 302, 200]);
    assert_eq!(sign_in.iter().map(|hop| hop.status).collect::<Vec<_>>(), [302, 200]);
    assert!(sign_in.iter().all(|hop| hop.method == "GET"));
    // ctl Connect supplies no browser return address: this visit ends at the callback
    // 200. The lab walk proves the browser returns to the welcome.
    let [authorize, callback] = sign_in.as_slice() else { panic!("issuer and callback landing") };
    assert!(authorize.path.starts_with("/api/accounts/authorize?"));
    assert!(callback.path.starts_with("/auth/callback?"));
    walk.scratch.remove();
}
