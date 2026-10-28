//! Checked edge bounds for temper's frame, cards, transcript and tree.
use crate::words::{Word, Words};
use alloc::boxed::Box;
use skein_ui::phrases::Hole;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub tree: skein_ui::Limits,
}
impl Limits {
    #[must_use]
    pub fn of(domain: &temper_client_domain::Limits, words: &Words) -> Option<Limits> {
        let receiving = &domain.client.domain;
        let text = words
            .capacity()?
            .max(receiving.text.checked_mul(2)?)
            .max(receiving.turn_text)
            .max(domain.forge.field_bytes.checked_mul(3)?.checked_add(128)?);
        // An account heading joins the provider, plan/display and holder's words.
        let text = text.max(receiving.text.checked_add(words.name_bytes.min(256).checked_mul(3)?)?.checked_add(64)?);
        let text = text.max(fold_bytes(receiving, words)?);
        // The sign-in return link percent-encodes a whole authorization address.
        let text = text.max(3400);
        let markdown = skein_ui::markdown::max_nodes(receiving.text)?;
        let cards = card_nodes(receiving, markdown)?;
        // Each watched task has one card at its first anchor: fourteen structural
        // nodes plus the result's markdown, and one region for live delegates.
        let delegations = temper_client_domain::pages::delegation::bound(domain);
        let delegated = markdown.checked_add(14)?.checked_mul(delegations)?.checked_add(1)?;
        // Both chat and agent pages price a committed turn by turn_text; fields
        // and cards retain text. One turn node holds at most turn_text bytes.
        // A decoded turn shares text across texts, names, inputs, outcomes and
        // summaries (jig-protocol-transcript's Budget; client/README.md, 5.4).
        // Splitting that text among rows squared blocks adds two markdown nodes
        // per block; five structural nodes cover a call's slot and two paragraphs.
        let blocks = receiving.rows.checked_mul(receiving.rows)?;
        let turn = skein_ui::markdown::max_nodes(receiving.turn_text)?
            .checked_add(blocks.checked_mul(2_u32.checked_add(5)?)?)?
            // Per message: message/who/says slots, Temper paragraph and text,
            // a call disclosure, and spare cut room (client/README.md, 3.5).
            .checked_add(receiving.rows.checked_mul(8)?)?
            // One turn region and a two-node cut status.
            .checked_add(3)?;
        // Provisional/live text is capped by thread::live_turn at text bytes.
        // Person/owner words have their own text cap, not Budget: a paragraph
        // and text per line (at most text + 1 lines), plus eight shell/state nodes.
        let plain = receiving.text.checked_add(1)?.checked_mul(2)?.checked_add(8)?;
        // An activity result can have three independent markdown documents:
        // cancellation reason, ending value and result value. No shared Budget;
        // price each whole, as before, plus its heading/status/region structure.
        let activity = markdown.checked_add(16)?.checked_mul(3)?;
        let line = turn.max(markdown.checked_add(8)?).max(plain).max(activity);
        let turns = line
            .checked_mul(temper_client_domain::pages::chat::LINES)?
            // Thread::lines retains LINES plus one Left status (two nodes).
            .checked_add(2)?
            // The conversation log.
            .checked_add(1)?
            // Request: paragraph and text per line, words at most text bytes;
            // region, message/who/says slots and the requester's paragraph/text.
            .checked_add(receiving.text.checked_add(1)?.checked_mul(2)?)?
            .checked_add(6)?
            // End line: one paragraph and its text, including the report's words.
            .checked_add(2)?;
        let counts = domain.accounts;
        // Per row: sources and their actions, state/fact pairs, ranks, editor and outcome.
        // Program observations may appear in both the state notice and the source list.
        // Fixed room covers disclosure/heading, controls, confirmation and sign-in ending.
        let hosts = counts.accounts.checked_mul(counts.sources.checked_mul(2)?.checked_add(4)?)?;
        let account = counts
            .sources
            .checked_mul(16)?
            .checked_add(hosts)?
            .checked_add(counts.uses.checked_mul(2)?)?
            .checked_add(64)?;
        let account_nodes = account
            .checked_mul(counts.accounts)?
            .checked_mul(2)?
            .checked_add(counts.sign_ins.checked_mul(2)?)?
            .checked_add(receiving.projects.checked_mul(4)?)?
            .checked_add(128)?
            .checked_add(receiving.rows.checked_mul(3)?)?
            // Worker and program observations, omission rows and retained sign-out failures.
            .checked_add(counts.accounts.checked_mul(counts.sources.checked_add(2)?)?.checked_mul(2)?)?
            .checked_add(2)?
            .checked_add(128_u32.checked_mul(2)?)?;
        // Catalogue: me_project_models x (1 Item + 1 Button + 2 Paragraphs),
        // plus me_model_efforts x 1 Button for the selected model. Fixed room:
        // 2 Slots + 1 List + 18 Paragraphs (including pending/refusal room)
        // + 3 Statuses + 6 Links + 2 Buttons = 32, plus the disclosure
        // and the plan line's separator, link and text priced below.
        let choices = temper_client_domain::pages::choice::MODELS
            .checked_mul(4)?
            .checked_add(temper_client_domain::pages::choice::EFFORTS)?
            .checked_add(32)?
            // Change model disclosure and the plan line's separator/link/text.
            .checked_add(4)?;
        // A goal's result: region, heading, status, their text and words slot plus
        // markdown; its contract paragraph and text. Budget: region, heading, balance,
        // six term/description pairs, a funder link, models term and cut line,
        // including text leaves: 34 nodes, plus two per allowed model name.
        let goal = markdown
            .checked_add(6)?
            .checked_add(2)?
            .checked_add(34)?
            // A procedure's goal slot, paragraph and link, including text leaves.
            .checked_add(5)?
            .checked_add(jig_client::contract::AUTHORITY_MODELS.checked_mul(2)?)?;
        // Each plan row adds a spend paragraph, dependency list and cut item
        // with a paragraph: six nodes including their text children. Each
        // dependency adds an item, link, status and added mark (client/README.md,
        // 5.8) with text: seven nodes, bounded by jig's receiving rows count per row.
        let dependencies = receiving.rows.checked_mul(7)?.checked_add(6)?.checked_mul(receiving.rows)?;
        // An agent's lineage, taskbar, brief and run: 32 fixed nodes including
        // text leaves, plus the spec's markdown and a dependency list. Jig's
        // task translation bounds dependencies by the receiving rows count:
        // one item, link, status and added mark (client/README.md, 5.8),
        // seven nodes with text per dependency.
        let agent = markdown.checked_add(33)?.checked_add(receiving.rows.checked_mul(7)?)?;
        let nodes = cards
            .checked_add(turns)?
            .checked_add(delegated)?
            .checked_add(goal)?
            .checked_add(agent)?
            .checked_add(receiving.rows.checked_mul(32)?)?
            .checked_add(dependencies)?
            .checked_add(receiving.projects.checked_mul(4)?)?
            .checked_add(128)?
            // Welcome steps, cards and foot, plus the shared accounts form.
            .max(account_nodes.checked_add(64)?)
            // Choice cards: region, heading, body, note and link with text;
            // the fixed step card also holds fields, snippets or progress.
            .max(temper_client_domain::forge::forges::ROWS.checked_mul(16)?.checked_add(152)?)
            .checked_add(choices)?
            // Owned rows add Runs and First password: a slot, term and description each.
            .checked_add(temper_client_domain::forge::forges::ROWS.checked_mul(6)?)?
            // The context line splits the repository from its forge suffix.
            .checked_add(1)?
            // Task amendment region, fields, buttons and explanatory text leaves (5.11).
            .checked_add(16)?
            // Your chats replaces two headings with a heading, filters and empty words.
            // The merge retains only scalar keys and borrowed lines on the stack.
            .checked_add(4)?
            // Start a chat heading and its text outside the demo start (2), and
            // a failed demo start's alert, its text leaf and Try again (3).
            .checked_add(5)?
            // The adoption form's region, named by the page's heading.
            .checked_add(1)?
            // GitHub's App line keeps each installation in its own bounded text slot.
            .checked_add(
                temper_client_domain::forge::forges::ROWS
                    .checked_mul(temper_client_domain::forge::forges::INSTALLATIONS)?
                    .checked_mul(2)?,
            )?;
        let tree = skein_ui::Limits {
            nodes,
            depth: receiving.rows.checked_add(34)?,
            text_per_node: text,
            text_bytes: nodes.checked_mul(text)?,
            drawing_bytes: 0,
            scene_marks: 0,
            scene_bytes: 0,
            styles_per_node: 2,
            // A diff emits at most four patches a node (skein's ui.md, 7).
            patches: nodes.checked_mul(4)?,
            bindings: nodes,
            slots: 0,
            markdown_depth: 3,
        };
        let limits = Limits { tree };
        valid(&limits, domain, words).then_some(limits)
    }
}
fn card_nodes(receiving: &jig_client::DomainLimits, markdown: u32) -> Option<u32> {
    // Each grant and note scope has a bounded sequence of pattern segments.
    let pattern = receiving.segments.checked_add(2)?.checked_mul(2)?;
    let authority = pattern.checked_mul(2)?.checked_add(4)?.checked_mul(receiving.rows)?.checked_add(24)?;
    let proposed = markdown
        .checked_mul(2)?
        .checked_add(authority)?
        .checked_add(receiving.rows.checked_mul(16)?)?
        .checked_add(32)?;
    let card = proposed.checked_mul(receiving.rows)?.checked_add(128)?;
    let cards = card.checked_mul(receiving.reads.checked_add(1)?)?;
    Some(cards)
}
// A disclosure joins admitted tool names, count, separators and repetition
// suffixes into one name (client/README.md, 3.5), beyond the turn's text budget.
fn fold_bytes(receiving: &jig_client::DomainLimits, words: &Words) -> Option<u32> {
    let calls = u64::from(receiving.rows);
    let count = words.say(Word::ToolCalls, &[Hole::Number(calls)]);
    let separator = words.say(Word::ToolSeparator, &[]);
    let repeated = words.say(Word::ToolRepeated, &[Hole::Name(Box::default()), Hole::Number(calls)]);
    let extra = u32::try_from(separator.len().checked_add(repeated.len())?).ok()?;
    receiving.turn_text.checked_add(u32::try_from(count.len()).ok()?)?.checked_add(receiving.rows.checked_mul(extra)?)
}

/// Refuses unrepresentable UI limits, account row counts and phrase capacity.
#[must_use]
pub fn valid(limits: &Limits, domain: &temper_client_domain::Limits, words: &Words) -> bool {
    // skein's valid() replaces this at the set's repin
    skein_ui::worst_case(&limits.tree).is_some()
        // skein's valid() replaces this at the set's repin
        && skein_ui::phrases::worst_case(words.book.limits()).is_some()
        && words.capacity().is_some()
        && domain.client.domain.rows.checked_mul(3).is_some()
}
