//! Causal delivery pacing shared by recorded stories' fake transports.
use skein_fake_llm_domain::api::Role;
use skein_fake_peers::llm::Observation;

/// Pause delivery of one recorded completion until the driver releases it.
pub struct ReplyGate {
    cue: &'static str,
    assistant_before: usize,
    seen: usize,
    paused: bool,
    released: bool,
}
impl ReplyGate {
    #[must_use]
    pub fn new(cue: &'static str, assistant_before: usize) -> Self {
        Self { cue, assistant_before, seen: 0, paused: false, released: false }
    }

    pub fn observe(&mut self, observations: &[Observation]) {
        for observation in &observations[self.seen..] {
            if let Observation::Query { query, .. } = observation
                && !self.released
                && String::from_utf8_lossy(&query.system).contains(self.cue)
                && query.messages.iter().filter(|message| message.role == Role::Assistant).count()
                    == self.assistant_before
            {
                self.paused = true;
            }
        }
        self.seen = observations.len();
    }

    pub fn release(&mut self) {
        self.released = true;
        self.paused = false;
    }

    #[must_use]
    pub fn paused(&self) -> bool {
        self.paused
    }
}

#[cfg(test)]
mod tests {
    use super::ReplyGate;
    use skein_fake_llm_domain::api;
    use skein_fake_peers::llm::Observation;
    use skein_lib::Token;

    fn query(cue: &str, replies: usize) -> Observation {
        Observation::Query {
            connection: Token::new(1),
            query: api::Query {
                credential: None,
                caching: api::Caching::Unscoped,
                model: b"fake".as_slice().into(),
                system: cue.as_bytes().into(),
                tools: Box::new([]),
                choice: api::ToolChoice::None,
                messages: (0..replies)
                    .map(|_| api::Message { role: api::Role::Assistant, parts: Box::new([]) })
                    .collect(),
                max_tokens: 1,
            },
        }
    }

    #[test]
    fn only_the_named_reply_is_paused_and_release_cannot_rearm_it() {
        let mut gate = ReplyGate::new("dark review", 7);
        let mut history = vec![query("CSV review", 7), query("dark review", 6)];
        gate.observe(&history);
        assert!(!gate.paused());
        history.push(query("dark review", 7));
        gate.observe(&history);
        assert!(gate.paused());
        gate.release();
        assert!(!gate.paused());
        history.push(query("dark review", 7));
        gate.observe(&history);
        assert!(!gate.paused());
    }
}
