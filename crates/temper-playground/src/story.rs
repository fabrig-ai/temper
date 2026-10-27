//! Compiled story documents, data alone (playground.md, 5.1 and 5.2).
use std::net::SocketAddr;

/// A story (playground.md, 5.1): data, no code.
#[derive(Debug)]
pub struct Story {
    /// The directory key the page uses to start this story.
    pub key: &'static str,
    /// Its name in the frame.
    pub name: &'static str,
    /// What the visitor is expected to do, which the frame shows.
    pub description: &'static str,
    pub deployment: &'static [u8],
    pub agent: &'static [u8],
    pub llm: &'static [u8],
    pub forge: &'static [u8],
}
impl Story {
    /// The clients' listener, also sent by the visitor as its Host.
    #[must_use]
    pub fn clients(&self) -> SocketAddr {
        let document: serde_json::Value = serde_json::from_slice(self.deployment).expect("story deployment JSON");
        document["application"]["clients"]["listener"]["socket"]
            .as_str()
            .expect("story clients socket")
            .parse()
            .expect("story clients address")
    }
}
/// Story A: the box's playground (playground.md, 5.2).
pub const A: Story = Story {
    key: "a",
    name: "A",
    description: "Try one of the three suggested requests. “Add CSV export to widget” is delegated, edited, pushed, checked by CI and landed on main; the other two are answered in text.",
    deployment: include_bytes!("../stories/a/temper.json"),
    agent: include_bytes!("../stories/a/agent.json"),
    llm: include_bytes!("../stories/a/llm.json"),
    forge: include_bytes!("../stories/a/forge.json"),
};
/// Every story, in the frame's order.
pub const STORIES: &[&Story] = &[&A];
