//! Store writes stay serialized while replayed requests run beside a new act (client/tools.md, 7).
use jig_client::{Event, Request};
use jig_client_shell_native::Shell;
use skein_lib::{Queue, Time};

pub(crate) struct Recovery {
    pub(crate) loaded: bool,
    pub(crate) ready: bool,
    pub(crate) sending: u32,
    pub(crate) printed: Option<temper_client_tools::jig::Printed>,
    writing: bool,
    writes: Queue<Request>,
}
impl Recovery {
    pub(crate) fn new(room: u32) -> Self {
        Self {
            loaded: false,
            ready: false,
            sending: 0,
            printed: None,
            writing: false,
            writes: Queue::with_capacity(room),
        }
    }
    pub(crate) fn event(&mut self, event: &Event) -> bool {
        match event {
            Event::Stored => self.writing = false,
            Event::NotStored => return false,
            Event::Start { .. }
            | Event::StartUnlinked { .. }
            | Event::SignIn { .. }
            | Event::SignOut
            | Event::SignOutLost
            | Event::Watch { .. }
            | Event::Unwatch { .. }
            | Event::Read { .. }
            | Event::Send { .. }
            | Event::Carry { .. }
            | Event::Authorize { .. }
            | Event::Reply { .. }
            | Event::Lost { .. }
            | Event::Streamed { .. }
            | Event::StreamGone
            | Event::Up { .. } => {}
        }
        true
    }
    pub(crate) fn down(&mut self, shell: &mut Shell, request: Request) {
        if matches!(request, Request::Save { .. } | Request::Erase { .. }) {
            self.writes.push(request);
        } else {
            shell.down(request);
        }
        self.pump(shell);
    }
    pub(crate) fn pump(&mut self, shell: &mut Shell) {
        if !self.writing
            && let Some(request) = self.writes.pop()
        {
            self.writing = true;
            shell.down(request);
        }
    }
    pub(crate) fn admits(&self) -> bool {
        self.ready && !self.writing && self.writes.is_empty()
    }
    pub(crate) fn unreadable(&self, opening: bool, shell: &Shell, now: Time) -> bool {
        !self.loaded && !opening && !shell.work_pending(now) && shell.next_deadline().is_none()
    }
}
