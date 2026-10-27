//! The box's ordered commands; each hides whether it is once, long or link.
use crate::{
    check::joined,
    setup::{Mode, Setup},
};
use alloc::boxed::Box;
use skein_lib::{Decimal, List, bytes};
/// The word carried by a group's step lines (protocol/setup.md, 7.3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    Fetch,
    Prepare,
    Run,
}
impl Stage {
    #[must_use]
    pub fn word(self) -> &'static [u8] {
        match self {
            Self::Fetch => b"fetch",
            Self::Prepare => b"prepare",
            Self::Run => b"run",
        }
    }
}
/// A dormant forge's steps and their place in the always chain.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Group {
    pub forge: u16,
    /// Owned opaque bytes the box uses to name the group on the terminal (protocol/setup.md, 7.3).
    pub label: Box<[u8]>,
    /// `recorded` whether its `started` writes the box's record: an owned forge's does, the demo's does not.
    pub recorded: bool,
    pub before: u32,
    pub steps: Box<[(Stage, Step)]>,
}
/// Commands run at startup, and groups run when asked.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Steps {
    pub always: Box<[Step]>,
    pub asked: Box<[Group]>,
}
/// How the box runs a step (protocol/setup.md, 7.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Once,
    Long,
    Link,
}
/// One command of the temper binary that the box runs.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Step {
    name: Box<[u8]>,
    arguments: Box<[Box<[u8]>]>,
    kind: Kind,
    ready: Option<core::net::SocketAddr>,
    listeners: Box<[core::net::SocketAddr]>,
}
impl Step {
    /// Constructs a step from its display name, arguments and lifetime.
    #[must_use]
    pub fn new(name: Box<[u8]>, arguments: Box<[Box<[u8]>]>, kind: Kind) -> Self {
        Self { name, arguments, kind, ready: None, listeners: Box::new([]) }
    }
    /// The listening sockets given to this step, in descriptor order from fd 3.
    /// Port zero stays the step's own bind: its kernel-chosen address cannot
    /// be claimed by a `Listen` naming port zero (protocol/setup.md, 7.1).
    #[must_use]
    pub fn listening(mut self, addrs: Box<[core::net::SocketAddr]>) -> Self {
        let mut listeners = List::with_capacity(u32::try_from(addrs.len()).expect("bounded listeners"));
        for address in addrs {
            if address.port() != 0 {
                listeners.push(address).expect("listener capacity");
            }
        }
        self.listeners = listeners.into_boxed();
        self
    }
    #[must_use]
    pub fn listeners(&self) -> &[core::net::SocketAddr] {
        &self.listeners
    }
    /// Wait for this owned process to answer its version route before continuing.
    #[must_use]
    pub fn with_ready(mut self, address: core::net::SocketAddr) -> Self {
        self.ready = Some(address);
        self
    }
    /// The loopback endpoint whose version proves the process ready.
    #[must_use]
    pub fn ready(&self) -> Option<core::net::SocketAddr> {
        self.ready
    }
    /// How the box runs this step.
    #[must_use]
    pub fn kind(&self) -> Kind {
        self.kind
    }
    /// The step's name for prefixed output.
    #[must_use]
    pub fn name(&self) -> &[u8] {
        &self.name
    }
    /// Arguments passed to temper.
    #[must_use]
    pub fn arguments(&self) -> &[Box<[u8]>] {
        &self.arguments
    }
}
fn forge(action: &[u8], flags: &[Box<[u8]>], number: u16, run: Option<&[u8]>) -> Step {
    let digits = Decimal::of(u64::from(number));
    let n = digits.as_bytes();
    let mut args = List::with_capacity(
        u32::try_from(flags.len()).expect("bounded flags").checked_add(4).expect("bounded arguments"),
    );
    for a in [b"forge".as_slice(), action] {
        args.push(bytes::copy_of(a)).expect("command words");
    }
    for flag in flags {
        args.push(flag.clone()).expect("flags counted");
    }
    args.push(bytes::copy_of(n)).expect("forge number");
    let name = match run {
        Some(which) => {
            args.push(bytes::copy_of(which)).expect("run argument");
            joined(&[which, b" ", n])
        }
        None => joined(&[action, b" ", n]),
    };
    Step::new(name, args.into_boxed(), if run.is_some() { Kind::Long } else { Kind::Once })
}
/// Carries the box's absolute directory flags after each command's words.
/// Commands in setup order: owned processes, legacy reconciles, then serve;
/// an asked forge's processes apart, in a group placed in that order.
#[must_use]
pub fn steps(
    setup: &Setup,
    flags: &[Box<[u8]>],
    listeners: Box<[core::net::SocketAddr]>,
    forgejo_listens: bool,
) -> Steps {
    let count = u32::try_from(setup.entries.len()).expect("bounded entries");
    let mut out = List::with_capacity(
        count.checked_mul(5).expect("bounded step count").checked_add(1).expect("bounded step count"),
    );
    let mut asked = List::with_capacity(count);
    for e in &setup.entries {
        match &e.mode {
            Mode::Managed { .. } | Mode::Fake(_) => {}
            Mode::Owned(o) => {
                let mut group = List::with_capacity(5);
                if o.fetch.forgejo || o.fetch.runner {
                    group.push((Stage::Fetch, forge(b"fetch", flags, e.forge, None))).expect("fetch");
                }
                group.push((Stage::Prepare, forge(b"prepare", flags, e.forge, None))).expect("prepare");
                // forge run absorbs Forgejo's KillParent (protocol/setup.md, 6.4, 7.1).
                let mut step = forge(b"run", flags, e.forge, Some(b"forgejo")).with_ready(o.listen);
                if forgejo_listens {
                    step = step.listening(Box::new([o.listen]));
                }
                group.push((Stage::Run, step)).expect("forgejo");
                if o.runner.is_some() {
                    group.push((Stage::Run, forge(b"run", flags, e.forge, Some(b"runner")))).expect("runner");
                }
                if e.asked {
                    let digits = Decimal::of(u64::from(e.forge));
                    asked
                        .push(Group {
                            forge: e.forge,
                            label: joined(&[b"Forge ", digits.as_bytes(), b" (Forgejo)"]),
                            recorded: true,
                            before: out.len(),
                            steps: group.into_boxed(),
                        })
                        .expect("entry capacity");
                } else {
                    for (_, step) in group.into_boxed() {
                        out.push(step).expect("step capacity");
                    }
                }
            }
        }
    }
    for e in &setup.entries {
        // Local forges reconcile in serve after taking their handover.
        let reconcile = match &e.mode {
            Mode::Owned(_) | Mode::Fake(_) | Mode::Managed { admin_token: None } => false,
            Mode::Managed { admin_token: Some(_) } => true,
        };
        if !e.asked && reconcile {
            out.push(forge(b"reconcile", flags, e.forge, None)).expect("step capacity");
        }
    }
    let mut serve = List::with_capacity(
        u32::try_from(flags.len()).expect("bounded flags").checked_add(3).expect("bounded arguments"),
    );
    serve.push(bytes::copy_of(b"internal")).expect("command word");
    serve.push(bytes::copy_of(b"serve")).expect("command word");
    for flag in flags {
        serve.push(flag.clone()).expect("flags counted");
    }
    serve.push(bytes::copy_of(b"--box")).expect("box flag");
    out.push(Step::new(bytes::copy_of(b"serve"), serve.into_boxed(), Kind::Long).listening(listeners))
        .expect("serve capacity");
    Steps { always: out.into_boxed(), asked: asked.into_boxed() }
}
