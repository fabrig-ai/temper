//! Section 3's names, without task numbers or scripted control flow.

/// The case's people, chats and tasks, named as the draft names them (section 3).
#[derive(Clone, Copy)]
pub struct Member {
    /// "director", "P", "P/i1": a delegate's name is its requester's, a slash, its own.
    pub name: &'static str,
    pub kind: MemberKind,
    /// The member it was made under; `None` for the person's chat.
    pub under: Option<&'static str>,
    pub charter: u32,
    pub model: Model,
    /// The fake LLM's cue for its scripts, and its spec's words.
    pub cue: &'static str,
    pub spec: &'static str,
}
#[derive(Clone, Copy)]
pub enum MemberKind {
    Chat,
    Goal { priority: u32 },
    Delegate,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Model {
    Claude,
    Gpt,
}
impl Model {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Gpt => "gpt",
        }
    }
    /// The configured models' order, also used by the delegation tool.
    #[must_use]
    pub const fn number(self) -> u32 {
        match self {
            Self::Claude => 0,
            Self::Gpt => 1,
        }
    }
}

/// One repository of the project, its home or not (section 3's kit and app).
#[derive(Clone, Copy)]
pub struct Repository {
    pub owner: &'static str,
    pub name: &'static str,
    pub home: bool,
    pub tree: &'static [(&'static str, &'static str)],
}

/// The project's landing configuration: how it lands, and each repository's gates.
#[derive(Clone, Copy)]
pub struct Landing {
    pub by: LandingBy,
    pub gates: &'static [Gate],
}
#[derive(Clone, Copy)]
pub enum LandingBy {
    Queue,
    Turn,
}
pub struct Gate {
    pub repository: &'static str,
    pub name: &'static str,
}

/// The case's start, as data.
#[derive(Clone, Copy)]
pub struct Cast {
    pub members: &'static [Member],
    pub repositories: &'static [Repository],
    pub landing: Landing,
}
impl Cast {
    /// Fresh-deployment task numbers follow the cast's creation order.
    #[must_use]
    pub fn number(&self, name: &str) -> u64 {
        for (at, member) in self.members.iter().enumerate() {
            if member.name == name {
                return u64::try_from(at).expect("cast length") + 1;
            }
        }
        panic!("member absent from cast: {name}");
    }

    #[must_use]
    pub fn descendants(&self, name: &str) -> u32 {
        let mut count = 0;
        for member in self.members {
            if member.under == Some(name) {
                count += 1 + self.descendants(member.name);
            }
        }
        count
    }

    #[must_use]
    pub fn depth(&self, name: &str) -> u32 {
        let mut depth = 0;
        for member in self.members {
            if member.under == Some(name) {
                depth = depth.max(1 + self.depth(member.name));
            }
        }
        depth
    }

    /// A run and all the work delegated beneath it, from one fixed purse.
    #[must_use]
    pub fn spend(&self, name: &str) -> u64 {
        let mut spend = super::RUN_SPEND;
        for member in self.members {
            if member.under == Some(name) {
                spend += self.spend(member.name);
            }
        }
        spend
    }
}

/// Shared by the director and the goal coordinators.
pub const COORDINATORS_CHARTER: u32 = 1;
pub const COORDINATORS: &str = "Coordinate the work your requester entrusted to you. Delegate its parts within your authority, follow your delegates, and bring decisions outside your authority to your requester.";

pub const DIRECTOR: Member = Member {
    name: "director",
    kind: MemberKind::Chat,
    under: None,
    charter: COORDINATORS_CHARTER,
    model: Model::Claude,
    cue: "@case2_director",
    spec: "@case2_director Coordinate the project; keep Themes running under P.",
};
pub const P: Member = Member {
    name: "P",
    kind: MemberKind::Goal { priority: 2 },
    under: Some(DIRECTOR.name),
    charter: COORDINATORS_CHARTER,
    model: Model::Claude,
    cue: "@case2_p",
    spec: "@case2_p Own Themes in app; delegate its two implementers and follow their work.",
};
pub const P_I1: Member = Member {
    name: "P/i1",
    kind: MemberKind::Delegate,
    under: Some(P.name),
    charter: 2,
    model: Model::Gpt,
    cue: "@case2_i1",
    spec: "@case2_i1 Implement the first part of Themes in app.",
};
pub const P_I2: Member = Member {
    name: "P/i2",
    kind: MemberKind::Delegate,
    under: Some(P.name),
    charter: 2,
    model: Model::Gpt,
    cue: "@case2_i2",
    spec: "@case2_i2 Implement the second part of Themes in app.",
};
pub const APP: Repository =
    Repository { owner: "acme", name: "app", home: true, tree: &[("README.md", "# app\n\nThemes and Export.\n")] };
pub const CAST: Cast = Cast {
    members: &[DIRECTOR, P, P_I1, P_I2],
    repositories: &[APP],
    landing: Landing { by: LandingBy::Queue, gates: &[] },
};
