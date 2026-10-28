//! Integer JSON for supplied read facts.
use skein_json::writer::Encoder;
#[expect(clippy::too_many_lines, reason = "one exhaustive encoding of jig's owner activity vocabulary")]
pub(super) fn encode_owner_activity(
    value: &jig_client::contract::OwnerActivity,
    out: &mut Encoder,
    budget: &mut super::Cut,
) {
    match value {
        jig_client::contract::OwnerActivity::Words { message, by, kind, words } => {
            out.object_start();
            out.key(b"words");
            out.object_start();
            out.key(b"message");
            out.unsigned(*(message));
            out.key(b"by");
            super::encode_owner_requester(by, out, budget);
            out.key(b"kind");
            super::encode_owner_message_kind(kind, out, budget);
            out.key(b"words");
            budget.words((words).bytes(), out);
            out.object_end();
            out.object_end();
        }
        jig_client::contract::OwnerActivity::Heard(field_0) => {
            out.object_start();
            out.key(b"heard");
            encode_owner_word(field_0, out, budget);
            out.object_end();
        }
        jig_client::contract::OwnerActivity::Notified { recipient, word } => {
            out.object_start();
            out.key(b"notified");
            out.object_start();
            out.key(b"recipient");
            super::encode_owner_requester(recipient, out, budget);
            out.key(b"word");
            encode_owner_word(word, out, budget);
            out.object_end();
            out.object_end();
        }
        jig_client::contract::OwnerActivity::RunBegan { attempt, why, resumed } => {
            out.object_start();
            out.key(b"run_began");
            out.object_start();
            out.key(b"attempt");
            out.unsigned(*(attempt));
            out.key(b"why");
            encode_owner_run_why(why, out, budget);
            out.key(b"resumed");
            out.boolean(*(resumed));
            out.object_end();
            out.object_end();
        }
        jig_client::contract::OwnerActivity::RunEnded { attempt, end, spent } => {
            out.object_start();
            out.key(b"run_ended");
            out.object_start();
            out.key(b"attempt");
            out.unsigned(*(attempt));
            out.key(b"end");
            encode_owner_run_end(*end, out, budget);
            out.key(b"spent");
            out.unsigned(*(spent));
            out.object_end();
            out.object_end();
        }
        jig_client::contract::OwnerActivity::Turn { reply, attempt, turn, read } => {
            out.object_start();
            out.key(b"turn");
            out.object_start();
            out.key(b"reply");
            out.unsigned(*(reply));
            out.key(b"attempt");
            out.unsigned(*(attempt));
            out.key(b"turn");
            out.unsigned(u64::from(*(turn)));
            out.key(b"read");
            match read {
                Some(item_1) => {
                    out.unsigned(*(item_1));
                }
                None => out.null(),
            }
            out.object_end();
            out.object_end();
        }
        jig_client::contract::OwnerActivity::Decided { revision } => {
            out.object_start();
            out.key(b"decided");
            out.object_start();
            out.key(b"revision");
            out.unsigned(*(revision));
            out.object_end();
            out.object_end();
        }
        jig_client::contract::OwnerActivity::Kept { attempt, pending, question } => {
            out.object_start();
            out.key(b"kept");
            out.object_start();
            out.key(b"attempt");
            out.unsigned(*attempt);
            out.key(b"pending");
            out.array_start();
            for task in pending {
                out.unsigned(*task);
            }
            out.array_end();
            out.key(b"question");
            match question {
                Some(number) => out.unsigned(*number),
                None => out.null(),
            }
            out.object_end();
            out.object_end();
        }
        jig_client::contract::OwnerActivity::Milestone { position } => {
            out.object_start();
            out.key(b"milestone");
            out.object_start();
            out.key(b"position");
            out.unsigned(*(position));
            out.object_end();
            out.object_end();
        }
    }
}

pub(super) fn encode_owner_activity_line(
    value: &jig_client::contract::OwnerActivityLine,
    out: &mut Encoder,
    budget: &mut super::Cut,
) {
    out.object_start();
    out.key(b"task");
    out.unsigned(value.task);
    out.key(b"position");
    out.unsigned(value.position);
    out.key(b"at");
    out.unsigned(value.at.as_nanos());
    out.key(b"line");
    encode_owner_activity(&value.line, out, budget);
    out.object_end();
}

pub(super) fn encode_owner_run_end(
    value: jig_client::contract::OwnerRunEnd,
    out: &mut Encoder,
    budget: &mut super::Cut,
) {
    let value = &value;
    match value {
        jig_client::contract::OwnerRunEnd::Parked => out.string(b"parked"),
        jig_client::contract::OwnerRunEnd::Finished => out.string(b"finished"),
        jig_client::contract::OwnerRunEnd::Cancelled => out.string(b"cancelled"),
        jig_client::contract::OwnerRunEnd::Failed { class } => {
            out.object_start();
            out.key(b"failed");
            out.object_start();
            out.key(b"class");
            super::encode_hold_failure(*(class), out, budget);
            out.object_end();
            out.object_end();
        }
        jig_client::contract::OwnerRunEnd::Refused => out.string(b"refused"),
    }
}

pub(super) fn encode_owner_run_why(
    value: &jig_client::contract::OwnerRunWhy,
    out: &mut Encoder,
    budget: &mut super::Cut,
) {
    match value {
        jig_client::contract::OwnerRunWhy::Made => out.string(b"made"),
        jig_client::contract::OwnerRunWhy::Ready => out.string(b"ready"),
        jig_client::contract::OwnerRunWhy::Message { message } => {
            out.object_start();
            out.key(b"message");
            out.object_start();
            out.key(b"message");
            out.unsigned(*(message));
            out.object_end();
            out.object_end();
        }
        jig_client::contract::OwnerRunWhy::Released => out.string(b"released"),
        jig_client::contract::OwnerRunWhy::Retry { tries } => {
            out.object_start();
            out.key(b"retry");
            out.object_start();
            out.key(b"tries");
            super::encode_owner_failure_tries(tries, out, budget);
            out.object_end();
            out.object_end();
        }
        jig_client::contract::OwnerRunWhy::Narrowed => out.string(b"narrowed"),
        jig_client::contract::OwnerRunWhy::Changed => out.string(b"changed"),
    }
}

pub(super) fn encode_owner_word(value: &jig_client::contract::OwnerWord, out: &mut Encoder, budget: &mut super::Cut) {
    out.object_start();
    out.key(b"number");
    out.unsigned(value.number);
    out.key(b"from");
    super::encode_owner_requester(&value.from, out, budget);
    out.key(b"kind");
    super::encode_owner_message_kind(&value.kind, out, budget);
    out.key(b"words");
    budget.words(value.words.bytes(), out);
    out.key(b"at");
    out.unsigned(value.at.as_nanos());
    out.key(b"hits");
    out.unsigned(u64::from(value.hits));
    out.key(b"eligible");
    out.boolean(value.eligible);
    out.object_end();
}
