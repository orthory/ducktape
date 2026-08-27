//! filled in by Task 3.
pub fn personal_message(ns: &[u8], preimage: &[u8]) -> Vec<u8> {
    commonware_utils::union_unique(ns, preimage)
}
pub fn eip191_digest(_message: &[u8]) -> [u8; 32] {
    unimplemented!("task 3")
}
pub(crate) fn verify_personal_sign(
    _pubkey: &[u8],
    _ns: &[u8],
    _preimage: &[u8],
    _proof: &[u8],
) -> bool {
    false
}
