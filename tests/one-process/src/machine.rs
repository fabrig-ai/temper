//! The composition's disk with the harness's client root at index 2.
use skein_lib::Queue;
pub(crate) use temper_playground::Signalled as SignalHost;
pub(crate) struct Machine(pub temper_playground::Machine);
impl skein_world::Machine for Machine {
    fn open_root(&mut self, path: &[u8]) -> Result<skein_sim::Handle, skein_io::kernel::Error> {
        if path == b"client" {
            return Ok(skein_sim::Handle::new(self.0.disk().reopen_root(2).raw()));
        }
        self.0.open_root(path)
    }
    fn close_root(&mut self, root: skein_sim::Handle) {
        self.0.close_root(root);
    }
    fn cut(&mut self, cut: skein_world::Cut, held: &[skein_sim::Handle], seed: u64) {
        self.0.cut(cut, held, seed);
    }
    fn step(&mut self, call: skein_sim::Call, answers: &mut Queue<skein_sim::Answer>) {
        self.0.step(call, answers);
    }
}
pub(crate) fn fresh() -> skein_fake_machine::Machine {
    let mut disk = temper_playground::Machine::fresh(1000);
    let root = disk.lay(&[]);
    disk.close(root);
    disk
}
