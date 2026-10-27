//! The browser tier (client/browser.md, 4.3): Chromium against `temper serve`, on a scratch deployment whose
//! forge and LLM endpoint are unreached, or the box's first-run demo with its own fakes, serving the
//! bundle scripts/browser.sh builds. Its helpers are jig's
//! `tests/client-browser/src/lib.rs`'s, for temper's binary and bundle.
use skein_fake_person::{Find, Step as PersonStep};
use skein_fake_person_browser::{
    Setup, Step,
    harness::{Names, Output, Printed},
};
use skein_ui::Role;
use std::{
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
};
mod authorize;
mod configuration;
mod forgery;
mod github;
mod issuer;

/// How long a story waits for what a page shows.
pub const WITHIN: skein_lib::Duration = skein_lib::Duration::from_secs(10);
/// How long a story watches for what a page must not show.
pub const ABSENT: skein_lib::Duration = skein_lib::Duration::from_secs(1);
/// The first chat's link on the Chats page.
pub const FIRST_CHAT: &[u8] = b"T1";
/// Its address.
pub const FIRST_CHAT_ADDRESS: &[u8] = b"/chats/1";

/// The `temper` binary beside the test executable.
#[must_use]
pub fn temper() -> PathBuf {
    let exe = std::env::current_exe().expect("test executable");
    let binary = exe.parent().and_then(Path::parent).expect("target directory").join("temper");
    assert!(binary.is_file(), "build the browser server with heavy cargo build -p temper: {}", binary.display());
    binary
}
#[must_use]
pub fn chromium() -> PathBuf {
    if let Some(path) = std::env::var_os("SKEIN_TEST_CHROMIUM") {
        let path = PathBuf::from(path);
        assert!(path.is_file(), "SKEIN_TEST_CHROMIUM must name a file");
        return path.canonicalize().expect("configured Chromium");
    }
    for directory in std::env::split_paths(&std::env::var_os("PATH").expect("PATH")) {
        let path = directory.join("chromium");
        if path.is_file() {
            return path.canonicalize().expect("Chromium on PATH");
        }
    }
    panic!("Chromium is required: set SKEIN_TEST_CHROMIUM or put chromium on PATH");
}
/// The bundle scripts/browser.sh builds, named by `TEMPER_TEST_BUNDLE`.
#[must_use]
pub fn bundle() -> PathBuf {
    let path = PathBuf::from(
        std::env::var_os("TEMPER_TEST_BUNDLE")
            .expect("run scripts/browser.sh to build the bundle and set TEMPER_TEST_BUNDLE"),
    );
    let path = if path.is_absolute() { path } else { skein_scratch::paths::workspace().join(path) };
    assert!(
        path.join("index.html").is_file(),
        "build the bundle with scripts/bundle.sh {}: it holds no index.html",
        path.display()
    );
    path.canonicalize().expect("absolute bundle")
}
/// The heading of the first chat a story starts: its number and its first line, `title`.
#[must_use]
pub fn first_chat(title: &[u8]) -> Find {
    Find::exactly_named(Role::Heading { level: 1 }, &[b"T1 \xc2\xb7 ".as_slice(), title].concat())
}
/// From a signed-in page in a world with a usable model, start the first chat with `words`, one line,
/// and see it titled by them.
#[must_use]
pub fn start_chat(words: &[u8]) -> Vec<Step> {
    let see = |find| Step::Person(PersonStep::See { find, within: WITHIN });
    let composer = || Find::exactly_named(Role::TextArea, b"Start a chat");
    vec![
        see(Find::exactly_named(Role::Link, b"Set up temper")),
        Step::Person(PersonStep::Go { address: Box::from(b"/chats".as_slice()) }),
        see(live()),
        see(composer()),
        Step::Person(PersonStep::Type { find: composer(), words: Box::from(words) }),
        Step::Person(PersonStep::Press { find: Find::exactly_named(Role::Button, b"Start chat") }),
        see(first_chat(words)),
    ]
}
/// Choose the playground at the printed link's welcome before starting a chat.
#[must_use]
pub fn start_playground_chat(words: &[u8]) -> Vec<Step> {
    let mut steps = vec![
        Step::Person(PersonStep::See {
            find: Find::exactly_named(Role::Button, b"Try the playground"),
            within: WITHIN,
        }),
        Step::Person(PersonStep::Press { find: Find::exactly_named(Role::Button, b"Try the playground") }),
        // The box starts the demo's group only when asked: wait for its start to be answered.
        Step::Person(PersonStep::See { find: Find::exactly_named(Role::Status, b"Ready"), within: WITHIN }),
        Step::Person(PersonStep::Gone { find: Find::exactly_named(Role::Status, b"Starting"), within: WITHIN }),
    ];
    steps.extend(start_chat(words).into_iter().skip(2));
    steps
}
fn live() -> Find {
    Find::exactly_named(Role::Status, b"Live")
}

/// A scratch deployment for one story's `temper serve` or `temper box`.
pub struct Tier {
    scratch: skein_scratch::Scratch,
    forged: Box<[u8]>,
    command: &'static str,
}
impl Tier {
    #[must_use]
    pub fn new(name: &str) -> Self {
        let scratch = skein_scratch::Scratch::new(name);
        configuration::write(scratch.path(), &temper(), &bundle());
        Self { scratch, forged: forgery::request(), command: "serve" }
    }
    /// Give the deployment a usable model without a credential or a reachable LLM peer.
    #[must_use]
    pub fn with_model(self) -> Self {
        let path = self.scratch.path().join("temper.json");
        let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).expect("deployment")).expect("JSON");
        // As in jig's browser deployment, a provider offering no credential source gets a
        // deployment-held None account at startup: the view's demo_accounts fixture has this shape.
        value["engine"]["llm"]["providers"][0]["sources"] = serde_json::json!({});
        configuration::private(&path, &serde_json::to_vec(&value).expect("model deployment JSON"));
        self
    }
    /// The first-run demo, with the box's own fake LLM and forge, started when the playground is chosen.
    #[must_use]
    pub fn demo(name: &str) -> Self {
        let scratch = skein_scratch::Scratch::new(name);
        configuration::demo(scratch.path(), &temper(), &bundle());
        Self { scratch, forged: forgery::request(), command: "box" }
    }
    /// Account offers with a static public key set, served by the same scratch server before Visit.
    #[must_use]
    pub fn accounts(name: &str) -> Self {
        let tier = Self::reserved(name);
        let path = tier.scratch.path().join("temper.json");
        let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).expect("deployment")).expect("JSON");
        let private_bundle = tier.scratch.path().join("bundle");
        copy_bundle(&bundle(), &private_bundle);
        let digest = fs::read_dir(&private_bundle)
            .expect("bundle entries")
            .map(|entry| entry.expect("bundle entry"))
            .find(|entry| entry.file_type().expect("bundle entry type").is_dir())
            .expect("digest directory");
        // Public, synthetic 2048-bit RSA modulus; no callback or signature is exercised here.
        let keys = serde_json::json!({"keys":[{"kty":"RSA","n":format!("g{}", "A".repeat(341)),"e":"AQAB"}]});
        fs::write(digest.path().join("issuer.webmanifest"), serde_json::to_vec(&keys).expect("key set"))
            .expect("static issuer key set");
        let socket = value["application"]["clients"]["listener"]["socket"].as_str().expect("reserved socket");
        let keys =
            format!("http://{socket}/bundle/{}/issuer.webmanifest", digest.file_name().to_str().expect("digest"));
        value["application"]["clients"]["bundle"] = serde_json::json!(private_bundle);
        value["engine"]["limits"]["overrides"]["accounts.sign_in_deadline_ms"] = 2000.into();
        let sign_in = serde_json::json!({
            "issuer":"http://127.0.0.1:9", "client":"temper-test", "scopes":["openid","chat"],
            "authorization":"http://127.0.0.1:9/authorize", "token":"http://127.0.0.1:9/token",
            "keys":keys, "transport":"plaintext", "must_grant":["chat"], "redirect_path":"callback"
        });
        account_provider(&mut value, &sign_in);
        configuration::private(&path, &serde_json::to_vec(&value).expect("accounts deployment JSON"));
        tier
    }
    /// A real loopback issuer for the tab's complete sign-in round trip.
    #[must_use]
    pub fn accounts_sign_in(name: &str) -> (Self, skein_fake_peers::oauth::Peer) {
        use std::os::fd::{AsRawFd, IntoRawFd};
        let tier = Self::reserved(name);
        let listener = TcpListener::bind("127.0.0.1:0").expect("reserve issuer port");
        let address = listener.local_addr().expect("issuer address");
        let origin = format!("http://{address}");
        let path = tier.scratch.path().join("temper.json");
        let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).expect("deployment")).expect("JSON");
        let sign_in = serde_json::json!({
            "issuer":origin, "client":"temper-test", "scopes":["openid","chat"],
            "authorization":format!("{origin}/authorize"), "token":format!("{origin}/token"),
            "revocation":format!("{origin}/revoke"), "keys":format!("{origin}/keys"),
            "transport":"plaintext", "must_grant":["chat"], "redirect_path":"callback"
        });
        account_provider(&mut value, &sign_in);
        configuration::private(&path, &serde_json::to_vec(&value).expect("accounts deployment JSON"));
        let mut peer = issuer::peer(address, &sign_in);
        peer.adopt_listener(skein_io::kernel::Fd::new(listener.as_raw_fd()), address).expect("held issuer listener");
        let _ = listener.into_raw_fd();
        (tier, peer)
    }
    /// A GitHub forge on the browser harness's loop, with a fixed clients' listener.
    #[must_use]
    pub fn github(name: &str) -> (Self, skein_fake_peers::forge::Peer) {
        use std::os::fd::{AsRawFd, IntoRawFd};
        let tier = Self::reserved(name);
        let listener = TcpListener::bind("127.0.0.1:0").expect("reserve GitHub port");
        let address = listener.local_addr().expect("GitHub address");
        let path = tier.scratch.path().join("temper.json");
        let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).expect("deployment")).expect("JSON");
        value["application"]["forges"] = serde_json::json!([{
            "number":1, "kind":"github", "api":{"socket":address.to_string()}, "transport":"plaintext",
            "git":format!("http://{address}"), "web":format!("http://{address}")
        }]);
        value["application"]["identities"] = serde_json::json!([{"number":1,"forge":1,"login":null,"author":null}]);
        github::project_creation(&mut value, address);
        configuration::private(&path, &serde_json::to_vec(&value).expect("GitHub deployment JSON"));
        let mut peer = github::peer(address);
        peer.adopt_listener(skein_io::kernel::Fd::new(listener.as_raw_fd()), address).expect("held GitHub listener");
        let _ = listener.into_raw_fd();
        (tier, peer)
    }
    /// Sign in ctl with the held server link, then open its printed manifest link in the tab.
    #[must_use]
    pub fn github_connect(&self) -> Vec<Step> {
        use skein_fake_person_browser::{Command, Opens, Stdin};
        let command = |words: &[&str], stdin, opens| {
            let mut args = vec!["ctl".into(), self.scratch.path().join("client").into_os_string()];
            args.extend(words.iter().map(std::ffi::OsString::from));
            Step::Run(Command { program: temper(), args, env: vec![], stdin, opens, within: WITHIN })
        };
        vec![
            command(&["sign-in", "-"], Stdin::Printed, None),
            Step::Exited { code: 0, within: WITHIN },
            command(
                &["forge-connect", "1", "github", "--key", "github-connect"],
                Stdin::Closed,
                Some(Opens {
                    prefix: b"http://127.0.0.1:".as_slice().into(),
                    on: Output::Stdout,
                    keep: b"manifest-link".as_slice().into(),
                }),
            ),
            Step::Exited { code: 0, within: WITHIN },
        ]
    }
    /// A tier whose server listens at a loopback port reserved beforehand, so that a restart opens the same
    /// address and a page reconnects to it. Jig's tier has the same hook.
    #[must_use]
    pub fn reserved(name: &str) -> Self {
        let tier = Self::new(name);
        let listener = TcpListener::bind("127.0.0.1:0").expect("reserve restart port");
        let address = listener.local_addr().expect("restart address");
        let path = tier.scratch.path().join("temper.json");
        let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).expect("deployment")).expect("JSON");
        value["application"]["clients"]["listener"]["socket"] = serde_json::json!(address.to_string());
        configuration::private(&path, &serde_json::to_vec(&value).expect("reserved deployment JSON"));
        tier
    }
    #[must_use]
    pub fn setup(&self) -> Setup<'_> {
        let mut args = vec![self.command.into()];
        args.extend(configuration::flags(self.scratch.path()));
        Setup {
            server: temper(),
            args: args.clone(),
            restart: args,
            printed: Printed {
                sent: skein_fake_person_browser::harness::Sent::Page,
                prefix: b"http://127.0.0.1:",
                on: Output::Stdout,
                names: Names::Url,
            },
            chromium: chromium(),
            downloads: None,
            expected: vec![],
            origins: vec![],
        }
    }
    /// The banner's status node names the link's state; signed out, the page says how to sign in.
    #[must_use]
    pub fn states(&self) -> jig_client_fake_person_browser::States {
        jig_client_fake_person_browser::States {
            live: live(),
            offline: Find::exactly_named(Role::Status, b"Offline"),
            signed_out: Find::text(b"Signed out. To sign in, open a sign-in link temper serve printed."),
        }
    }
    /// A forged chat would show its words on the chats page if accepted.
    #[must_use]
    pub fn target(&self) -> jig_client_fake_person_browser::Target<'_> {
        jig_client_fake_person_browser::Target {
            prefix: b"/jig",
            page: b"/chats",
            request: &self.forged,
            done: Find::text(b"Forged"),
        }
    }
}

/// The two account stories share their provider and room for its driver.
fn account_provider(value: &mut serde_json::Value, sign_in: &serde_json::Value) {
    // One git operation leaves the small profile's sockets room for the accounts driver.
    value["application"]["limits"]["overrides"]["git_component.operations"] = 1.into();
    value["application"]["limits"]["overrides"]["checkout.workspaces"] = 1.into();
    value["engine"]["llm"]["providers"] = serde_json::json!([{
        "name":"Test provider", "number":17, "signed_in_plan":false, "models":["fake"],
        "sources":{"keys":true,"sign_in":sign_in}
    }]);
}

/// Keep the built bundle immutable while giving this one-serve story its static issuer document.
fn copy_bundle(from: &Path, to: &Path) {
    fs::create_dir(to).expect("private bundle directory");
    for entry in fs::read_dir(from).expect("built bundle") {
        let entry = entry.expect("bundle entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("bundle entry type").is_dir() {
            copy_bundle(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("copy bundle file");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::configuration::{UNREACHED, documents};
    use std::path::Path;
    fn tier_configuration() -> (serde_json::Value, serde_json::Value) {
        let (agent, value) =
            documents(Path::new("/private/test"), Path::new("/test/temper"), Path::new("/test/bundle"));
        (serde_json::from_slice(&agent).expect("agent JSON"), serde_json::from_slice(&value).expect("deployment JSON"))
    }
    #[test]
    fn the_tier_names_the_bundle_for_its_server() {
        assert_eq!(tier_configuration().1["application"]["clients"]["bundle"], "/test/bundle");
    }
    #[test]
    fn the_tier_listens_on_loopback_at_a_dynamic_port() {
        assert_eq!(tier_configuration().1["application"]["clients"]["listener"]["socket"], "127.0.0.1:0");
    }
    #[test]
    fn the_tiers_forge_is_unreached() {
        assert_eq!(tier_configuration().1["application"]["forges"][0]["api"]["socket"], UNREACHED);
    }
    #[test]
    fn the_tiers_llm_endpoint_is_unreached() {
        let (agent, value) = tier_configuration();
        assert_eq!(value["engine"]["llm"]["endpoints"][0]["address"]["socket"], UNREACHED);
        assert_eq!(agent["endpoints"][0]["address"], UNREACHED);
    }
    #[test]
    fn the_tiers_paths_are_absolute() {
        let value = tier_configuration().1;
        assert_eq!(value["application"]["forges"][0]["api_token"], "/private/test/secrets/forge-1.api-token");
        assert_eq!(value["application"]["workers"][0]["root"], "/private/test/worker-root");
    }
}
