//! Real SEV-SNP verification exercised end-to-end on non-TEE hardware: the
//! testkit mints a chain + report, and `airlock::verify` runs the REAL parser,
//! certificate-chain verify, and report-signature verify against it.
#![cfg(feature = "testkit")]

use airlock::attest::{self, Measurement, MRTD_LEN};
use airlock::testkit::SnpTestEnclave;
use airlock::verify::verify_quote;

fn meas(b: u8) -> Measurement {
    Measurement([b; MRTD_LEN])
}

#[tokio::test]
async fn minted_snp_quote_verifies_and_binds_report_data() {
    let enclave = SnpTestEnclave::new(&meas(0x11)).unwrap();
    let rd = attest::make_report_data(&[9u8; 32], &[8u8; 32]);
    let quote = enclave.quote(&rd).unwrap();
    let out = verify_quote(&quote, &meas(0x11), &enclave.roots()).await.unwrap();
    assert_eq!(out, rd);
}

#[tokio::test]
async fn wrong_measurement_is_rejected() {
    let enclave = SnpTestEnclave::new(&meas(0x11)).unwrap();
    let quote = enclave.quote(&attest::make_report_data(&[9u8; 32], &[8u8; 32])).unwrap();
    assert!(verify_quote(&quote, &meas(0x22), &enclave.roots()).await.is_err());
}

#[tokio::test]
async fn tampered_report_data_breaks_the_signature() {
    let enclave = SnpTestEnclave::new(&meas(0x11)).unwrap();
    let mut quote = enclave.quote(&attest::make_report_data(&[9u8; 32], &[8u8; 32])).unwrap();
    quote[0x50] ^= 1; // REPORT_DATA offset in the SNP report
    assert!(verify_quote(&quote, &meas(0x11), &enclave.roots()).await.is_err());
}

#[tokio::test]
async fn a_quote_from_a_different_chain_is_rejected() {
    // Two independently minted enclaves: A's roots must refuse B's quote.
    let a = SnpTestEnclave::new(&meas(0x11)).unwrap();
    let b = SnpTestEnclave::new(&meas(0x11)).unwrap();
    let quote = b.quote(&attest::make_report_data(&[9u8; 32], &[8u8; 32])).unwrap();
    assert!(verify_quote(&quote, &meas(0x11), &a.roots()).await.is_err());
}

#[tokio::test]
async fn garbage_quote_is_rejected() {
    let enclave = SnpTestEnclave::new(&meas(0x11)).unwrap();
    assert!(verify_quote(&[0u8; 32], &meas(0x11), &enclave.roots()).await.is_err());
}
