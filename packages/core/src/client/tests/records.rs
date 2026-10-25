//! Receipts, transactions and blocks read from Elysium testnet's own answers
//! (`elysium_testnet.json`), each field against the text the node wrote: one block holding
//! ArbOS's internal transaction, a legacy one and an EIP-1559 one, and a transaction of each of
//! Nitro's types.

use serde_json::Value;

use crate::client::rpc::{BlockTag, nitro};
use crate::client::tx_hash;
use crate::test_support::{answer, result, text};

use super::*;

fn quantity(value: &Value) -> u64 {
    u64::from_str_radix(text(value).trim_start_matches("0x"), 16).unwrap()
}

fn transaction(nth: usize) -> (Transaction, Value) {
    let said = answer("eth_getTransactionByHash", nth);
    (nitro::transaction(B256::ZERO).answer(&said).unwrap().unwrap(), result(&said))
}

fn receipt(nth: usize) -> (Receipt, Value) {
    let said = answer("eth_getTransactionReceipt", nth);
    (nitro::receipt(B256::ZERO).answer(&said).unwrap().unwrap(), result(&said))
}

#[test]
fn a_block_names_its_parent_block_send_count_and_root() {
    let said = answer("eth_getBlockByNumber", 0);
    let raw = result(&said);
    let block = nitro::block(BlockTag::Latest, false).answer(&said).unwrap().unwrap();
    assert_eq!((block.number, block.hash.to_hex()), (0xee63c6, text(&raw["hash"]).to_owned()));
    assert_eq!(block.l1_block_number, quantity(&raw["l1BlockNumber"]));
    assert_eq!(block.send_count, quantity(&raw["sendCount"]));
    assert_eq!(block.send_root.to_hex(), text(&raw["sendRoot"]));
    assert_eq!(block.send_root.to_hex(), text(&raw["extraData"]), "Nitro keeps the send root in the extra data");
    assert_eq!(block.arbos_version(), 61, "the mix hash's third eight bytes");
    assert_eq!(block.gas_limit, 0x4_0000_0000_0000, "not the real limit: ArbGasInfo's is 32 million");
    assert_eq!(block.base_fee_per_gas, Some(U256::from_u64(0x5f62f20)));
    let BlockTransactions::Hashes(hashes) = &block.transactions else { panic!("hashes were asked for") };
    assert_eq!(hashes.len(), raw["transactions"].as_array().unwrap().len());

    let whole = nitro::block(BlockTag::Latest, true).answer(&answer("eth_getBlockByNumber", 1)).unwrap().unwrap();
    let BlockTransactions::Full(transactions) = &whole.transactions else { panic!("whole transactions") };
    let kinds: Vec<TxKind> = transactions.iter().map(Transaction::tx_kind).collect();
    assert_eq!(kinds[0], TxKind::Internal, "ArbOS opens every block");
    assert!(kinds.contains(&TxKind::Legacy) && kinds.contains(&TxKind::DynamicFee), "{kinds:?}");
    assert_eq!(transactions.iter().map(|one| one.hash).collect::<Vec<_>>(), *hashes);
    let by_hash = nitro::block_by_hash(block.hash, false).answer(&answer("eth_getBlockByHash", 0)).unwrap().unwrap();
    assert_eq!(by_hash, block);
}

#[test]
fn each_transaction_type_reads_with_its_own_fields() {
    let (internal, raw) = transaction(0);
    assert_eq!((internal.tx_kind(), internal.from, internal.gas), (TxKind::Internal, crate::nitro::ARBOS, 0));
    assert_eq!(internal.to, Some(crate::nitro::ARBOS));
    assert_eq!(internal.block_timestamp, Some(quantity(&raw["blockTimestamp"])));

    let (legacy, raw) = transaction(1);
    assert_eq!(legacy.tx_kind(), TxKind::Legacy);
    assert_eq!(legacy.v, Some(U256::from_u64(quantity(&raw["v"]))));
    assert_eq!(legacy.v.and_then(U256::to_u64).map(|v| (v - 35) / 2), Some(99_801), "EIP-155 on Elysium's chain");
    assert_eq!(legacy.to_eip1559(), None);

    let (dynamic, raw) = transaction(2);
    assert_eq!((dynamic.tx_kind(), dynamic.chain_id), (TxKind::DynamicFee, Some(99_801)));
    assert_eq!(dynamic.max_priority_fee_per_gas, Some(U256::ZERO), "no tip on Elysium");
    let rebuilt = dynamic.to_eip1559().unwrap();
    let signature = dynamic.signature().unwrap();
    assert_eq!(tx_hash(&rebuilt.signed(&signature).unwrap()).to_hex(), text(&raw["hash"]));

    let (deposit, raw) = transaction(3);
    assert_eq!(deposit.tx_kind(), TxKind::Deposit);
    assert_eq!(deposit.request_id.map(|id| id.to_hex()).as_deref(), raw["requestId"].as_str());
    assert_eq!(deposit.message_number().and_then(U256::to_u64), Some(1_493_518));
    assert_eq!(deposit.value, text(&raw["value"]).parse().unwrap());

    let (submit, raw) = transaction(4);
    assert_eq!(submit.tx_kind(), TxKind::SubmitRetryable);
    assert_eq!(submit.message_number().and_then(U256::to_u64), Some(1_493_519));
    assert_eq!(submit.beneficiary.map(|address| address.to_hex()).as_deref(), raw["beneficiary"].as_str());
    assert_eq!(submit.deposit_value, Some(text(&raw["depositValue"]).parse().unwrap()));
    assert_eq!(submit.max_submission_fee, Some(text(&raw["maxSubmissionFee"]).parse().unwrap()));
    assert_eq!(submit.retry_to.map(|address| address.to_hex()).as_deref(), raw["retryTo"].as_str());
    assert!(submit.retry_data.as_ref().is_some_and(|data| !data.is_empty()));

    let (retry, raw) = transaction(5);
    assert_eq!(retry.tx_kind(), TxKind::Retry);
    assert_eq!(retry.ticket_id, Some(submit.hash), "a redeem names its ticket, the submission's hash");
    assert_eq!(retry.max_refund, Some(text(&raw["maxRefund"]).parse().unwrap()));
    assert_eq!(retry.submission_fee_refund, Some(text(&raw["submissionFeeRefund"]).parse().unwrap()));
    assert!(TxKind::Retry.is_nitro() && !TxKind::DynamicFee.is_nitro());
    for byte in [0x00, 0x01, 0x02, 0x04, 0x64, 0x65, 0x66, 0x68, 0x69, 0x6a, 0x7e] {
        assert_eq!(TxKind::from_byte(byte).byte(), byte);
    }
}

#[test]
fn a_receipt_says_what_its_gas_paid_for_hyperevm() {
    let (internal, _) = receipt(0);
    assert_eq!((internal.gas_used, internal.gas_used_for_l1, internal.fee()), (0, 0, Some(U256::ZERO)));
    for nth in 1..=5 {
        let (receipt, raw) = receipt(nth);
        assert_eq!(receipt.gas_used, quantity(&raw["gasUsed"]));
        assert_eq!(receipt.gas_used_for_l1, quantity(&raw["gasUsedForL1"]));
        assert_eq!(receipt.l1_block_number, Some(quantity(&raw["l1BlockNumber"])));
        assert_eq!(receipt.gas_used_for_l2(), receipt.gas_used - receipt.gas_used_for_l1);
        assert_eq!(receipt.tx_kind().map(TxKind::byte), Some(u8::try_from(quantity(&raw["type"])).unwrap()));
        assert!(receipt.succeeded(), "{nth}");
        let price: U256 = text(&raw["effectiveGasPrice"]).parse().unwrap();
        assert_eq!(receipt.fee(), price.checked_mul(U256::from_u64(receipt.gas_used)));
        assert_eq!(receipt.logs.len(), raw["logs"].as_array().unwrap().len());
    }
    let (legacy, _) = receipt(1);
    assert!(legacy.gas_used_for_l1 > 0, "a user's transaction pays for HyperEVM's data");
}
