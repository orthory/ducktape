//! the guide the model reads before it uses anything here.
//!
//! MCP's `initialize` response carries free-form server `instructions`, and both
//! runner CLIs surface them to the model. that makes this — not a skill file, not
//! a section bolted onto the consensus-composed prompt — the natural home for
//! "how Ducktape works and how to act in it": it ships with the binary, so it
//! can never describe a tool the binary does not have, and it costs the run
//! nothing until the tool server is actually attached.

use std::sync::LazyLock;

/// the server instructions. deliberately short: the tool DESCRIPTIONS carry the
/// per-tool detail, and repeating them here would be one more thing to keep in
/// sync. what belongs here is only what no single tool's description can say —
/// the shape of the system, and the two rules an agent gets wrong if nobody
/// tells it.
///
/// the one list it does carry — the action vocabulary — is INTERPOLATED from
/// [`agent::KNOWN_ACTIONS`] rather than typed out, so it cannot drift from the
/// names consensus actually enforces. a hand-copied list here silently taught
/// the model a short vocabulary, and the whole point of the paragraph is that
/// it can name the action behind a refusal.
pub static GUIDE: LazyLock<String> = LazyLock::new(|| {
    format!(
        "\
You are running inside Ducktape: a peer-to-peer workspace where chat, tasks, \
pages, a git forge, and a replicated filesystem (duckfs) are all modules of one \
consensus-backed network. These tools are how you read and write that network. \
Call ducktape_whoami first if you are unsure what you are permitted to do.

Two things are easy to get wrong:

1. YOUR WORKSPACE IS NOT DUCKFS. The directory you are working in is a checkout, \
already materialized on disk — read and edit it with your ordinary file tools, \
not with ducktape_files_*. In a Forge checkout, use ordinary git freely: commits \
you create are preserved, and any uncommitted tree left at the end is captured \
with the commit message from your final response. The ducktape_files_* tools \
read the SHARED filesystem, which is a different thing and mostly outside your \
checkout.

2. WRITES ARE GATED BY THE NETWORK ITSELF, AND A REFUSAL IS INFORMATION. Each \
write tool needs a matching action in your grant, and some also need resource \
caps. The complete action vocabulary is {actions}. The check does not happen in \
this tool server — your write passes through this run's narrow host signer and \
is refused or accepted by Ducktape itself, so a refusal is final and tells you \
exactly which action or cap you lack. Say so in your answer rather than working \
around it. Do not retry a refusal; it will not become allowed.

Agents are peers, not a permanent parent/child hierarchy. If another registered \
agent is useful, call ducktape_delegate while this run is live, then use \
ducktape_delegations to collect its result. Each call receives only the \
intersection of both agents' grants, and the root run's peer-call budget bounds \
concurrent live calls across the whole recursive tree. Completed calls release \
their slot. Reuse a request_id only for the same call.

You can write while you work — post progress to a channel, tick off a todo as \
you finish it — rather than saving everything for your final answer. Your final \
answer still follows whatever output contract your prompt gave you; these tools \
do not replace it.",
        actions = agent::KNOWN_ACTIONS.join(", ")
    )
});

#[cfg(test)]
mod tests {
    use super::GUIDE;

    #[test]
    fn the_guide_names_every_known_action() {
        // the guide's promise is that a refused agent can name what it lacks.
        // that only holds if the vocabulary it prints is the vocabulary
        // consensus enforces — so no hand-written subset may creep back in.
        for action in agent::KNOWN_ACTIONS {
            assert!(
                GUIDE.contains(action),
                "the initialize guide does not name the {action} action"
            );
        }
    }
}
