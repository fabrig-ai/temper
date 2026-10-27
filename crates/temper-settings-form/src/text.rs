//! Counted text with the existing ctl truncation rules.
use alloc::boxed::Box;
use skein_lib::Writer;
#[derive(Debug)]
pub struct Text {
    writer: Option<Writer>,
    room: usize,
    written: usize,
    left: usize,
}
impl Text {
    #[must_use]
    pub fn measure() -> Self {
        Self { writer: None, room: 0, written: 0, left: 0 }
    }
    #[must_use]
    pub fn structural(&self) -> usize {
        self.written
    }
    #[must_use]
    pub fn bounded(text: u32, structural: usize) -> Self {
        let limit = usize::try_from(text).expect("text bound fits");
        let room = limit.saturating_sub(structural).saturating_sub(64);
        Self { writer: None, room, written: 0, left: 0 }
    }
    #[must_use]
    pub fn writing(mut self, text: u32, structural: usize) -> Self {
        self.cut_line();
        let mut output = Self::bounded(text, structural);
        output.writer = Some(Writer::new(self.written));
        output
    }
    #[must_use]
    pub fn writing_exact(measured: Self) -> Self {
        let mut out = Self::measure();
        out.writer = Some(Writer::new(measured.written));
        out
    }
    #[must_use]
    pub fn room(&self) -> usize {
        self.room
    }
    #[must_use]
    pub fn written(&self) -> usize {
        self.written
    }
    #[must_use]
    pub fn left(&self) -> usize {
        self.left
    }
    #[must_use]
    pub fn with_room(room: usize) -> Self {
        let mut text = Self::measure();
        text.room = room;
        text
    }
    pub fn omit(&mut self, bytes: usize) {
        self.left = self.left.checked_add(bytes).expect("bounded document");
    }
    pub fn reserve(&mut self, bytes: usize) {
        self.room = self.room.checked_sub(bytes).expect("within text bound");
    }
    pub fn put(&mut self, bytes: &[u8]) {
        self.written = self.written.checked_add(bytes.len()).expect("document bound");
        if let Some(writer) = &mut self.writer {
            writer.put(bytes).expect("measured document capacity");
        }
    }
    pub fn number(&mut self, mut number: u64) {
        let mut bytes = [0u8; 20];
        let mut start = bytes.len();
        for _ in 0..20u8 {
            start = start.checked_sub(1).expect("decimal digit slot");
            *bytes.get_mut(start).expect("decimal digit slot") =
                b'0'.checked_add(u8::try_from(number % 10).expect("digit fits")).expect("ASCII digit");
            number /= 10;
            if number == 0 {
                break;
            }
        }
        self.put(bytes.get(start..).expect("decimal range"));
    }
    pub fn words(&mut self, bytes: &[u8]) {
        let end = text_end(bytes, self.room);
        self.room = self.room.checked_sub(end).expect("within text budget");
        self.put(bytes.get(..end).expect("text range"));
        self.left = self
            .left
            .checked_add(bytes.len().checked_sub(end).expect("text cut within value"))
            .expect("document bound");
    }
    /// A result's words follow its state, with multiline words indented within the text budget.
    pub fn result_words(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        if !bytes.contains(&b'\n') {
            self.put(b" \xc2\xb7 ");
            self.words(bytes);
            return;
        }
        self.put(b"\n");
        let mut from = 0;
        for (at, byte) in bytes.iter().chain(core::iter::once(&b'\n')).enumerate() {
            if *byte != b'\n' {
                continue;
            }
            if from != 0 {
                self.words(b"\n");
            }
            let line = bytes.get(from..at).expect("line within words");
            from = at.checked_add(1).expect("document bound");
            if self.room < 2 {
                self.left = self.left.checked_add(line.len()).expect("document bound");
                continue;
            }
            self.room = self.room.checked_sub(2).expect("indent fits text budget");
            self.put(b"  ");
            self.words(line);
        }
    }
    #[must_use]
    pub fn finish(mut self) -> Box<[u8]> {
        self.cut_line();
        self.writer.expect("only written output is finished").finish()
    }
    fn cut_line(&mut self) {
        if self.left > 0 {
            self.put(b"\n");
            self.number(u64::try_from(self.left).expect("byte count fits"));
            self.put(b" bytes left\n");
        }
    }
}
#[must_use]
pub fn text_end(bytes: &[u8], most: usize) -> usize {
    let mut end = bytes.len().min(most);
    for _ in 0..4u8 {
        match bytes.get(end) {
            Some(byte) if byte & 0xc0 == 0x80 => end = end.checked_sub(1).expect("continuation follows a leading byte"),
            _ => break,
        }
    }
    end
}
