//! Sending through a signing [`Client`] against the fake node, which answers estimates, the gas
//! price and the nonce as Elysium testnet did and takes raw transactions as a node does: the
//! transaction is the node's estimate with headroom, a fee cap of twice the base fee and no tip,
//! on chain 99801; the nonce is read once and kept; what the node was sent recovers to the key;
//! a refused or lost send forgets the nonce, and nothing is ever sent twice.

use std::future::Future;

use df_elysium_core::account::{LocalKey, SignError, Signature, Signer};
use df_elysium_core::client::{RpcError, SendError, TransportError, tx_hash, with_headroom};
use df_elysium_core::utils::Address;
use df_elysium_core::{B256, Client, Dec, Network, U256, transfer};
use serde_json::json;

use crate::support::node::{Chain, Node};
use crate::support::{address, answer, call, key};

const GUARDIAN: &str = "0x8499e50474b6e8c988b232f52583439abf37a8a1";

/// A node that estimates, prices and counts as testnet did: 21,174 gas, base fee 0x5f71980,
/// nonce 0x5f.
fn node() -> Node {
    Node::testnet()
        .answer(Chain::Elysium, "eth_estimateGas", answer(&call("eth_estimateGas", 0)))
        .answer(Chain::Elysium, "eth_gasPrice", answer(&call("eth_gasPrice", 0)))
        .answer(Chain::Elysium, "eth_getTransactionCount", answer(&call("eth_getTransactionCount", 0)))
}

fn count(client: &Client<Node, LocalKey>, method: &str) -> usize {
    client.http().methods(Chain::Elysium).iter().filter(|said| *said == method).count()
}

#[tokio::test]
async fn a_transfer_is_estimated_priced_signed_and_sent_once_and_its_nonce_kept() {
    let client = Client::new(node(), Network::Testnet, key());
    let me = key().address();
    let transfer = transfer::hype(address(GUARDIAN), Dec::parse("0.001").unwrap()).unwrap();
    let tx = client.prepare(transfer.clone()).await.unwrap();
    assert_eq!((tx.chain_id, tx.nonce), (99_801, 0x5f), "the network's chain and the pending nonce");
    assert_eq!(tx.gas, with_headroom(0x52b6), "the node's estimate and thirty percent");
    assert_eq!(tx.gas, 27_526);
    let base = U256::from(0x5f7_1980u64);
    assert_eq!((tx.max_priority_fee, tx.max_fee), (U256::ZERO, U256::from(2 * 0x5f7_1980u64)), "no tip, twice {base}");
    assert_eq!(
        (tx.to, tx.value, tx.data.as_slice()),
        (Some(address(GUARDIAN)), U256::from(1_000_000_000_000_000u64), &[][..])
    );
    let estimated = &client.http().calls(Chain::Elysium)[0];
    assert_eq!(estimated["params"], json!([{"from": me.to_hex(), "to": GUARDIAN, "value": "0x38d7ea4c68000"}]));

    let hash = client.send_prepared(&tx).await.unwrap();
    let raws = client.http().raws();
    let [raw] = raws.as_slice() else { panic!("one raw transaction: {raws:?}") };
    assert_eq!(raw, &tx.sign(&key()).unwrap(), "the bytes the key signs");
    assert_eq!(hash, tx_hash(raw), "the node answers with the transaction's hash");
    let signature = key().sign_digest(&tx.signing_hash().0).unwrap();
    assert_eq!(tx.signer(&signature), Ok(me));

    // The next send keeps the nonce: one past the last, and the node is not asked again.
    let next = client.prepare(transfer.clone()).await.unwrap();
    assert_eq!(next.nonce, 0x60);
    client.send(transfer).await.unwrap();
    assert_eq!(count(&client, "eth_getTransactionCount"), 1, "read once");
    assert_eq!(count(&client, "eth_sendRawTransaction"), 2);
    assert_eq!(client.nonces().kept(), Some(0x61));
}

#[tokio::test]
async fn a_synchronous_send_answers_with_the_receipt_of_the_transaction_sent() {
    let receipt = call("eth_getTransactionReceipt", 2);
    let node = node().sync_receipt(answer(&receipt));
    let client = Client::new(node, Network::Testnet, key());
    let sent = client.send_sync(transfer::hype_wei(address(GUARDIAN), U256::ONE), Some(2_000)).await.unwrap();
    let raws = client.http().raws();
    assert_eq!(sent.transaction_hash, tx_hash(&raws[0]));
    assert!(sent.succeeded());
    assert_eq!((sent.gas_used, sent.gas_used_for_l1), (0x11edd, 8), "the receipt as the node wrote it");
    let call = &client.http().calls(Chain::Elysium)[3];
    assert_eq!(call["method"], "eth_sendRawTransactionSync");
    assert_eq!(call["params"][1], 2000, "the timeout in milliseconds, as a number: Nitro refuses a quantity");
    assert_eq!(client.nonces().kept(), Some(0x60));
}

#[tokio::test]
async fn a_refused_or_lost_send_forgets_the_nonce_and_nothing_is_sent_twice() {
    let too_low = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"nonce too low"}}"#;
    let node = node()
        .status(Chain::Elysium, Some("eth_sendRawTransaction"), 200, too_low)
        .unreachable(Chain::Elysium, Some("eth_sendRawTransaction"));
    let client = Client::new(node, Network::Testnet, key());
    let call = transfer::hype_wei(address(GUARDIAN), U256::ONE);

    let refused = client.send(call.clone()).await;
    let Err(SendError::Rpc(RpcError::Node { method: "eth_sendRawTransaction", error })) = refused else {
        panic!("{refused:?}")
    };
    assert_eq!(error.message, "nonce too low");
    assert_eq!(client.nonces().kept(), None, "forgotten");

    let lost = client.send(call.clone()).await;
    assert!(matches!(lost, Err(SendError::Rpc(RpcError::Transport(TransportError::Unreachable(_))))), "{lost:?}");
    assert_eq!(client.nonces().kept(), None, "it may have landed: the node's count says");

    client.send(call).await.unwrap();
    assert_eq!(count(&client, "eth_getTransactionCount"), 3, "read again after each failure");
    assert_eq!(count(&client, "eth_sendRawTransaction"), 3, "each sent once, never retried");
}

/// Claims the key's address, signs with another.
struct Impostor {
    claims: Address,
    signs: LocalKey,
}

impl Signer for Impostor {
    fn address(&self) -> Address {
        self.claims
    }

    fn sign_hash(&self, hash: &B256) -> impl Future<Output = Result<Signature, SignError>> {
        std::future::ready(self.signs.sign_digest(&hash.0))
    }
}

#[tokio::test]
async fn a_signature_from_another_key_is_refused_before_anything_is_sent() {
    let other = LocalKey::from_hex("0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d").unwrap();
    let impostor = Impostor { claims: key().address(), signs: other };
    let signed_by = impostor.signs.address();
    let client = Client::new(node(), Network::Testnet, impostor);
    let refused = client.send(transfer::hype_wei(address(GUARDIAN), U256::ONE)).await;
    assert_eq!(
        refused,
        Err(SendError::Sign(SignError::WrongSigner { expected: key().address(), recovered: signed_by }))
    );
    assert!(client.http().raws().is_empty(), "nothing went out");
}
