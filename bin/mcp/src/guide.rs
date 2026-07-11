//! the guide the model reads before it uses anything here.
//!
//! MCP's `initialize` response carries free-form server `instructions`, and both
//! runner CLIs surface them to the model. that makes this — not a skill file, not
//! a section bolted onto the consensus-composed prompt — the natural home for
//! "how Ducktape works and how to act in it": it ships with the binary, so it
//! can never describe a tool the binary does not have, and it costs the run
//! nothing until the tool server is actually attached.

/// the server instructions. deliberately short: the tool DESCRIPTIONS carry the
/// per-tool detail, and repeating them here would be one more thing to keep in
/// sync. what belongs here is only what no single tool's description can say —
/// the shape of the system, and the two rules an agent gets wrong if nobody
/// tells it.
pub const GUIDE: &str = "\
You are running inside Ducktape: a peer-to-peer workspace where chat, tasks, \
pages, a git forge, and a replicated filesystem (duckfs) are all modules of one \
consensus-backed network. These tools are how you read and write that network. \
Call ducktape_whoami first if you are unsure what you are permitted to do.

Two things are easy to get wrong:

1. YOUR WORKSPACE IS NOT DUCKFS. The directory you are working in is a checkout, \
already materialized on disk — read and edit it with your ordinary file tools, \
not with ducktape_files_*. Whatever you leave in it is committed back to \
Ducktape when your run ends; you do not commit it yourself. The ducktape_files_* \
tools read the SHARED filesystem, which is a different thing and mostly outside \
your checkout.

2. WRITES ARE GATED, AND A REFUSAL IS INFORMATION. Each write tool needs a \
matching action in your grant (chat.post, tasks.create, tasks.update_status, \
pages.comment, pages.set_checked), and some also need resource caps. If a tool \
refuses, it names the exact action or cap you lack — say so in your answer \
rather than working around it. Do not retry a refusal; it will not become \
allowed.

You can write while you work — post progress to a channel, tick off a todo as \
you finish it — rather than saving everything for your final answer. Your final \
answer still follows whatever output contract your prompt gave you; these tools \
do not replace it.";
