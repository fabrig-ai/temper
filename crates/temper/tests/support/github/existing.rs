//! The existing-installation ctl story on the fake GitHub.
use super::*;

pub fn accept_existing() {
    let clock = Clock::new();
    let began = clock.now().now;
    let scratch = Scratch::new("github_accept");
    let directory = skein_scratch::paths::package().join("tests/support/github");
    let mut description = cluster::Cluster::read_from(scratch.path(), &directory);
    let [clients] = crate::support::free_addresses(&[]);
    description.bind("story.clients", &clients.to_string());
    description.prepare("forge");
    let (mut members, hosts) = description.members();
    members.up(&["forge", "llm"]);
    let mut world = real::World::new(Existing {
        members,
        app: None,
        stage: ExistingStage::Fakes,
        scratch: scratch.path().to_path_buf(),
        deadline: began.saturating_add(WITHIN),
        outputs: Vec::new(),
    });
    for host in hosts {
        world.spawn(|| host);
    }
    world.spawn(cluster::Root::client);
    let outcome = world.run(&clock, WITHIN);
    assert!(outcome.procs.iter().all(Host::is_empty));
    scratch.remove();
    eprintln!("github accept: complete {:?}", clock.now().now.saturating_since(began));
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExistingStage {
    Fakes,
    Engine,
    Stray,
    Accept,
    Accepted,
    Repositories,
    Adopt,
    Missing,
    Stopping,
}
struct Existing {
    members: cluster::Members,
    app: Option<App>,
    stage: ExistingStage,
    scratch: PathBuf,
    deadline: Time,
    outputs: Vec<Vec<u8>>,
}
impl Existing {
    fn queue(&self, host: &mut cluster::Root, calls: Vec<ctl::Call>) {
        let link = format!("http://{}", self.members.capture("engine", "link").expect("engine link"));
        let mut batch = ctl::Calls::linked(&link, &self.scratch.join("client"));
        for call in calls {
            batch.push(ctl::Step::Run(call));
        }
        let cluster::Root::Client { calls, .. } = host else { panic!("client root") };
        *calls = Some(Box::new(batch));
    }
    fn called(&mut self, host: &mut cluster::Root, calls: &ctl::Calls) {
        for ended in calls.ends() {
            self.outputs.push(ended.stdout.clone());
            self.outputs.push(ended.stderr.clone());
            assert_eq!(
                ended.exit,
                Some(Exit::Code(if self.stage == ExistingStage::Missing { 2 } else { 0 })),
                "ctl {:?}: {} {}",
                self.stage,
                String::from_utf8_lossy(&ended.stdout),
                String::from_utf8_lossy(&ended.stderr)
            );
        }
        let ended = calls.ends().last().expect("completed call");
        match self.stage {
            ExistingStage::Stray => {
                let text = String::from_utf8_lossy(&calls.ends()[calls.ends().len() - 2].stdout);
                assert!(text.contains("kept: the App is read-only"), "{text}");
                assert!(text.contains("forge accept 1 1"), "{text}");
                assert!(text.contains("Connecting: no installation accepted yet"), "{text}");
                let row = Judge::row(&ctl::json(ended));
                assert_eq!(row["read_only"], true);
                assert_eq!(row["installations"][0]["stray"], true);
                self.no_uninstall();
                self.queue(host, vec![ctl::Call::new(&["forge", "accept", "1", "1", "--key", "accept-existing"])]);
                self.stage = ExistingStage::Accept;
            }
            ExistingStage::Accept => {
                let text = String::from_utf8_lossy(&ended.stdout);
                assert!(text.contains("accepted: installation 1 on forge 1"), "{text}");
                assert!(text.contains("forge repositories 1 --installation 1"), "{text}");
                self.queue(
                    host,
                    vec![ctl::Call::new(&["forge", "list"]), ctl::Call::new(&["forge", "list", "--json"])],
                );
                self.stage = ExistingStage::Accepted;
            }
            ExistingStage::Accepted => {
                let text = String::from_utf8_lossy(&calls.ends()[0].stdout);
                assert!(!text.contains("not accepted"), "{text}");
                assert!(text.contains("Connected as"), "{text}");
                let row = Judge::row(&ctl::json(ended));
                assert_eq!(row["installations"][0]["stray"], false);
                assert_eq!(row["state"], "connected");
                self.queue(host, vec![ctl::Call::new(&["forge", "repositories", "1", "--installation", "1"])]);
                self.stage = ExistingStage::Repositories;
            }
            ExistingStage::Repositories => {
                let text = String::from_utf8_lossy(&ended.stdout);
                assert!(text.contains("acme/app"), "{text}");
                self.queue(host, vec![Judge::adoption()]);
                self.stage = ExistingStage::Adopt;
            }
            ExistingStage::Adopt => {
                assert!(ctl::json(ended)["adopted"].is_object());
                self.queue(host, vec![ctl::Call::new(&["forge", "accept", "1", "999"])]);
                self.stage = ExistingStage::Missing;
            }
            ExistingStage::Missing => {
                let text = String::from_utf8_lossy(&ended.stderr);
                assert!(text.contains("GitHub has no installation 999 of this App"), "{text}");
                self.members.stop();
                self.stage = ExistingStage::Stopping;
            }
            ExistingStage::Fakes | ExistingStage::Engine | ExistingStage::Stopping => unreachable!("no calls"),
        }
    }
    fn no_uninstall(&self) {
        let lines = fs::read_to_string(self.members.dir("forge").join("observations")).expect("observations");
        let rows: Vec<Value> = lines.lines().map(|line| serde_json::from_str(line).expect("observation")).collect();
        assert!(!rows.iter().any(|row| row["observation"] == "installation_deleted"));
        let created: Vec<_> = rows.iter().filter(|row| row["observation"] == "installation_created").collect();
        assert_eq!(created.len(), 1, "only the initial fixture snapshot");
        assert_eq!(created[0]["installation"], 1);
        assert!(!rows.iter().any(|row| row["observation"] == "installations_full"));
        if self.stage == ExistingStage::Stopping {
            assert_eq!(rows.last().expect("checkpoint")["observations_lost"], 0);
        }
    }
}
impl Referee<cluster::Root> for Existing {
    fn act(&mut self, now: Time, hosts: &mut [cluster::Root]) {
        self.members.act(now, hosts);
        if self.members.failed() {
            self.stage = ExistingStage::Stopping;
            return;
        }
        let host = hosts.last_mut().expect("client root");
        if self.stage == ExistingStage::Fakes && self.members.is_up() {
            // The fixture's public PEM is skein-fake-forge-github's fixed key 0.
            // Keep its App and installation, without a conversion replacing them.
            let pem = fs::read(skein_scratch::paths::package().join("tests/support/github/public-app.pem"))
                .expect("public fake key");
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(self.scratch.join("github-app.pem"))
                .expect("private fixture key");
            file.write_all(&pem).expect("fixture key");
            self.app = Some(App { id: 4242, pem: Secret::new(pem.into_boxed_slice()) });
            self.members.prepare("engine");
            let path = self.members.dir("engine").join("config/temper.json");
            let mut document: Value = serde_json::from_slice(&fs::read(&path).expect("engine config")).expect("JSON");
            document["application"]["forges"][0]["app_read_only"] = true.into();
            fs::write(path, serde_json::to_vec(&document).expect("JSON")).expect("read-only forge");
            self.members.up(&["engine"]);
            self.members.act(now, hosts);
            self.stage = ExistingStage::Engine;
        } else if self.stage == ExistingStage::Engine && self.members.is_up() {
            self.queue(
                host,
                vec![
                    ctl::Call::sign_in(),
                    ctl::Call::new(&[
                        "forge",
                        "connect",
                        "1",
                        "github-app",
                        &self.app.as_ref().expect("made App").id.to_string(),
                        self.scratch.join("github-app.pem").to_str().expect("key path"),
                        "--key",
                        "connect-existing",
                    ]),
                    ctl::Call::new(&["forge", "list", "--until", "connected:1"]),
                    ctl::Call::new(&["forge", "list", "--until", "settled"]),
                    ctl::Call::new(&["forge", "list"]),
                    ctl::Call::new(&["forge", "list", "--json"]),
                ],
            );
            self.stage = ExistingStage::Stray;
        } else if host.is_empty()
            && matches!(
                self.stage,
                ExistingStage::Stray
                    | ExistingStage::Accept
                    | ExistingStage::Accepted
                    | ExistingStage::Repositories
                    | ExistingStage::Adopt
                    | ExistingStage::Missing
            )
        {
            let cluster::Root::Client { calls, .. } = host else { panic!("client root") };
            let calls = calls.take().expect("completed calls");
            self.called(host, &calls);
        }
    }
    fn observe(&mut self, now: Time, hosts: &[cluster::Root]) {
        self.members.observe(now, &hosts[..hosts.len() - 1]);
        if let Some(stopped) = self.members.stopped() {
            assert!(hosts.iter().all(Host::is_empty));
            for member in stopped {
                assert_eq!(
                    member.exit,
                    Some(Exit::Code(0)),
                    "{}: {}",
                    member.name,
                    String::from_utf8_lossy(&member.stderr)
                );
                assert!(!member.killed && member.survivors.is_empty());
            }
            self.no_uninstall();
            let scan = Scan::new(self.app.as_ref().expect("made App").pem.expose());
            for output in &self.outputs {
                scan.clean(output);
            }
        }
    }
    fn next_deadline(&self) -> Option<Time> {
        Some(self.members.next_deadline().map_or(self.deadline, |at| at.min(self.deadline)))
    }
    fn overdue(&self, now: Time) -> Option<String> {
        (now >= self.deadline).then(|| format!("GitHub acceptance waited in {:?}", self.stage))
    }
    fn passed(&self) -> bool {
        self.members.passed()
    }
}
