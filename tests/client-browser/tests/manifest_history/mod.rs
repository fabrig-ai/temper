//! Read-only checkpoints keep the first post's assertions before history continues.
use skein_fake_forge_github::manifest::Manifest;
use skein_fake_peers::{Listening, forge::Peer};
use skein_fake_person::{Find, Next, Step as PersonStep};
use skein_fake_person_browser::{BrowserFace, Play, Script, Step};
use skein_io::kernel::{Complete, Submit};
use skein_lib::{Queue, Time, Wall};
use skein_ui::Role;
use skein_world::Host;
use std::{cell::Cell, rc::Rc};
use temper_client_browser_world::WITHIN;

type Counts = Rc<Cell<(usize, usize)>>;

pub(super) struct Observed {
    pub peer: Peer,
    counts: Counts,
    pub first_manifest: Option<(Box<[u8]>, Manifest)>,
}
impl Observed {
    pub fn new(peer: Peer) -> Self {
        Self { peer, counts: Rc::new(Cell::new((0, 0))), first_manifest: None }
    }
    pub fn counts(&self) -> Counts {
        Rc::clone(&self.counts)
    }
}
impl Host for Observed {
    fn iterate(&mut self, now: Time, wall: Wall) {
        self.peer.iterate(now, wall);
        let posts = self
            .peer
            .observations()
            .iter()
            .filter(|record| {
                matches!(record,
            skein_fake_peers::forge::Observation::Head { call, .. }
            if call.method == skein_http::Method::Post && call.target.starts_with(b"/settings/apps/new?"))
            })
            .count();
        let installations = self.peer.github_layer().expect("GitHub layer").installations().iter().count();
        self.counts.set((posts, installations));
        if posts == 1 && self.first_manifest.is_none() {
            let target = self
                .peer
                .observations()
                .iter()
                .find_map(|record| match record {
                    skein_fake_peers::forge::Observation::Head { call, .. }
                        if call.method == skein_http::Method::Post =>
                    {
                        Some(call.target.as_ref())
                    }
                    _ => None,
                })
                .expect("first manifest post");
            let state = target.strip_prefix(b"/settings/apps/new?state=").expect("manifest state");
            let Some(manifest) = self.peer.github_layer().expect("GitHub layer").manifest(state) else {
                return; // The head can arrive before the form body has been decoded.
            };
            // Preserve the first post's fields before Continue may replace the record for this state.
            self.first_manifest = Some((
                state.into(),
                Manifest {
                    name: manifest.name.clone(),
                    url: manifest.url.clone(),
                    public: manifest.public,
                    redirect_url: manifest.redirect_url.clone(),
                    setup_url: manifest.setup_url.clone(),
                    setup_on_update: manifest.setup_on_update,
                    hook_attributes: manifest.hook_attributes.clone(),
                    default_permissions: manifest.default_permissions.clone(),
                    default_events: manifest.default_events.clone(),
                    account: manifest.account.clone(),
                },
            ));
        }
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        self.peer.completions()
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        self.peer.submissions()
    }
    fn work_pending(&self, now: Time) -> bool {
        self.peer.work_pending(now)
    }
    fn next_deadline(&self) -> Option<Time> {
        self.peer.next_deadline()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        self.peer.next_policy_deadline()
    }
    fn is_empty(&self) -> bool {
        self.peer.is_empty()
    }
    fn worst_case(&self) -> u64 {
        self.peer.worst_case()
    }
    fn operations(&self) -> u32 {
        self.peer.operations()
    }
}
impl Listening for Observed {
    fn address(&self) -> std::net::SocketAddr {
        self.peer.address().expect("listening GitHub")
    }
    fn listening_address(&self) -> Option<std::net::SocketAddr> {
        self.peer.address()
    }
    fn shutdown(&mut self) {
        self.peer.shutdown();
    }
}

pub(super) struct History {
    scripts: [Script; 3],
    stage: usize,
    counts: Counts,
}
impl History {
    pub fn new(first: Script, counts: Counts) -> Self {
        let see =
            |role, name: &[u8]| Step::Person(PersonStep::See { find: Find::exactly_named(role, name), within: WITHIN });
        Self {
            scripts: [
                first,
                Script::new(Box::new([
                    Step::Back,
                    see(Role::Heading { level: 2 }, "temper’s App, on GitHub".as_bytes()),
                    see(Role::Button, b"Continue to GitHub"),
                    see(Role::Link, b"Back to temper"),
                ])),
                Script::new(Box::new([
                    Step::Person(PersonStep::Press { find: Find::exactly_named(Role::Button, b"Continue to GitHub") }),
                    see(Role::Link, b"Create GitHub App"),
                ])),
            ],
            stage: 0,
            counts,
        }
    }
}
impl Play for History {
    fn next(&mut self, face: &mut BrowserFace<'_>, now: Time) -> Next<Box<[u8]>> {
        let next = self.scripts[self.stage].next(face, now);
        if next == Next::Done {
            let expected = if self.stage == 2 { (2, 0) } else { (1, 0) };
            assert_eq!(self.counts.get(), expected, "manifest posts and installations at stage {}", self.stage);
            if self.stage < 2 {
                self.stage += 1;
                return Next::Wait { until: None };
            }
        }
        next
    }
    fn at(&self) -> u32 {
        self.scripts[self.stage].at()
    }
}
