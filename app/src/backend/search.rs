use super::*;

/// DOES THE ANSWER A SEARCH WAS SENT FOR STILL SPEAK FOR WHAT IS IN THE BOX?
///
/// Every search surface is ENTER-TO-SUBMIT and two-way bound with no
/// `change=` route, so a keystroke writes the draft and runs no handler at
/// all: nothing but this comparison can retire an answer as the reader types
/// on. `query` is the string a search was actually SENT for and is empty when
/// no answer is standing; `searching` covers the round trip the submit opened,
/// during which no answer exists yet.
///
/// It replaces the hand-synced conjunct arms pages and chat each carried. A
/// module-owned view carries the same reading in its own crate — the view
/// wire carries words, not functions.
pub fn search_answer_stands(query: &str, draft: &str, searching: bool) -> bool {
    !searching && !query.is_empty() && draft.trim() == query
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PaletteSearchData {
    pub chat_hits: Vec<ChatSearchHit>,
    pub page_hits: Vec<PageSearchHit>,
}

/// The command palette's per-keystroke search: one debounced call covering
/// chat and pages together. Typing a word used to issue two RPC round trips
/// per keystroke. The Ice `replace` lane owns cancellation and stale delivery;
/// dropping a superseded task during this sleep prevents its RPCs from firing.
pub async fn palette_search(rpc: String, text: String) -> Result<PaletteSearchData, AppError> {
    tokio::time::sleep(Duration::from_millis(250)).await;
    let (chat, pages) = tokio::join!(
        search_chat(rpc.clone(), String::new(), text.clone()),
        search_pages(rpc, String::new(), text)
    );
    let both_failed = chat.is_err() && pages.is_err();
    if both_failed {
        return Err(AppError {
            message: "Search did not reach the node. Retry in a moment.".into(),
            committed: false,
        });
    }
    Ok(PaletteSearchData {
        chat_hits: chat.map(|data| data.hits).unwrap_or_default(),
        page_hits: pages.map(|data| data.hits).unwrap_or_default(),
    })
}
