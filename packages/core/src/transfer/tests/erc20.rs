//! ERC-20 calldata against viem's `encodeFunctionData` (`arbitrum_sdk.json`), and amounts at a
//! token's decimals, exact or refused.

use crate::test_support::{arbitrum_sdk, text};

use super::*;

fn viem(name: &str) -> (Vec<String>, String) {
    let file = arbitrum_sdk();
    let case = file["calldata"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["contract"] == "ERC20" && text(&case["signature"]).contains(&format!(" {name}(")))
        .unwrap_or_else(|| panic!("no ERC20 {name}"));
    let args = case["args"].as_array().unwrap().iter().map(|arg| text(arg).to_owned()).collect();
    (args, text(&case["calldata"]).to_owned())
}

fn hex(bytes: &[u8]) -> String {
    crate::utils::hex::encode_prefixed(bytes)
}

#[test]
fn each_call_writes_viems_calldata() {
    let token = Address::from_bytes([0x77; 20]);
    let (args, calldata) = viem("allowance");
    let read = allowance(token, args[0].parse().unwrap(), args[1].parse().unwrap());
    assert_eq!((read.call.to, hex(&read.call.data)), (Some(token), calldata));
    let (args, calldata) = viem("transferFrom");
    let call = transfer_from(token, args[0].parse().unwrap(), args[1].parse().unwrap(), args[2].parse().unwrap());
    assert_eq!(hex(&call.data), calldata);
    let (_, calldata) = viem("totalSupply");
    assert_eq!(hex(&total_supply(token).call.data), calldata);
    let (_, calldata) = viem("name");
    assert_eq!(hex(&name(token).call.data), calldata);
    // df-hyperliquid-core's calldata, named by this crate's methods.
    let owner = Address::from_bytes([0x22; 20]);
    assert_eq!(balance(token, owner).call.data[..4], BALANCE_OF.selector());
    assert_eq!(transfer(token, owner, U256::ONE).data[..4], TRANSFER.selector());
    assert_eq!(approve(token, owner, U256::ONE).data[..4], APPROVE.selector());
    assert_eq!(decimals(token).call.data, DECIMALS.selector());
    assert_eq!(symbol(token).call.data, SYMBOL.selector());
}

#[test]
fn answers_read_and_amounts_convert_at_the_tokens_decimals() {
    let word = |value: u64| U256::from_u64(value).to_be_bytes().to_vec();
    assert_eq!(balance(Address::ZERO, Address::ZERO).decode(&word(12_500_000)), Ok(U256::from_u64(12_500_000)));
    assert_eq!(decimals(Address::ZERO).decode(&word(6)), Ok(6));
    assert!(decimals(Address::ZERO).decode(&word(256)).is_err(), "a uint8 holds no 256");
    assert_eq!(amount(U256::from_u64(12_500_000), 6), Ok(Dec::parse("12.5").unwrap()));
    assert_eq!(units(Dec::parse("12.5").unwrap(), 6), Ok(U256::from_u64(12_500_000)));
    assert!(units(Dec::parse("0.0000001").unwrap(), 6).is_err(), "a seventh place is refused, never rounded");
}
