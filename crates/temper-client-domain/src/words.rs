//! Temper's shared overrides of jig's client vocabulary (client/README.md).
use alloc::boxed::Box;
use jig_client::{
    contract as c,
    words::{HoldWord, Word},
};
use skein_lib::bytes::copy_of;
use skein_ui::phrases::{Hole, HoleKind, HoleShape, Language, PhraseShape, Segment};

/// The budget explanation names the task whose budget needs changing.
#[must_use]
pub fn shape(word: Word, text: u32) -> PhraseShape {
    let mut shape = word.shape(text);
    if word == Word::Hold(HoldWord::Budget) {
        shape.holes = Box::new([HoleShape { kind: HoleKind::Number, bytes: 0 }]);
    }
    shape
}

/// The identity required by a hold phrase, including temper's budget override.
#[must_use]
pub fn hold_number(task: u64, reason: &c::HoldCategory) -> Option<Hole> {
    match reason {
        c::HoldCategory::Budget => Some(Hole::Number(task)),
        c::HoldCategory::Dependency { task } => Some(Hole::Number(*task)),
        c::HoldCategory::Uncertain { entry } => Some(Hole::Number(*entry)),
        c::HoldCategory::PoolLost { pool } => Some(Hole::Number(*pool)),
        c::HoldCategory::Exceeded(value) if value.bound != c::Bound::History => Some(Hole::Number(value.value)),
        c::HoldCategory::Exceeded(_)
        | c::HoldCategory::Failures { .. }
        | c::HoldCategory::Stalled
        | c::HoldCategory::Stopped
        | c::HoldCategory::StoppedBy { .. }
        | c::HoldCategory::Drift
        | c::HoldCategory::Effects
        | c::HoldCategory::EffectFailed
        | c::HoldCategory::Deadline
        | c::HoldCategory::HoldsWaited
        | c::HoldCategory::ResourceUnavailable
        | c::HoldCategory::ResourceAccess
        | c::HoldCategory::Circle
        | c::HoldCategory::Procedure => None,
    }
}

#[must_use]
pub fn english() -> Language {
    use jig_client::words::StatusWord;
    let mut language = jig_client::words::english();
    for phrase in &mut language.phrases {
        if phrase.id == Word::Decision(jig_client::words::DecisionWord::Funds).id() {
            phrase.forms.first_mut().expect("English word has a form").segments = Box::new([
                Segment::Text(copy_of(b"Funds ")),
                Segment::Hole { index: 0, kind: HoleKind::Amount },
                Segment::Text(copy_of(b" from your pool \xc2\xb7 ")),
                Segment::Hole { index: 1, kind: HoleKind::Amount },
                Segment::Text(copy_of(b" left, ")),
                Segment::Hole { index: 2, kind: HoleKind::Amount },
                Segment::Text(copy_of(b" after")),
            ]);
        }
        if phrase.id == Word::Hold(HoldWord::Budget).id() {
            phrase.forms.first_mut().expect("English word has a form").segments = Box::new([
                Segment::Text(copy_of(b"Insufficient activation budget. A release alone holds it again: give the task more budget (task amend ")),
                Segment::Hole { index: 0, kind: HoleKind::Number },
                Segment::Text(copy_of(b" with a larger budget, or decide its budget proposal), then release it; or cancel it.")),
            ]);
        }
        let literal =
            if phrase.id == Word::Status(StatusWord::Parked).id() { Some(b"Parked".as_slice()) } else { None };
        if let Some(literal) = literal {
            phrase.forms.first_mut().expect("English word has a form").segments =
                Box::new([Segment::Text(copy_of(literal))]);
        }
    }
    language
}

/// A checked phrasebook for text clients using the same overrides as the view.
#[must_use]
pub fn phrasebook(text: u32) -> skein_ui::phrases::Phrasebook {
    let mut shapes = jig_client::words::phrase_shapes(text);
    for (entry, word) in shapes.iter_mut().zip(jig_client::words::ALL) {
        *entry = shape(word, text);
    }
    let limits = skein_ui::phrases::PhraseLimits {
        languages: 1,
        phrases: jig_client::words::PHRASES,
        holes: 3,
        segments: jig_client::words::PHRASES.checked_mul(8).expect("phrase segments"),
        text_bytes: 16_384,
        hole_bytes: text,
    };
    skein_ui::phrases::Phrasebook::load(shapes, Box::new([english()]), &limits).expect("checked client phrases")
}
