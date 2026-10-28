//! A request key in a completed log write, followed by that log's sync.
use skein_io::kernel::{Complete, Done, Op};
use skein_lib::RequestKey;
use std::collections::BTreeMap;
// The standalone service's stores may each keep an old and a new log open.
const LOGS: usize = 32;
// A story can arm up to sixteen durable holds before it runs.
pub(super) const CUES: usize = 16;
pub struct Cue {
    key: RequestKey,
    logs: BTreeMap<i32, Log>,
    held: bool,
}
#[derive(Default)]
struct Log {
    carry: Vec<u8>,
    written: bool,
}
impl Cue {
    pub(super) fn new(key: RequestKey) -> Self {
        Self { key, logs: BTreeMap::new(), held: false }
    }
    pub(super) fn held(&self) -> bool {
        self.held
    }
    pub(super) fn complete(&mut self, complete: &Complete) {
        if self.held {
            return;
        }
        match (&complete.kind, &complete.result) {
            (Op::Open { path, .. }, Ok(Done::Fd(fd))) => {
                let leaf = path.rsplit(|byte| *byte == b'/').next().expect("open path");
                if leaf
                    .strip_prefix(b"log-")
                    .is_some_and(|number| number.len() == 20 && number.iter().all(u8::is_ascii_digit))
                {
                    assert!(self.logs.len() < LOGS, "bounded open service logs");
                    self.logs.insert(fd.raw(), Log::default());
                }
            }
            (Op::Write { fd, bytes, from, .. }, Ok(Done::Count(count))) => {
                if let Some(log) = self.logs.get_mut(&fd.raw()) {
                    for byte in &bytes[*from as usize..(*from + *count) as usize] {
                        log.carry.push(*byte);
                        if log.carry.len() == self.key.len() {
                            log.written |= log.carry.as_slice() == self.key;
                            log.carry.remove(0);
                        }
                    }
                }
            }
            (Op::Sync { fd }, Ok(Done::Nothing)) => {
                self.held |= self.logs.get(&fd.raw()).is_some_and(|log| log.written);
            }
            (Op::Close { fd }, _) => {
                self.logs.remove(&fd.raw());
            }
            _ => {}
        }
    }
}
