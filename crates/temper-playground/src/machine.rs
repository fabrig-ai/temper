//! Disk roots and cuts of the playground machine.
use skein_lib::Queue;
/// The fake machine's disk and its state, worker and prepared agent roots.
pub struct Machine {
    disk: skein_fake_machine::Machine,
    prepared: Box<[u8]>,
}
impl Machine {
    /// Opens prepared beneath the worker root when it exists, else the worker root.
    #[must_use]
    pub fn new(disk: skein_fake_machine::Machine, prepared: &[u8]) -> Self {
        Self { disk, prepared: prepared.into() }
    }
    /// The disk, for a caller that lays or observes its files.
    pub fn disk(&mut self) -> &mut skein_fake_machine::Machine {
        &mut self.disk
    }
    /// Returns the disk after the world stops.
    #[must_use]
    pub fn into_disk(self) -> skein_fake_machine::Machine {
        self.disk
    }

    /// Lays the state root, then the worker with workspaces, trash and home.
    #[must_use]
    pub fn fresh(user: u32) -> skein_fake_machine::Machine {
        let mut machine = skein_fake_machine::Machine::for_user(user);
        for items in [
            vec![],
            vec![
                skein_fake_machine::Item::directory(b"workspaces"),
                skein_fake_machine::Item::directory(b"trash"),
                skein_fake_machine::Item::directory(b"home"),
            ],
        ] {
            let root = machine.lay(&items);
            machine.close(root);
        }
        machine
    }
}
impl skein_sim_world::Machine for Machine {
    fn open_root(&mut self, path: &[u8]) -> Result<skein_sim::Handle, skein_io::kernel::Error> {
        if path == b"agent" {
            let worker = self.disk.reopen_root(1);
            // The parent has no workspace; repository runs adopt its prepared directory.
            let root = match self.disk.open(worker, &self.prepared, skein_fake_machine::How::Directory) {
                Ok(root) => {
                    self.disk.close(worker);
                    root
                }
                Err(skein_fake_machine::Refusal::NotFound) => worker,
                Err(why) => panic!("prepared agent directory: {why:?}"),
            };
            return Ok(skein_sim::Handle::new(root.raw()));
        }
        let index = match path {
            b"state" => 0,
            b"worker" => 1,
            _ => return Err(skein_io::kernel::Error::NotFound),
        };
        Ok(skein_sim::Handle::new(self.disk.reopen_root(index).raw()))
    }
    fn close_root(&mut self, root: skein_sim::Handle) {
        self.disk.close(skein_fake_machine::Opened::new(root.raw()));
    }
    fn cut(&mut self, cut: skein_sim_world::Cut, held: &[skein_sim::Handle], seed: u64) {
        match cut {
            skein_sim_world::Cut::Kill => {
                for handle in held {
                    self.disk.close(skein_fake_machine::Opened::new(handle.raw()));
                }
            }
            skein_sim_world::Cut::PowerLoss => self.disk.crash(seed),
        }
    }
    fn step(&mut self, call: skein_sim::Call, answers: &mut Queue<skein_sim::Answer>) {
        skein_fake_machine::step(&mut self.disk, call, answers);
    }
}
