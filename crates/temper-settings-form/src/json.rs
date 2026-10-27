//! Bounded whole JSON tokenization.
use skein_json::{Document, tokenizer};
use skein_lib::{
    Env, Intake, List, Queue, Time, Wall,
    stream::{Down, Up},
};
pub(crate) fn document_depth(bytes: &[u8], depth: u32) -> Result<Document, ()> {
    let length = u32::try_from(bytes.len()).or(Err(()))?;
    if length == u32::MAX {
        return Err(());
    }
    let limits = tokenizer::Limits { depth, string: length, number: 20, chunk: length.max(1), length };
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let mut machine = tokenizer::Tokenizer::new(&limits);
    let mut intake = Intake::with_capacity(length.max(tokenizer::largest_demand(&limits)));
    intake.append(bytes).or(Err(()))?;
    let mut above = Queue::with_capacity(1);
    let mut below = Queue::with_capacity(1);
    let mut tokens = List::with_capacity(length);
    let mut demanded = None;
    let steps = bytes.len().checked_mul(4).ok_or(())?.checked_add(8).ok_or(())?;
    for _ in 0..steps {
        match demanded.take() {
            Some(read) => match intake.meet(read) {
                Some(bytes) => tokenizer::up(&mut machine, &env, Up::Bytes(bytes), &mut above, &mut below),
                None => tokenizer::up(&mut machine, &env, Up::End, &mut above, &mut below),
            },
            None => tokenizer::down(&mut machine, &env, tokenizer::Request::Next, &mut above, &mut below),
        }
        match below.pop() {
            Some(Down::Demand { read, .. }) => demanded = Some(read),
            Some(Down::Send(_) | Down::Finish) => return Err(()),
            None => {}
        }
        match above.pop() {
            Some(tokenizer::Event::Token(token)) => tokens.push(token).or(Err(()))?,
            Some(tokenizer::Event::Done) => {
                return Document::from_tokens(
                    tokens.as_slice(),
                    &skein_json::document::Limits { tokens: length, text: length },
                )
                .or(Err(()));
            }
            Some(
                tokenizer::Event::Failed(_)
                | tokenizer::Event::Closed
                | tokenizer::Event::Long(_)
                | tokenizer::Event::Skipped(_),
            ) => return Err(()),
            None => {}
        }
    }
    Err(())
}
