//! Real transactions on Elysium testnet, run by hand with `just send`: from the key in
//! `ELYSIUM_TEST_KEY` (testnet only), each with no value and a few tens of thousands of gas. A
//! self transfer answered with its receipt, two sends back to back on the kept nonce, and a
//! contract created both ways, its code found where the crate says it lands. A test without the
//! key, or with nothing to pay gas, says so and does nothing.

// A test crate: clippy's allowance for tests covers `#[test]` functions only, not their helpers.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use df_elysium_core::account::LocalKey;
use df_elysium_core::client::native::Native;
use df_elysium_core::client::{Receipt, create_address};
use df_elysium_core::utils::keccak256;
use df_elysium_core::{B256, Client, Network, U256, deploy, transfer};

/// Copies one byte of code, `STOP`: the smallest contract there is.
const INIT_CODE: [u8; 12] = [0x60, 0x01, 0x80, 0x60, 0x0b, 0x60, 0x00, 0x39, 0x60, 0x00, 0xf3, 0x00];

/// A sender on testnet, or why there is none.
async fn sender() -> Option<Client<Native, LocalKey>> {
    let Ok(secret) = std::env::var("ELYSIUM_TEST_KEY") else {
        println!("ELYSIUM_TEST_KEY is not set: nothing sent");
        return None;
    };
    let client = Client::new(Native::new(), Network::Testnet, LocalKey::from_hex(&secret).unwrap());
    let held = client.balance(client.address()).await.unwrap();
    if held.is_zero() {
        println!("{} holds no HYPE on Elysium testnet: nothing sent", client.address());
        return None;
    }
    Some(client)
}

/// The receipt of `hash`, once it is in a block: a second or two at most.
async fn landed(client: &Client<Native, LocalKey>, hash: B256) -> Receipt {
    for _ in 0..50 {
        if let Some(receipt) = client.receipt(hash).await.unwrap() {
            return receipt;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    panic!("{hash} did not land in ten seconds");
}

#[tokio::test]
#[ignore = "sends"]
async fn a_self_transfer_answers_its_receipt_and_costs_what_it_says() {
    let Some(client) = sender().await else { return };
    let me = client.address();
    let before = client.balance(me).await.unwrap();
    let receipt = client.send_sync(transfer::hype_wei(me, U256::ZERO), Some(5_000)).await.unwrap();
    assert!(receipt.succeeded(), "{receipt:?}");
    assert!(receipt.gas_used > 21_000 && receipt.gas_used_for_l1 > 0, "HyperEVM's data is paid for: {receipt:?}");
    let fee = receipt.fee().unwrap();
    let after = client.balance(me).await.unwrap();
    assert_eq!(before.checked_sub(after), Some(fee), "the balance falls by the receipt's fee, exactly");
    println!(
        "{}: {} gas, {} of it HyperEVM's data",
        Network::Testnet.tx_url(receipt.transaction_hash),
        receipt.gas_used,
        receipt.gas_used_for_l1
    );
}

#[tokio::test]
#[ignore = "sends"]
async fn two_sends_back_to_back_take_consecutive_nonces_from_the_kept_one() {
    let Some(client) = sender().await else { return };
    let me = client.address();
    let first = client.prepare(transfer::hype_wei(me, U256::ZERO)).await.unwrap();
    let first_hash = client.send_prepared(&first).await.unwrap();
    let second = client.prepare(transfer::hype_wei(me, U256::ZERO)).await.unwrap();
    assert_eq!(second.nonce, first.nonce + 1, "kept, not read again");
    let second_hash = client.send_prepared(&second).await.unwrap();
    let (first, second) = (landed(&client, first_hash).await, landed(&client, second_hash).await);
    assert!(first.succeeded() && second.succeeded());
    assert!(first.block_number <= second.block_number, "in order");
    assert_eq!(client.nonce(me).await.unwrap(), client.nonces().kept().unwrap(), "the node agrees with the kept nonce");
}

#[tokio::test]
#[ignore = "sends"]
async fn a_contract_created_either_way_is_where_the_crate_says() {
    let Some(client) = sender().await else { return };
    let me = client.address();

    let creation = client.prepare(deploy::create(&INIT_CODE, &[])).await.unwrap();
    let receipt = client.send_prepared_sync(&creation, None).await.unwrap();
    assert!(receipt.succeeded(), "{receipt:?}");
    let at = create_address(me, creation.nonce);
    assert_eq!(receipt.contract_address, Some(at));
    assert_eq!(client.code(at).await.unwrap(), [0x00], "STOP, deployed");

    // A salt of its own each run: CREATE2 lands once per salt.
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let salt = B256(keccak256(&[me.bytes().as_slice(), &now.to_be_bytes()].concat()));
    let receipt = client.send_sync(deploy::create2(salt, &INIT_CODE), None).await.unwrap();
    assert!(receipt.succeeded(), "{receipt:?}");
    let at = deploy::create2_address(deploy::CREATE2_DEPLOYER, salt, deploy::init_code_hash(&INIT_CODE));
    assert_eq!(client.code(at).await.unwrap(), [0x00], "at the CREATE2 address, whoever sent it");
    println!("{}", Network::Testnet.address_url(at));
}
