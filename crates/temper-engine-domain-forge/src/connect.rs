//! Volatile public connects; only handles cross this boundary (client/README.md, 8).
use crate::{AcceptAnswer, Answered, Ask, ConnectAnswer, Connection, Domain, ForgeRefusal, InstallAnswer, Request};
use alloc::boxed::Box;
use jig_domain_core::Carried;
use jig_domain_people::Role;
use skein_lib::{Map, Queue, Token};

#[derive(Debug)]
pub(crate) struct State {
    pub(crate) answers: Map<Token, Answered>,
    pending: Map<Token, Token>,
    running: Map<u16, (Token, Option<Carried>)>,
    reading: Map<u16, Token>,
    previous: Map<u16, (Connection, Option<crate::FailedStep>)>,
}
impl State {
    pub(crate) fn new(bound: u32) -> Self {
        Self {
            answers: Map::with_capacity(bound),
            pending: Map::with_capacity(bound),
            running: Map::with_capacity(bound),
            reading: Map::with_capacity(bound),
            previous: Map::with_capacity(bound),
        }
    }
    fn answer(&mut self, request: Token, token: Token, answer: Answered, out: &mut Queue<Request>) {
        if self.answers.len() == self.answers.capacity() && !self.answers.contains_key(&token) {
            let oldest = *self.answers.first().expect("full answers").0;
            self.answers.remove(&oldest);
        }
        self.answers.insert(token, answer).expect("bounded answers");
        out.push(Request::ApplicationDone { request, token });
    }
}
pub(crate) fn worst_case(bound: u32) -> Option<u64> {
    Map::<Token, Answered>::worst_case(bound)?
        .checked_add(Map::<Token, Token>::worst_case(bound)?)?
        .checked_add(Map::<u16, (Token, Option<Carried>)>::worst_case(bound)?)?
        .checked_add(Map::<u16, Token>::worst_case(bound)?)?
        .checked_add(Map::<u16, (Connection, Option<crate::FailedStep>)>::worst_case(bound)?)?
        .checked_add(u64::from(bound).checked_mul(crate::STOPPED_TEXT_BYTES)?)?
        .checked_add(u64::from(bound).checked_mul(
            2048_u64.max(100_u64.checked_mul(u64::try_from(size_of::<crate::Reached>()).ok()?.checked_add(2048)?)?),
        )?)
}
pub(crate) fn start(
    d: &mut Domain,
    request: Token,
    token: Token,
    ask: Ask,
    role: Option<Role>,
    carried: Option<Carried>,
    out: &mut Queue<Request>,
) {
    if let Ask::Reachable { forge, installation, page } = ask {
        reachable(d, request, token, forge, installation, page, role, carried, out);
        return;
    }
    if let Ask::Connect { forge, how: how @ crate::How::ForgejoAdmin { .. } } = ask {
        admin(d, request, token, forge, how, role, carried, out);
        return;
    }
    let (forge, install, manifest) = match &ask {
        Ask::Connect { forge, how: crate::How::GitHubApp { .. } } => (*forge, false, false),
        Ask::Connect { forge, how: crate::How::GitHubManifest { .. } } => (*forge, false, true),
        Ask::Install { forge, .. } | Ask::Accept { forge, .. } => (*forge, true, false),
        Ask::Connect { how: crate::How::ForgejoAdmin { .. }, .. }
        | Ask::Reachable { .. }
        | Ask::Adopt(_)
        | Ask::SetUp { .. }
        | Ask::StartDemo => unreachable!("connect admission"),
    };
    let (connected, connecting) = match d.connection(forge) {
        Connection::Connected { .. } => (true, false),
        Connection::Connecting => (false, true),
        Connection::NotConnected
        | Connection::NotSetUp
        | Connection::SettingUp { .. }
        | Connection::SetUpFailed { .. }
        | Connection::Stopped { .. } => (false, false),
    };
    let refusal = if role != Some(Role::Owner) {
        Some(ForgeRefusal::Role)
    } else if !d.configured.contains_key(&forge) {
        Some(ForgeRefusal::NoSuchForge)
    } else if let Some(config) = d.configured.get(&forge)
        && (config.kind != crate::Kind::GitHub || config.mode == crate::Mode::Fake || manifest && config.read_only)
    {
        Some(ForgeRefusal::NotTaken)
    } else if d.connect.running.contains_key(&forge)
        || d.connect.pending.len() == d.connect.pending.capacity()
        || d.connect.running.len() == d.connect.running.capacity()
    {
        Some(ForgeRefusal::Busy)
    } else if install && !connected {
        Some(ForgeRefusal::NotConnected)
    } else if !install && (connected || connecting) {
        Some(ForgeRefusal::Connected)
    } else if !install && !manifest && carried.is_none() {
        Some(ForgeRefusal::NoKey)
    } else {
        None
    };
    if let Some(why) = refusal {
        if let Some(value) = carried {
            out.push(Request::CarriedUnused { number: value.number });
        }
        let answer = if let Ask::Accept { .. } = ask {
            Answered::Accept(AcceptAnswer::Refused(why))
        } else if install {
            Answered::Install(InstallAnswer::Refused(why))
        } else {
            Answered::Connect(ConnectAnswer::Refused(why))
        };
        d.connect.answer(request, token, answer, out);
        return;
    }
    if d.ended.remove(&forge).is_some() {
        out.push(Request::MarkedForges);
    }
    d.connect.pending.insert(request, token).expect("room checked");
    send(d, request, forge, ask, carried, out);
}
fn send(d: &mut Domain, request: Token, forge: u16, ask: Ask, carried: Option<Carried>, out: &mut Queue<Request>) {
    match ask {
        Ask::Connect { how: how @ crate::How::GitHubApp { .. }, .. } => {
            let carried = carried.expect("admitted key");
            d.connect.running.insert(forge, (request, Some(carried))).expect("room checked");
            let kept = match d.connections.get(&forge) {
                Some((_, kept, _)) => *kept,
                None => None,
            };
            d.connections.insert(forge, (Connection::Connecting, kept, None)).expect("configured forge");
            out.push(Request::MarkedForges);
            out.push(Request::Connect { forge, request, how, carried });
        }
        Ask::Connect { how: crate::How::GitHubManifest { back }, .. } => {
            if let Some(value) = carried {
                out.push(Request::CarriedUnused { number: value.number });
            }
            d.connect.running.insert(forge, (request, None)).expect("room checked");
            out.push(Request::Manifest { forge, request, back });
        }
        Ask::Accept { installation, .. } => {
            if let Some(value) = carried {
                out.push(Request::CarriedUnused { number: value.number });
            }
            d.connect.running.insert(forge, (request, None)).expect("room checked");
            out.push(Request::Accept { forge, request, installation });
        }
        Ask::Install { back, .. } => {
            if let Some(value) = carried {
                out.push(Request::CarriedUnused { number: value.number });
            }
            d.connect.running.insert(forge, (request, None)).expect("room checked");
            out.push(Request::Install { forge, request, back });
        }
        Ask::Connect { how: crate::How::ForgejoAdmin { .. }, .. }
        | Ask::Reachable { .. }
        | Ask::Adopt(_)
        | Ask::SetUp { .. }
        | Ask::StartDemo => unreachable!("connect admission"),
    }
}
/// Admission for the string-managed Forgejo form (client/README.md, 8).
#[expect(clippy::too_many_arguments, reason = "the admitted request and its caller travel together")]
fn admin(
    d: &mut Domain,
    request: Token,
    token: Token,
    forge: u16,
    how: crate::How,
    role: Option<Role>,
    carried: Option<Carried>,
    out: &mut Queue<Request>,
) {
    let previous = d.connection(forge);
    let config = d.configured.get(&forge);
    let occupied = match &previous {
        Connection::Connected { .. } | Connection::Connecting | Connection::SettingUp { .. } => true,
        Connection::NotConnected
        | Connection::NotSetUp
        | Connection::SetUpFailed { .. }
        | Connection::Stopped { .. } => false,
    };
    let refusal = if role != Some(Role::Owner) {
        Some(ForgeRefusal::Role)
    } else if match config {
        Some(config) => {
            config.kind != crate::Kind::Forgejo || config.mode != crate::Mode::Managed || !config.admin_connect
        }
        None => true,
    } {
        Some(ForgeRefusal::NotTaken)
    } else if occupied {
        Some(ForgeRefusal::Connected)
    } else if d.connect.running.contains_key(&forge)
        || d.connect.pending.len() == d.connect.pending.capacity()
        || d.connect.running.len() == d.connect.running.capacity()
    {
        Some(ForgeRefusal::Busy)
    } else if carried.is_none() {
        Some(ForgeRefusal::NoKey)
    } else if let crate::How::ForgejoAdmin { first: Some(first) } = &how
        && first.org.eq_ignore_ascii_case(&config.expect("admitted configured forge").bot)
    {
        Some(ForgeRefusal::FirstIsBot)
    } else {
        None
    };
    if let Some(why) = refusal {
        if let Some(value) = carried {
            out.push(Request::CarriedUnused { number: value.number });
        }
        d.connect.answer(request, token, Answered::Connect(ConnectAnswer::Refused(why)), out);
        return;
    }
    let carried = carried.expect("admitted admin token");
    d.ended.remove(&forge);
    d.connect.pending.insert(request, token).expect("room checked");
    d.connect.running.insert(forge, (request, Some(carried))).expect("room checked");
    let (kept, failed) = match d.connections.get(&forge) {
        Some((_, kept, failed)) => (*kept, *failed),
        None => (None, None),
    };
    d.connect.previous.insert(forge, (previous, failed)).expect("one previous connection per running connect");
    d.connections.insert(forge, (Connection::Connecting, kept, None)).expect("configured forge");
    if let crate::How::ForgejoAdmin { first } = &how {
        d.first.remove(&forge);
        if let Some(first) = first {
            d.first
                .insert(
                    forge,
                    crate::First {
                        playground: false,
                        org: first.org.clone(),
                        repository: first.repository.clone(),
                        project: first.project.clone(),
                        state: crate::FirstState::Making,
                        label: first.label.clone(),
                    },
                )
                .expect("configured forges bound first repositories");
        }
    }
    out.push(Request::MarkedForges);
    out.push(Request::Connect { forge, request, how, carried });
}
pub(crate) fn connected(d: &mut Domain, forge: u16, request: Token, answer: ConnectAnswer, out: &mut Queue<Request>) {
    if let Some(token) = d.connect.pending.remove(&request) {
        if let ConnectAnswer::Link(_) = &answer
            && let Some((held, _)) = d.connect.running.get(&forge)
            && *held == request
        {
            d.connect.running.remove(&forge);
        }
        if let ConnectAnswer::Refused(_) = answer
            && let Some((held, _)) = d.connect.running.get(&forge)
            && *held == request
        {
            if let Some((_, Some(carried))) = d.connect.running.remove(&forge) {
                out.push(Request::CarriedUnused { number: carried.number });
            }
            let kept = match d.connections.get(&forge) {
                Some((_, kept, _)) => *kept,
                None => None,
            };
            let (previous, failed) = d.connect.previous.remove(&forge).unwrap_or((Connection::NotConnected, None));
            d.connections.insert(forge, (previous, kept, failed)).expect("configured forge");
            out.push(Request::MarkedForges);
        }
        d.connect.answer(request, token, Answered::Connect(answer), out);
    }
}
pub(crate) fn installed(
    d: &mut Domain,
    forge: u16,
    request: Token,
    answer: InstallAnswer,
    stopped: Option<Box<[u8]>>,
    out: &mut Queue<Request>,
) {
    if let Some(token) = d.connect.pending.remove(&request) {
        if let Some((held, _)) = d.connect.running.get(&forge)
            && *held == request
        {
            d.connect.running.remove(&forge);
            if let Some(why) = stopped {
                stopped_end(d, forge, &why, None, out);
            }
        }
        d.connect.answer(request, token, Answered::Install(answer), out);
    }
}
pub(crate) fn accepted(
    d: &mut Domain,
    forge: u16,
    request: Token,
    answer: AcceptAnswer,
    stopped: Option<Box<[u8]>>,
    out: &mut Queue<Request>,
) {
    if let Some(token) = d.connect.pending.remove(&request) {
        if let Some((held, _)) = d.connect.running.get(&forge)
            && *held == request
        {
            d.connect.running.remove(&forge);
            if let Some(why) = stopped {
                stopped_end(d, forge, &why, None, out);
            }
        }
        d.connect.answer(request, token, Answered::Accept(answer), out);
    }
}
pub(crate) fn ended(
    d: &mut Domain,
    forge: u16,
    request: Token,
    connected: bool,
    why: Option<Box<[u8]>>,
    stop: Option<crate::AdminStop>,
    out: &mut Queue<Request>,
) {
    if let Some((held, _)) = d.connect.running.get(&forge)
        && *held == request
    {
        if let Some((_, Some(carried))) = d.connect.running.remove(&forge) {
            out.push(Request::EraseCarried { number: carried.number });
        }
        let admin = d.connect.previous.contains_key(&forge);
        if connected {
            d.connect.previous.remove(&forge);
        }
        if connected && !admin {
            d.apps.insert(forge, crate::AppWay::Pasted).expect("configured forge App way");
            out.push(Request::MarkedForges);
        }
        if !connected {
            if let Some(why) = why {
                stopped_end(d, forge, &why, if admin { stop } else { None }, out);
            }
            let kept = match d.connections.get(&forge) {
                Some((_, kept, _)) => *kept,
                None => None,
            };
            let (previous, failed) = d.connect.previous.remove(&forge).unwrap_or((Connection::NotConnected, None));
            d.connections.insert(forge, (previous, kept, failed)).expect("configured forge");
            out.push(Request::MarkedForges);
        }
    }
}

/// Volatile per-forge diagnostics, bounded in bytes without splitting UTF-8 (client/forge.md, 5.4).
fn stopped_end(d: &mut Domain, forge: u16, why: &[u8], stop: Option<crate::AdminStop>, out: &mut Queue<Request>) {
    // The host supplies UTF-8. Back over continuation bytes at the cut;
    // text validation belongs to the protocol boundary.
    let mut end = why.len().min(crate::CONNECT_WHY_BYTES);
    while end > 0 {
        let Some(byte) = why.get(end) else { break };
        if byte & 0xc0 != 0x80 {
            break;
        }
        end = end.saturating_sub(1);
    }
    d.ended
        .insert(forge, (Box::from(why.get(..end).expect("bounded reason prefix")), stop))
        .expect("configured forge ends");
    out.push(Request::MarkedForges);
}

#[expect(clippy::too_many_arguments, reason = "the admitted request and its caller travel together")]
fn reachable(
    d: &mut Domain,
    request: Token,
    token: Token,
    forge: u16,
    installation: Option<u64>,
    page: u32,
    role: Option<Role>,
    carried: Option<Carried>,
    out: &mut Queue<Request>,
) {
    if let Some(value) = carried {
        out.push(Request::CarriedUnused { number: value.number });
    }
    let config = d.configured.get(&forge);
    let mut selected = installation;
    let mut accepted = false;
    if let Some(config) = config {
        match config.kind {
            crate::Kind::GitHub => {
                for ((owner, id), state) in &d.installations {
                    if *owner == forge
                        && !state.stray
                        && !state.suspended
                        && (selected.is_none() || selected == Some(*id))
                    {
                        selected = Some(*id);
                        accepted = true;
                        break;
                    }
                }
            }
            crate::Kind::Forgejo => accepted = installation.is_none(),
        }
    }
    let connected = match d.connection(forge) {
        Connection::Connected { .. } => true,
        Connection::NotConnected
        | Connection::NotSetUp
        | Connection::SettingUp { .. }
        | Connection::SetUpFailed { .. }
        | Connection::Connecting
        | Connection::Stopped { .. } => false,
    };
    let why = if role != Some(Role::Owner) {
        Some(ForgeRefusal::Role)
    } else if config.is_none() {
        Some(ForgeRefusal::NoSuchForge)
    } else if config.expect("configured").mode == crate::Mode::Fake || page == 0 {
        Some(ForgeRefusal::NotTaken)
    } else if d.connect.reading.contains_key(&forge)
        || d.connect.reading.len() == d.connect.reading.capacity()
        || d.connect.pending.len() == d.connect.pending.capacity()
    {
        Some(ForgeRefusal::Busy)
    } else if !connected {
        Some(ForgeRefusal::NotConnected)
    } else if !accepted {
        Some(ForgeRefusal::NotTaken)
    } else {
        None
    };
    if let Some(why) = why {
        d.connect.answer(request, token, Answered::Reachable(crate::ReachableAnswer::Refused(why)), out);
        return;
    }
    d.connect.pending.insert(request, token).expect("room checked");
    d.connect.reading.insert(forge, request).expect("room checked");
    out.push(Request::Reachable { forge, request, installation: selected, page });
}
pub(crate) fn reached(
    d: &mut Domain,
    forge: u16,
    request: Token,
    answer: crate::ReachableAnswer,
    out: &mut Queue<Request>,
) {
    if d.connect.reading.get(&forge) != Some(&request) {
        return;
    }
    d.connect.reading.remove(&forge);
    if let Some(token) = d.connect.pending.remove(&request) {
        d.connect.answer(request, token, Answered::Reachable(answer), out);
    }
}
