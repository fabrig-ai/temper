//! Load and check external input before DIRECTORY opens (client/tools.md, 8.2).
use super::{Class, Down, OneShot, Parsed, Read, Time, Up, arguments, io, limits};
use temper_client_tools::{
    help,
    jig::{
        self,
        call::{Source, Value, Values},
        table::Kind,
    },
    table::TOOLS,
};
impl OneShot {
    pub(super) fn prepare_start(&mut self) {
        match self.call.as_ref().expect("unstarted call") {
            Parsed::File { file: Source::Path(path), .. } => self.directory.load_call(path),
            Parsed::File { file: Source::Stdin, .. } => {
                self.input = Some(Vec::new());
                self.read_input();
            }
            Parsed::Call { call, json } if self.class == Class::Help => {
                let selected = help::selection(call).expect("checked help selection");
                let bytes = if let (help::Selection::Tool(index), true) = (selected, *json) {
                    jig::json::write_tool(TOOLS[usize::from(index)], &self.root_env.limits.tools).expect("help record")
                } else {
                    let mut out =
                        skein_lib::Writer::new(usize::try_from(self.root_env.limits.tools.text).expect("help bound"));
                    help::selected(selected, &mut out).expect("help fits text window");
                    let mut exact = skein_lib::Writer::new(out.written());
                    help::selected(selected, &mut exact).expect("measured help");
                    exact.finish()
                };
                self.output.push(false, bytes);
                self.preparing = false;
                self.finish(0, false);
            }
            Parsed::Call { call, .. } if call.tool == temper_client_tools::jig::rows::EXPORT_AT => {
                self.directory.create_record(self.move_file.as_deref().expect("export file"));
            }
            Parsed::Call { call, .. } if arguments::key_file(call).is_some() => {
                if call.tool == temper_client_tools::jig::rows::IMPORT_AT {
                    self.directory.record_path(self.move_file.as_deref().expect("import file"));
                }
                self.directory.load_key(arguments::key_file(call).expect("key file"), arguments::key_bytes(call));
            }
            Parsed::Call { call, .. } => {
                if let Some((index, path)) = text_file(call) {
                    self.directory.load_text(
                        index,
                        path,
                        matches!(call.tool, jig::rows::CHANGE_SETTINGS_AT | jig::rows::PROPOSE_SETTINGS_AT),
                    );
                } else if self.class != Class::SignIn && call.values.contains(&Some(Values::One(Value::Stdin))) {
                    self.input = Some(Vec::new());
                    self.read_input();
                } else {
                    self.preparing = false;
                }
            }
        }
    }
    pub(super) fn prepare_progress(&mut self) {
        if !self.preparing || self.status.is_some() {
            return;
        }
        if self.file_input() {
            self.directory.progress(self.env.now);
        }
        if self.exporting {
            if self.directory.created && self.directory.ready() {
                self.directory.start(self.class);
                self.preparing = false;
            }
            return;
        }
        let Some(bytes) = self.directory.link.take() else { return };
        match self.call.take().expect("input's call") {
            Parsed::File { file, json } => match jig::json::call(TOOLS, &bytes, file, &self.root_env.limits.tools) {
                Ok(call) => {
                    let specs = TOOLS[usize::from(call.tool)].forms[usize::from(call.form)];
                    if specs.iter().zip(call.values.iter()).any(|(spec, value)| {
                        spec.kind == Kind::File && matches!(value, Some(Values::One(Value::File(_) | Value::Stdin)))
                    }) {
                        self.fail(temper_client_tools::table::USAGE);
                        return;
                    }
                    self.class = arguments::class(&call);
                    self.directory.start(self.class);
                    self.call = Some(Parsed::Call { call, json });
                    self.prepare_start();
                }
                Err(
                    why @ (jig::argv::Usage::Timeout
                    | jig::argv::Usage::WaitPart { .. }
                    | jig::argv::Usage::TerminalPhase { .. }),
                ) => {
                    let mut measured = skein_lib::Writer::new(1024);
                    temper_client_tools::table::refusal(why, &[], &mut measured).expect("refusal");
                    let mut exact = skein_lib::Writer::new(measured.written());
                    temper_client_tools::table::refusal(why, &[], &mut exact).expect("refusal");
                    self.fail(&exact.finish());
                }
                Err(jig::argv::Usage::AmendmentRevision) => self.fail(b"task amend needs --rev REV"),
                Err(_) => self.fail(temper_client_tools::table::USAGE),
            },
            Parsed::Call { call, json } if arguments::key_file(&call).is_some() => {
                let stripped = if call.tool == temper_client_tools::jig::rows::IMPORT_AT {
                    bytes.as_ref()
                } else {
                    bytes.strip_suffix(b"\n").unwrap_or(&bytes)
                };
                if stripped.is_empty() || stripped.len() > arguments::key_bytes(&call) as usize {
                    self.key_file_fail(
                        &call,
                        if call.tool == temper_client_tools::jig::rows::IMPORT_AT {
                            b"record file must contain 1 to 131072 bytes"
                        } else if arguments::key_bytes(&call) == 4089 {
                            b"key file must contain 1 to 4089 bytes after one final newline"
                        } else {
                            b"key file must contain 1 to 4096 bytes after one final newline"
                        },
                    );
                    return;
                }
                if call.tool == temper_client_tools::jig::rows::IMPORT_AT
                    && !stripped.starts_with(b"JIGM")
                    && stripped.len() >= 2
                {
                    // The generated form begins with its u16 version; the temporary
                    // JIGM form goes whole to jig's decoder (decision 1345).
                    const READS: u16 = 1;
                    let version = u16::from_be_bytes([stripped[0], stripped[1]]);
                    let reads = READS;
                    if version > reads {
                        let mut line = self.move_file.as_deref().expect("import file").to_vec();
                        line.extend_from_slice(format!(" holds a moved record of form v{version}; this temper reads up to v{reads}: upgrade temper").as_bytes());
                        self.fail(&line);
                        return;
                    }
                }
                self.value = Some(skein_lib::Secret::new(stripped.into()));
                self.directory.start(self.class);
                self.call = Some(Parsed::Call { call, json });
                self.preparing = false;
            }
            Parsed::Call { mut call, json } => {
                let file = text_file_index(&call);
                let specs = TOOLS[usize::from(call.tool)].forms[usize::from(call.form)];
                for (index, (spec, slot)) in specs.iter().zip(call.values.iter_mut()).enumerate() {
                    if file == Some(index) || (file.is_none() && *slot == Some(Values::One(Value::Stdin))) {
                        let text =
                            if spec.kind == Kind::File { bytes.strip_suffix(b"\n").unwrap_or(&bytes) } else { &bytes };
                        *slot = Some(Values::One(Value::Text(text.into())));
                    }
                }
                self.directory.start(self.class);
                self.call = Some(Parsed::Call { call, json });
                self.prepare_start();
            }
        }
    }
    pub(super) fn key_file_fail(&mut self, call: &jig::call::Call, reason: &[u8]) {
        if call.tool == temper_client_tools::jig::rows::IMPORT_AT {
            let mut message = b"record file".to_vec();
            message.extend_from_slice(
                reason.strip_prefix(b"key file").unwrap_or(reason.strip_prefix(b"record file").unwrap_or(reason)),
            );
            self.fail(&message);
        } else if TOOLS[usize::from(call.tool)].name == b"forge connect" {
            let mut message = reason.to_vec();
            message.extend_from_slice(b": ");
            message.extend_from_slice(arguments::key_file(call).expect("key file"));
            self.fail(&message);
        } else {
            self.fail(reason);
        }
    }
    pub(super) fn preparation_pending(&self, now: Time) -> bool {
        self.preparing && self.file_input() && self.directory.pending(now)
    }
    fn file_input(&self) -> bool {
        match &self.call {
            Some(Parsed::File { file: Source::Path(_), .. }) => true,
            Some(Parsed::Call { call, .. }) => {
                call.tool == temper_client_tools::jig::rows::EXPORT_AT
                    || arguments::key_file(call).is_some()
                    || text_file(call).is_some()
            }
            _ => false,
        }
    }
    fn input_limit(&self) -> u32 {
        if self.class == Class::SignIn && !matches!(self.call, Some(Parsed::File { .. })) {
            limits::LINK
        } else {
            u32::try_from(self.root_env.limits.tools.ask_bytes).expect("input bound")
        }
    }
    pub(super) fn read_input(&mut self) {
        self.request(io::Request::Stream {
            stream: self.stdin.expect("stdin"),
            down: Down::Demand { read: Read::Arrived { max: self.input_limit() + 1 }, room: 0 },
        });
    }
    pub(super) fn input_event(&mut self, up: Up) {
        match up {
            Up::Bytes(bytes) => {
                let limit = self.input_limit() as usize;
                let input = self.input.as_mut().expect("reading input");
                if input.len() + bytes.len() > limit {
                    if let Some(Parsed::Call { call, .. }) = &self.call
                        && matches!(call.tool, jig::rows::CHANGE_SETTINGS_AT | jig::rows::PROPOSE_SETTINGS_AT)
                    {
                        self.fail(format!("settings file: file bytes: bound {}", self.input_limit()).as_bytes());
                    } else {
                        self.fail(b"input is too large");
                    }
                    return;
                }
                input.extend_from_slice(&bytes);
                self.read_input();
            }
            Up::End => self.directory.link = self.input.take().map(Vec::into_boxed_slice),
            Up::Failed(_) => self.fail(b"input could not be read"),
            Up::Room => unreachable!("stdin never writes"),
        }
    }
}

fn text_file_index(call: &jig::call::Call) -> Option<usize> {
    let (index, _) = text_file(call)?;
    Some(usize::from(index))
}
fn text_file(call: &jig::call::Call) -> Option<(u8, &[u8])> {
    let name = TOOLS[usize::from(call.tool)].name;
    if name != b"task amend" && name != b"settings apply" && name != b"settings propose" {
        return None;
    }
    for (index, slot) in call.values.iter().enumerate() {
        if let Some(Values::One(Value::File(path))) = slot {
            return Some((u8::try_from(index).expect("argument index"), path));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::fixtures::Frame;
    use skein_io::kernel::Fd;

    const WORDS: &[u8] = b"Run the checks\n\n";
    fn amendment(words: &[&str]) -> Frame {
        let mut frame = Frame::read(words);
        // Stop at DIRECTORY after input preparation; no engine is needed to inspect it.
        frame.absent = true;
        frame
    }
    fn text(frame: &Frame, index: usize) -> &[u8] {
        let Some(Parsed::Call { call, .. }) = &frame.host.call else { panic!("prepared call") };
        let Some(Some(Values::One(Value::Text(bytes)))) = call.values.get(index) else { panic!("loaded text") };
        bytes
    }
    #[test]
    fn amend_reads_instructions_from_a_file() {
        let mut frame =
            amendment(&["task", "amend", "41", "--rev", "7", "--instructions", "/instructions/link-file", "because"]);
        frame.link = WORDS;
        frame.drain();
        assert_eq!(text(&frame, 6), b"Run the checks\n");
        assert_eq!(frame.link_root, Some(Fd::new(36)));
    }
    #[test]
    fn amend_reads_instructions_from_stdin() {
        let mut frame = amendment(&["task", "amend", "41", "--rev", "7", "--instructions", "-", "because"]);
        frame.input = WORDS;
        frame.drain();
        assert_eq!(text(&frame, 6), b"Run the checks\n");
    }
    #[test]
    fn amend_sends_empty_instructions() {
        let mut frame =
            amendment(&["task", "amend", "41", "--rev", "7", "--instructions", "/instructions/link-file", "because"]);
        frame.link = b"\n";
        frame.drain();
        assert_eq!(text(&frame, 6), b"");
    }
    #[test]
    fn amend_loads_both_files_from_their_own_parents() {
        let mut frame = amendment(&[
            "task",
            "amend",
            "41",
            "--rev",
            "7",
            "--instructions",
            "/instructions/link-file",
            "--spec",
            "/spec/link",
            "because",
        ]);
        frame.link = WORDS;
        frame.drain();
        assert_eq!(text(&frame, 6), b"Run the checks\n");
        assert_eq!(text(&frame, 7), b"Run the checks\n");
        assert_eq!(frame.link_root, Some(Fd::new(37)));
        assert_eq!(frame.link_leaf.as_deref(), Some(b"link".as_slice()));
    }
    #[test]
    fn amend_loads_a_file_and_stdin_before_directory() {
        let mut frame = amendment(&[
            "task",
            "amend",
            "41",
            "--rev",
            "7",
            "--instructions",
            "/instructions/link-file",
            "--spec",
            "-",
            "because",
        ]);
        frame.link = WORDS;
        frame.input = b"Build\n";
        frame.drain();
        assert_eq!(text(&frame, 6), b"Run the checks\n");
        assert_eq!(text(&frame, 7), b"Build");
    }
    #[test]
    fn amend_call_carries_both_texts_inline() {
        let mut frame = amendment(&["--call", "call.json"]);
        frame.link = br#"{"tool":"task amend","arguments":{"task":41,"rev":7,"instructions":"Run\n","spec":"Build","reason":"because"}}"#;
        frame.drain();
        assert_eq!(text(&frame, 6), b"Run\n");
        assert_eq!(text(&frame, 7), b"Build");
        assert_eq!(frame.link_leaf.as_deref(), Some(b"call.json".as_slice()));
    }
    #[test]
    fn a_json_amend_without_revision_has_the_specific_usage_error() {
        let mut frame = amendment(&["--call", "call.json"]);
        frame.link = br#"{"tool":"task amend","arguments":{"task":41,"instructions":"Run","reason":"because"}}"#;
        frame.drain();
        assert_eq!(frame.stderr(), b"temper ctl: task amend needs --rev REV\n");
        assert!(!frame.opened(b"directory"));
    }
    #[test]
    fn the_old_spec_revision_form_asks_for_rev() {
        assert_eq!(
            crate::arguments::parse(&[
                b"task",
                b"amend",
                b"41",
                b"--spec",
                b"8",
                b"--model",
                b"chosen-model",
                b"because"
            ]),
            Err(jig::argv::Usage::AmendmentRevision)
        );
    }
}

#[cfg(test)]
mod settings_input_tests {
    use super::*;
    use crate::tests::fixtures::Frame;
    const ITEMS: &[u8] = br#"{"items":[{"setting":{"kind":"rules"},"edit":"reset"}]}"#;
    fn loaded(frame: &Frame) -> &[u8] {
        let Some(Parsed::Call { call, .. }) = &frame.host.call else { panic!("prepared call") };
        let Some(Some(Values::One(Value::Text(bytes)))) = call.values.first() else { panic!("loaded items") };
        bytes
    }
    #[test]
    fn settings_acts_read_their_file_before_directory() {
        for tool in ["apply", "propose"] {
            let mut frame =
                Frame::read(&["settings", tool, "/settings/link-file", "--against", "5", "--key", "change"]);
            frame.absent = true;
            frame.link = ITEMS;
            frame.drain();
            assert_eq!(loaded(&frame), ITEMS);
        }
    }
    #[test]
    fn settings_acts_read_their_items_from_stdin() {
        for tool in ["apply", "propose"] {
            let mut frame = Frame::read(&["settings", tool, "-", "--against", "5", "--key", "change"]);
            frame.absent = true;
            frame.input = ITEMS;
            frame.drain();
            assert_eq!(loaded(&frame), ITEMS);
        }
    }
    #[test]
    fn settings_acts_name_their_file_byte_bound_for_files_and_stdin() {
        static OVERSIZED: [u8; 1_048_577] = [b' '; 1_048_577];
        for (tool, path) in
            ["apply", "propose"].into_iter().flat_map(|tool| ["-", "/settings/link-file"].map(|path| (tool, path)))
        {
            let mut frame = Frame::read(&["settings", tool, path, "--against", "5", "--key", "change"]);
            frame.absent = true;
            frame.input = &OVERSIZED;
            frame.link = &OVERSIZED;
            frame.drain();
            assert!(frame.host.status.is_some());
            assert!(
                frame
                    .stderr()
                    .windows(b"settings file: file bytes: bound 1048576".len())
                    .any(|window| window == b"settings file: file bytes: bound 1048576")
            );
        }
    }
}

#[cfg(test)]
mod wait_input_tests {
    use crate::tests::fixtures::Frame;
    #[test]
    fn invalid_json_waits_print_the_specific_refusal_before_directory() {
        for (json, expected) in [
            (
                br#"{"tool":"me","arguments":{"timeout":5}}"#.as_slice(),
                b"temper ctl: --timeout needs --since or --until\n".as_slice(),
            ),
            (
                br#"{"tool":"task show","arguments":{"task":1,"tree":true,"until":"ended"}}"#,
                b"temper ctl: task show waits on its subject alone: drop --tree\n",
            ),
        ] {
            let mut frame = Frame::read(&["--call", "call.json"]);
            frame.link = json;
            frame.drain();
            assert_eq!(frame.stderr(), expected);
            assert!(!frame.opened(b"directory"));
            assert_eq!(frame.requests(), 0);
        }
    }
}
