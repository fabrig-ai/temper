//! temper's own lines waiting for the terminal's room.
use skein_lib::Queue;
pub(crate) struct Waiting {
    lines: Queue<Box<[u8]>>,
}
impl Waiting {
    pub(crate) fn new() -> Self {
        Self { lines: Queue::uncapped() }
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
    pub(crate) fn push(&mut self, line: Box<[u8]>) {
        if !(&self.lines).into_iter().any(|waiting| *waiting == line) {
            self.lines.push(line);
        }
    }
    pub(crate) fn say(&mut self, room: u32, mut say: impl FnMut(&[u8])) {
        for _ in 0..room {
            let Some(line) = self.lines.pop() else { break };
            say(&line);
        }
    }
}
