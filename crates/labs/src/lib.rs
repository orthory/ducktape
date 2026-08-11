//! Labs — consensus modules that are NOT part of any shipping network.
//!
//! ## why this crate exists
//!
//! A module registered at genesis is in the root-hash. Every node on the network
//! must then run it, agree on its state root at every height, and keep doing so
//! forever. So putting an experiment in the production
//! genesis registry is not a small thing. It commits the whole network to
//! carrying it, and a bug in it forks the chain.
//!
//! Everything in here is therefore **deliberately unwired**. No binary in this
//! repo registers these modules; `bin/node`'s genesis registry does not mention
//! them. They compile, they are tested, and they are ready — and taking one to
//! production is an explicit, network-wide decision someone makes on purpose,
//! not something that happens because a crate was in the workspace.
//!
//! Their heavier dependencies (revm, alloy, k256) are quarantined here too, so
//! the shipping node does not build them.
//!
//! ## what's in it
//!
//! - [`evm`] — a stateful EVM (REVM over the existing QMDB `Kv`), executing
//!   ordinary create/call transactions from the ordered op stream.
//! - [`multisig`] — M-of-N coordination of external-chain (Safe) transactions:
//!   consensus orders the owner approvals, Ethereum's `ecrecover` verifies them.

pub mod evm;
pub mod multisig;
