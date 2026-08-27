//! filled in by Task 4.
pub fn webauthn_challenge(_ns: &[u8], _preimage: &[u8]) -> [u8; 32] {
    unimplemented!("task 4")
}
pub fn webauthn_proof(
    _authenticator_data: &[u8],
    _client_data_json: &[u8],
    _signature: &[u8],
) -> Vec<u8> {
    unimplemented!("task 4")
}
pub(crate) fn verify_assertion(_pubkey: &[u8], _ns: &[u8], _preimage: &[u8], _proof: &[u8]) -> bool {
    false
}
