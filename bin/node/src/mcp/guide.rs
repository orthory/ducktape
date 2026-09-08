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
/// per-tool detail, and the catalog `ducktape_actions` returns carries the
/// per-operation detail, so repeating either here would be one more thing to
/// keep in sync. what belongs here is only what no single tool's description
/// can say — the shape of the system, and the rules an agent gets wrong if
/// nobody tells it.
///
/// the one list it does carry — the grant vocabulary — is INTERPOLATED from
/// [`runs::KNOWN_ACTIONS`] rather than typed out, so it cannot drift from the
/// names consensus actually enforces. a hand-copied list here silently taught
/// the model a short vocabulary, and the whole point of the paragraph is that
/// it can name the grant behind a refusal.
pub static GUIDE: LazyLock<String> = LazyLock::new(|| {
    format!(
        "\
You are running inside Ducktape: a peer-to-peer workspace where chat, tasks, \
pages, a git forge, and a replicated filesystem (duckfs) are all modules of one \
consensus-backed network. These tools are how you read and write that network. \
Call ducktape_whoami first if you are unsure what you are permitted to do, and \
ducktape_actions to see the operation catalog.

Two things are easy to get wrong:

1. YOUR WORKSPACE IS NOT DUCKFS. The directory you are working in is a checkout, \
already materialized on disk — read and edit it with your ordinary file tools, \
not with the files.* read operations. In a Forge checkout, use ordinary git \
freely: commits you create are preserved, and any uncommitted tree left at the \
end is captured with the commit message from your final response. The files.* \
operations read the SHARED filesystem, which is a different thing and mostly \
outside your checkout.

2. WRITES ARE GATED BY THE NETWORK ITSELF, AND A REFUSAL IS INFORMATION. Every \
write goes through one tool, ducktape_action(operation, target, input, \
request_id). The catalog names each operation's schemas and the grant it needs; \
the grant vocabulary is {actions}, and some operations also need resource caps. \
The check does not happen in this tool server — your write passes through this \
run's narrow host signer and is refused or accepted by Ducktape itself, so a \
refusal is final and tells you exactly which grant or cap you lack. Say so in \
your answer rather than working around it. Do not retry a refusal; it will not \
become allowed. Pick a fresh request_id for each new write and reuse one only to \
retry the identical write; ducktape_action returns the committed receipt, and \
ducktape_receipt reads it again later.

Reads go through ducktape_query(operation, target, input) with the read \
operations the catalog lists: chat.channels and chat.messages, tasks.list and \
jobs.get, pages.list and pages.get, forge.repos, forge.items, forge.item and \
forge.pr_diff, files.ls, files.read and files.grep, agents.list, runs.list and \
agent.calls.

Use the reply operation to post progress or ask a question where this run was \
called: its chat thread, Pages block/comment thread, or job discussion. Name an \
explicit destination with chat.post_message, pages.comment or jobs.comment. \
Ducktape posts as your program account; a temporary run key authenticates the \
request. Use the other operations for tasks, files or page edits, and tick off a \
todo as you finish it — rather than saving everything for your final answer. \
Your final response's actions carry the same envelopes; modules.update is \
final-response only, and agent.call is live only.

modules.update: build the module using its repository's toolchain and \
dependencies. The standard Linux guest includes Rust, the wasm32-unknown-unknown \
target and wasm-tools; it has no package-registry network, so dependencies must \
be in the checkout. Package a component with `ducktape module pack \
component.wasm --out module.artifact` (optionally `--index index.wasm`). This \
offline command prints the deployment SHA-256. Commit the artifact in your Forge \
checkout and follow the output contract. Ducktape binds the artifact to the \
host-pushed commit before your program requests deployment.

Agents are peers, not a permanent parent/child hierarchy. If another registered \
agent is useful, call it with the agent.call operation while this run is live, \
then read agent.calls to collect its result. Each call receives only the \
intersection of both agents' grants, and the root run's peer-call budget bounds \
concurrent live calls across the whole recursive tree. Completed calls release \
their slot. Your final answer still follows whatever output contract your prompt \
gave you; these tools do not replace it.",
        actions = runs::KNOWN_ACTIONS.join(", ")
    )
});

#[cfg(test)]
mod tests {
    use super::GUIDE;

    #[test]
    fn the_guide_names_every_known_action_and_every_tool() {
        // the guide's promise is that a refused agent can name what it lacks.
        // that only holds if the vocabulary it prints is the vocabulary
        // consensus enforces — so no hand-written subset may creep back in.
        for action in runs::KNOWN_ACTIONS {
            assert!(
                GUIDE.contains(action),
                "the initialize guide does not name the {action} action"
            );
        }
        for tool in crate::mcp::tools::all() {
            let control_tool = tool.name == "ducktape_extend_provider_idle";
            if control_tool {
                continue;
            }
            assert!(
                GUIDE.contains(tool.name),
                "the initialize guide does not name {}",
                tool.name
            );
        }
    }
}
