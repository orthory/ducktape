//! Fluent31 read model for EVM receipts and logs.
//!
//! QMDB remains canonical EVM state. This mapper only folds the module's
//! authenticated `Receipt` follow-ups, producing ordered receipt/log views.
//! Receipt history cannot be reconstructed from current account state, so the
//! mapper deliberately has no from-state rebuild: after state sync its views
//! begin at the reported backfill boundary and accrue from there.

use indexer::{ApplyCtx, Derived, Error, ModuleIndexer, OpMeta, OriginKind, Result, ViewReader};
use serde::{Deserialize, Serialize};

use super::{EvmLog, EvmMsg, EvmResult, EvmTx, decode_msg};

const DEFAULT_LIMIT: usize = 50;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptRow {
    pub height: u64,
    pub time: u64,
    pub seq: u32,
    pub caller: String,
    pub transaction: EvmTx,
    pub result: EvmResult,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogRow {
    pub height: u64,
    pub time: u64,
    pub seq: u32,
    pub log_index: u32,
    pub address: String,
    pub topics: Vec<String>,
    pub data: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EvmViewQuery {
    #[serde(rename_all = "camelCase")]
    Receipts {
        #[serde(default)]
        after: Option<String>,
        #[serde(default)]
        limit: Option<usize>,
    },
    #[serde(rename_all = "camelCase")]
    Contract { address: String },
    #[serde(rename_all = "camelCase")]
    Logs {
        #[serde(default)]
        address: Option<String>,
        #[serde(default)]
        topic: Option<String>,
        #[serde(default)]
        after: Option<String>,
        #[serde(default)]
        limit: Option<usize>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EvmViewReply {
    #[serde(rename_all = "camelCase")]
    Receipts {
        receipts: Vec<ReceiptRow>,
        has_more: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        next_after: Option<String>,
    },
    Contract(Option<ReceiptRow>),
    #[serde(rename_all = "camelCase")]
    Logs {
        logs: Vec<LogRow>,
        has_more: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        next_after: Option<String>,
    },
}

pub struct EvmIndex {
    module: String,
}

impl EvmIndex {
    pub fn new(module: impl Into<String>) -> Self {
        Self {
            module: module.into(),
        }
    }
}

fn receipt_key(height: u64, seq: u32) -> String {
    format!("receipt/{height:016x}/{seq:04x}")
}

fn contract_key(address: &[u8; 20]) -> String {
    format!("contract/{}", hex(address))
}

fn log_suffix(height: u64, seq: u32, log_index: u32) -> String {
    format!("{height:016x}/{seq:04x}/{log_index:04x}")
}

fn log_entries(meta: &OpMeta<'_>, log_index: u32, log: &EvmLog) -> Result<Vec<(String, Vec<u8>)>> {
    let address = hex(&log.address);
    let suffix = log_suffix(meta.height, meta.seq, log_index);
    let row = LogRow {
        height: meta.height,
        time: meta.time,
        seq: meta.seq,
        log_index,
        address: format!("0x{address}"),
        topics: log
            .topics
            .iter()
            .map(|topic| format!("0x{}", hex(topic)))
            .collect(),
        data: format!("0x{}", hex(&log.data)),
    };
    let bytes = serde_json::to_vec(&row).map_err(|e| Error::Mapper(e.to_string()))?;
    let mut entries = vec![
        (format!("log/all/{suffix}"), bytes.clone()),
        (format!("log/address/{address}/{suffix}"), bytes.clone()),
    ];
    for topic in &log.topics {
        let topic = hex(topic);
        entries.push((format!("log/topic/{topic}/{suffix}"), bytes.clone()));
        entries.push((
            format!("log/address-topic/{address}/{topic}/{suffix}"),
            bytes.clone(),
        ));
    }
    Ok(entries)
}

fn decode_rows<T: for<'de> Deserialize<'de>>(entries: &[(Vec<u8>, Vec<u8>)]) -> Result<Vec<T>> {
    entries
        .iter()
        .map(|(_, value)| serde_json::from_slice(value).map_err(|e| Error::Mapper(e.to_string())))
        .collect()
}

#[async_trait::async_trait(?Send)]
impl ModuleIndexer for EvmIndex {
    fn module(&self) -> &str {
        &self.module
    }

    fn index_op(
        &self,
        _ctx: &ApplyCtx,
        meta: &OpMeta,
        payload: &[u8],
        out: &mut Derived,
    ) -> Result<()> {
        let EvmMsg::Receipt {
            transaction,
            caller,
            result,
        } = decode_msg(payload).map_err(Error::Mapper)?
        else {
            return Ok(());
        };
        if meta.origin.kind != OriginKind::Module
            || meta.origin.id.as_deref() != Some(self.module.as_str())
        {
            return Err(Error::Mapper(
                "applied EVM receipt did not originate from the EVM module".into(),
            ));
        }

        let row = ReceiptRow {
            height: meta.height,
            time: meta.time,
            seq: meta.seq,
            caller: format!("0x{}", hex(&caller)),
            transaction,
            result,
        };
        let bytes = serde_json::to_vec(&row).map_err(|e| Error::Mapper(e.to_string()))?;
        out.put(receipt_key(meta.height, meta.seq), bytes.clone());
        if let Some(address) = row.result.created_address {
            out.put(contract_key(&address), bytes);
        }
        for (log_index, log) in row.result.logs.iter().enumerate() {
            for (key, value) in log_entries(meta, log_index as u32, log)? {
                out.put(key, value);
            }
        }
        Ok(())
    }

    fn serve_view(&self, reader: &ViewReader, req: &[u8]) -> Result<Vec<u8>> {
        let query: EvmViewQuery =
            serde_json::from_slice(req).map_err(|e| Error::View(e.to_string()))?;
        let reply = match query {
            EvmViewQuery::Receipts { after, limit } => {
                let page = reader.scan(
                    b"receipt/",
                    after.as_deref().map(str::as_bytes),
                    limit.unwrap_or(DEFAULT_LIMIT),
                )?;
                EvmViewReply::Receipts {
                    receipts: decode_rows(&page.entries)?,
                    has_more: page.has_more,
                    next_after: page.next_after,
                }
            }
            EvmViewQuery::Contract { address } => {
                let address = fixed_hex::<20>(&address)?;
                let receipt = reader
                    .get(contract_key(&address).as_bytes())?
                    .map(|bytes| {
                        serde_json::from_slice(&bytes).map_err(|e| Error::Mapper(e.to_string()))
                    })
                    .transpose()?;
                EvmViewReply::Contract(receipt)
            }
            EvmViewQuery::Logs {
                address,
                topic,
                after,
                limit,
            } => {
                let prefix = match (address, topic) {
                    (Some(address), Some(topic)) => format!(
                        "log/address-topic/{}/{}/",
                        hex(&fixed_hex::<20>(&address)?),
                        hex(&fixed_hex::<32>(&topic)?),
                    ),
                    (Some(address), None) => {
                        format!("log/address/{}/", hex(&fixed_hex::<20>(&address)?))
                    }
                    (None, Some(topic)) => {
                        format!("log/topic/{}/", hex(&fixed_hex::<32>(&topic)?))
                    }
                    (None, None) => "log/all/".into(),
                };
                let page = reader.scan(
                    prefix.as_bytes(),
                    after.as_deref().map(str::as_bytes),
                    limit.unwrap_or(DEFAULT_LIMIT),
                )?;
                EvmViewReply::Logs {
                    logs: decode_rows(&page.entries)?,
                    has_more: page.has_more,
                    next_after: page.next_after,
                }
            }
        };
        serde_json::to_vec(&reply).map_err(|e| Error::View(e.to_string()))
    }
}

fn fixed_hex<const N: usize>(value: &str) -> Result<[u8; N]> {
    let value = value.strip_prefix("0x").unwrap_or(value);
    if value.len() != N * 2 {
        return Err(Error::View(format!("expected {}-byte hex value", N)));
    }
    let mut out = [0; N];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[i * 2..i * 2 + 2], 16)
            .map_err(|_| Error::View("invalid hex value".into()))?;
    }
    Ok(out)
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use indexer::{AppliedOp, BlockOps, IndexStore, OriginTag};

    use super::*;
    use crate::evm::{EvmStatus, encode_msg};

    fn receipt() -> EvmMsg {
        EvmMsg::Receipt {
            transaction: EvmTx::Call {
                to: [7; 20],
                input: vec![1, 2, 3],
                gas_limit: 50_000,
            },
            caller: [9; 20],
            result: EvmResult {
                status: EvmStatus::Success,
                gas_used: 22_000,
                output: Vec::new(),
                created_address: Some([7; 20]),
                logs: vec![EvmLog {
                    address: [7; 20],
                    topics: vec![[8; 32]],
                    data: vec![4, 5],
                }],
            },
        }
    }

    fn view(store: &IndexStore, query: serde_json::Value) -> EvmViewReply {
        serde_json::from_slice(
            &store
                .view("evm", &serde_json::to_vec(&query).unwrap())
                .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn indexes_receipts_contracts_and_filtered_logs() {
        let dir = tempfile::tempdir().unwrap();
        let store = IndexStore::open(dir.path(), &["evm"])
            .unwrap()
            .with_indexer(Box::new(EvmIndex::new("evm")));
        store
            .apply_block(&BlockOps {
                height: 4,
                time: 44,
                ops: vec![AppliedOp {
                    module: "evm".into(),
                    origin: OriginTag::module("evm"),
                    payload: encode_msg(&receipt()),
                }],
                record: None,
            })
            .unwrap();

        let EvmViewReply::Receipts { receipts, .. } =
            view(&store, serde_json::json!({"receipts": {}}))
        else {
            panic!("receipt reply")
        };
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].caller, format!("0x{}", hex(&[9; 20])));

        let EvmViewReply::Contract(Some(contract)) = view(
            &store,
            serde_json::json!({"contract": {"address": format!("0x{}", hex(&[7; 20]))}}),
        ) else {
            panic!("contract reply")
        };
        assert_eq!(contract.height, 4);

        let EvmViewReply::Logs { logs, .. } = view(
            &store,
            serde_json::json!({"logs": {
                "address": format!("0x{}", hex(&[7; 20])),
                "topic": format!("0x{}", hex(&[8; 32]))
            }}),
        ) else {
            panic!("logs reply")
        };
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].data, "0x0405");
    }
}
