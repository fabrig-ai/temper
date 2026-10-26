//! Provision a project or project-only service member through the public client on the story's ring.
use jig_client::{self as client, contract as c};
use jig_client_shell_native::Shell;
use skein_io::kernel::{Complete, Fd, Op, Submit};
use skein_lib::{List, Queue, Secret, Time, Token, Wall};
use std::path::Path;

const KEY: client::Key = client::Key { asker: client::Asker::JIG, number: 1 };
const PROJECT: u32 = 1;
const ME: client::Handle = client::Handle { asker: client::Asker::JIG, number: 0 };
const SEED: u64 = 713;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    ConfirmingOwner,
    Creating,
    Ready,
    Signing,
    Keeping,
}

pub struct Member {
    shell: Shell,
    client: client::Client,
    out: Queue<client::Request>,
    owner: Option<Secret>,
    made: Option<(u64, Secret)>,
    project: Option<(&'static str, u32)>,
    key: client::Key,
    phase: Phase,
    closing: bool,
    sequence: u64,
    operations: std::collections::BTreeMap<Token, (Token, Option<Token>)>,
    names: std::collections::BTreeMap<Token, Token>,
}
impl Member {
    pub fn new(root: Fd, base: &Path, owner: &Path, directory: &str) -> Self {
        let address = std::fs::read(owner.join("address")).expect("fixture owner address");
        let host = std::str::from_utf8(&address)
            .expect("fixture address text")
            .trim()
            .strip_prefix("http://")
            .expect("fixture HTTP address");
        let engine = skein_io::Addr::Ip(host.parse().expect("fixture socket"));
        let token = Secret::new(std::fs::read(owner.join("token")).expect("fixture owner token").into());
        let member = base.join(directory);
        std::fs::create_dir_all(&member).expect("fixture member directory");
        super::super::write_private(&member.join("address"), &address);
        let bounds = temper_client_shell_cli::limits::native();
        let mut shell = Shell::new(temper_client_shell_cli::limits::settings(engine, directory.as_bytes().into()));
        shell.adopt(root, skein_shell::effective_user(), SEED);
        Self {
            shell,
            client: client::Client::new(&bounds.root.domain.client, SEED).expect("native client bounds"),
            out: Queue::with_capacity(client::max_out(&bounds.root.domain.client)),
            owner: Some(token),
            made: None,
            project: None,
            key: KEY,
            phase: Phase::ConfirmingOwner,
            closing: false,
            sequence: super::MEMBER,
            operations: std::collections::BTreeMap::new(),
            names: std::collections::BTreeMap::new(),
        }
    }
    pub fn project(root: Fd, base: &Path, owner: &Path, name: &'static str, number: u32) -> Self {
        let mut host = Self::new(root, base, owner, "case-project");
        host.project = Some((name, number));
        host.key = client::Key { asker: client::Asker::JIG, number: 2 };
        host
    }
    fn event(&mut self, now: Time, wall: Wall, event: client::Event) {
        self.client.step(now, wall, event, &mut self.out);
        self.drain();
    }
    fn drain(&mut self) {
        while let Some(request) = self.out.pop() {
            match request {
                client::Request::ServiceCredential { party, credential } => self.made = Some((party, credential)),
                client::Request::Told { told: client::Told::Answered { key, outcome } } if key == self.key => {
                    if let Some((_, number)) = self.project {
                        assert!(
                            matches!(outcome, client::Outcome::Answered(client::Answered::Jig(c::Answer::Done(
                                c::Outcome::Owner(c::OwnerOutcome::ProjectMade { project })
                            ))) if project == number),
                            "project creation answered: {outcome:?}"
                        );
                        self.close();
                        continue;
                    }
                    assert!(
                        matches!(outcome, client::Outcome::Answered(client::Answered::Jig(c::Answer::Done(_)))),
                        "member creation answered: {outcome:?}"
                    );
                    assert!(self.made.is_some(), "member creation supplied no credential; public outcome: {outcome:?}");
                    self.phase = Phase::Ready;
                }
                client::Request::Told { .. } => {}
                request @ (client::Request::Exchange { .. }
                | client::Request::OpenStream { .. }
                | client::Request::CloseStream
                | client::Request::Save { .. }
                | client::Request::Erase { .. }
                | client::Request::Down { .. }
                | client::Request::SignOut
                | client::Request::Authorized(_)) => self.shell.down(request),
            }
        }
    }
    pub fn iterate(&mut self, now: Time, wall: Wall) {
        self.shell.iterate(now, wall);
        if self.closing {
            while self.shell.events().pop().is_some() {}
            return;
        }
        if let Some((_, records)) = self.shell.loaded() {
            let mut kept = List::with_capacity(u32::try_from(records.len()).expect("bounded records"));
            for record in records {
                kept.push(record).expect("record room");
            }
            self.event(now, wall, client::Event::Start { kept });
            let owner = self.owner.take().expect("owner token");
            self.event(now, wall, client::Event::SignIn { with: client::With::Held(owner) });
            self.event(now, wall, client::Event::Watch { handle: ME, subject: client::Watchable::Jig(c::Subject::Me) });
        }
        while let Some(event) = self.shell.events().pop() {
            self.event(now, wall, event);
        }
        self.client.fire(now, wall, &mut self.out);
        self.drain();
        self.client.reclaim();
        if self.phase == Phase::ConfirmingOwner
            && self.client.link().state == client::Link::Live
            && self.client.document(ME).is_some_and(|document| {
                document.state == client::State::Live
                    && matches!(document.latest, Some(client::Latest::Jig(c::Delivery::Me(_))))
            })
        {
            self.phase = Phase::Creating;
            let request = match self.project {
                Some((name, _)) => c::Ask::MakeProject {
                    name: c::Text::new(name.as_bytes().into(), 64).expect("fixture project name"),
                    policy: 1,
                },
                None => c::Ask::MakeService {
                    project: PROJECT,
                    name: c::Text::new(b"settings-member".as_slice().into(), 64).expect("fixture name"),
                    role: c::HeldRole::Member,
                },
            };
            self.event(
                now,
                wall,
                client::Event::Send { key: self.key, name: None, request: client::Ask::Jig(Box::new(request)) },
            );
        }
        if self.phase == Phase::Ready {
            let (_, credential) = self.made.as_ref().expect("member credential");
            let credential = credential.clone();
            self.phase = Phase::Signing;
            self.event(now, wall, client::Event::SignIn { with: client::With::Service(credential) });
        }
        if self.phase == Phase::Signing && self.client.link().state == client::Link::Live {
            let (party, _) = self.made.as_ref().expect("made member");
            if self.client.link().party == Some(*party) {
                let credential = self.client.link().credential.expect("member held credential").clone();
                self.shell.keep(Some(credential));
                self.phase = Phase::Keeping;
            }
        }
        if self.phase == Phase::Keeping
            && let Some(kept) = self.shell.kept()
        {
            assert!(kept, "member token is durable");
            self.close();
        }
    }
    pub fn close(&mut self) {
        self.closing = true;
        self.shell.close();
    }
    pub fn complete(&mut self, mut complete: Complete) {
        let (original, cancel) = self.operations.remove(&complete.op).expect("member operation");
        if self.names.get(&original) == Some(&complete.op) {
            self.names.remove(&original);
        }
        complete.op = original;
        if let (Op::Cancel { target }, Some(original)) = (&mut complete.kind, cancel) {
            *target = original;
        }
        self.shell.completions().push(complete);
    }
    pub fn submission(&mut self) -> Option<Submit> {
        let mut submit = self.shell.submissions().pop()?;
        let original = submit.op;
        let token = Token::new(self.sequence);
        self.sequence = self.sequence.checked_add(1).expect("finite story operations");
        let mut cancel = None;
        if let Op::Cancel { target } = &mut submit.kind {
            cancel = Some(*target);
            *target = self.names.get(target).copied().unwrap_or_else(|| {
                let absent = Token::new(self.sequence);
                self.sequence = self.sequence.checked_add(1).expect("finite story operations");
                absent
            });
        }
        self.operations.insert(token, (original, cancel));
        self.names.insert(original, token);
        submit.op = token;
        Some(submit)
    }
    pub fn work_pending(&self, now: Time) -> bool {
        self.shell.work_pending(now)
            || (!self.closing && self.client.next_deadline().is_some_and(|deadline| deadline <= now))
    }
    pub fn next_deadline(&self) -> Option<Time> {
        self.shell
            .next_deadline()
            .into_iter()
            .chain((!self.closing).then(|| self.client.next_deadline()).flatten())
            .min()
    }
    pub fn is_empty(&self) -> bool {
        self.closing && self.shell.is_empty() && self.operations.is_empty()
    }
    pub fn status(&self) -> String {
        format!(
            "phase={:?} link={:?} made={} signed={} closing={} shell_empty={} operations={} pending={:?}",
            self.phase,
            self.client.link().state,
            self.made.is_some(),
            self.made.as_ref().is_some_and(|(party, _)| self.client.link().party == Some(*party)),
            self.closing,
            self.shell.is_empty(),
            self.operations.len(),
            self.client.pending(self.key)
        )
    }
    pub fn worst_case(&self) -> u64 {
        self.shell.worst_case()
            + client::worst_case(&temper_client_shell_cli::limits::native().root.domain.client)
                .expect("native client memory")
    }
}
