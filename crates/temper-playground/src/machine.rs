//! Disk roots and cuts of the playground machine.
use skein_lib::Queue;
/// The fake machine's disk and its named state and worker roots.
pub struct Machine {
    disk: skein_fake_machine::Machine,
}
impl Machine {
    /// Wraps the disk whose roots the selectors name.
    #[must_use]
    pub fn new(disk: skein_fake_machine::Machine) -> Self {
        Self { disk }
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
    fn open_beneath(
        &mut self,
        root: skein_sim::Handle,
        path: &[u8],
    ) -> Option<Result<skein_sim::Handle, skein_io::kernel::Error>> {
        Some(
            self.disk
                .open(skein_fake_machine::Opened::new(root.raw()), path, skein_fake_machine::How::Directory)
                .map(|opened| skein_sim::Handle::new(opened.raw()))
                .map_err(skein_fake_machine::kernel_error),
        )
    }
    fn open_root(&mut self, path: &[u8]) -> Result<skein_sim::Handle, skein_io::kernel::Error> {
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
