//! Child timers available to the root's fire pass, on the injected clock.
use crate::{Domain, Limits};
use skein_lib::{Duration, Env, Time, Wall};

impl Domain {
    /// Earliest participating child timer, including wall timers mapped to
    /// monotonic time. Stopped roots have no policy timer left to fire.
    #[must_use]
    pub fn next_deadline(&self, env: &Env<Limits>) -> Option<Time> {
        if self.journal.stopped() || self.core.restart_failure().is_some() {
            return None;
        }
        let mut next = earliest(self.core.accounts.next_deadline(), self.core.views.next_deadline());
        if let Some(start) = &self.demo_start
            && let Some(demo) = self.forge.demo()
            && !crate::route::demo::connected(&self.forge.connection(demo.adoption.forge))
        {
            next = earliest(next, Some(start.until));
        }
        if self.core.stopping() {
            return next;
        }
        if self.ready() {
            for deadline in [
                self.core.people.next_deadline(),
                self.core.tasks.next_deadline(),
                self.core.fleet.next_deadline(),
                self.core.brief.next_deadline(),
            ] {
                next = earliest(next, deadline);
            }
            next = earliest(next, wall_deadline(self.core.people.next_wall_expiry(), env.now, env.wall));
            next = earliest(next, wall_deadline(self.core.effect_deadline(), env.now, env.wall));
        }
        if self.ready() || self.settling || crate::restart::connector_restarting(self) {
            next = earliest(next, self.forge.next_deadline());
            next = earliest(next, wall_deadline(self.forge.next_wall_deadline(), env.now, env.wall));
        }
        next
    }
}
fn earliest(left: Option<Time>, right: Option<Time>) -> Option<Time> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (left, None) => left,
        (None, right) => right,
    }
}
fn wall_deadline(deadline: Option<Wall>, now: Time, wall: Wall) -> Option<Time> {
    let deadline = deadline?;
    let span = Duration::from_nanos(deadline.as_nanos().saturating_sub(wall.as_nanos()));
    Some(now.saturating_add(span))
}

#[cfg(test)]
mod tests {
    use super::wall_deadline;
    use skein_lib::{Time, Wall};
    #[test]
    fn wall_timers_keep_their_span_and_expired_ones_are_due_now() {
        let now = Time::from_nanos(10);
        let wall = Wall::from_nanos(100);
        for (deadline, expected) in [
            (None, None),
            (Some(Wall::from_nanos(105)), Some(Time::from_nanos(15))),
            (Some(Wall::from_nanos(100)), Some(Time::from_nanos(10))),
            (Some(Wall::from_nanos(99)), Some(Time::from_nanos(10))),
        ] {
            assert_eq!(wall_deadline(deadline, now, wall), expected);
        }
    }
}
