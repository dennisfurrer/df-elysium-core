//! Calldata against viem's `encodeFunctionData` (`arbitrum_sdk.json`), and against
//! df-hyperliquid-core's own ABI encoder over the same values: static words, `bytes` and
//! `bytes32[]` in any place, empty and past a word.

use df_hyperliquid_core::client::evm::abi::{self, Function, Token};

use crate::test_support::{arbitrum_sdk, bytes, text};

use super::*;

/// The same call through df-hyperliquid-core's encoder, which shares nothing with ours.
fn reference(signature: &str, tokens: &[Token]) -> Vec<u8> {
    Function::parse(signature).unwrap().encode(tokens).unwrap()
}

#[test]
fn a_selector_is_the_signature_hashed_as_viem_and_df_hyperliquid_core_hash_it() {
    let file = arbitrum_sdk();
    for case in file["calldata"].as_array().unwrap() {
        let declared = text(&case["signature"]).trim_start_matches("function ");
        let canonical = Function::parse(declared).unwrap().signature();
        let method = Method::new(Box::leak(canonical.into_boxed_str()), "");
        assert_eq!(crate::utils::hex::encode_prefixed(&method.selector()), text(&case["selector"]), "{declared}");
        assert_eq!(method.selector(), abi::selector(declared).unwrap());
    }
}

#[test]
fn every_argument_kind_lands_where_abi_encode_puts_it() {
    let method = Method::new("f(address,bytes,uint256,bytes32[],bool,bytes32)", "");
    let long: Vec<u8> = (0..70).collect();
    let words = [B256([0x11; 32]), B256([0x22; 32])];
    for (data, proof) in [(&[][..], &[][..]), (&[0xab][..], &words[..1]), (&long[..], &words[..])] {
        let ours = method.calldata(&[
            Arg::Address(Address::from_bytes([0x33; 20])),
            Arg::Bytes(data),
            Arg::u64(7),
            Arg::Words(proof),
            Arg::Bool(true),
            Arg::Word(B256([0x44; 32])),
        ]);
        let theirs = reference(
            method.signature,
            &[
                Token::Address(Address::from_bytes([0x33; 20])),
                Token::Bytes(data.to_vec()),
                Token::uint(7u64),
                Token::Array(proof.iter().map(|word| Token::Fixed(word.0.to_vec())).collect()),
                Token::Bool(true),
                Token::Fixed(vec![0x44; 32]),
            ],
        );
        assert_eq!(ours, theirs, "{} bytes, {} words", data.len(), proof.len());
    }
    assert_eq!(Method::new("f()", "").calldata(&[]), Method::new("f()", "").selector());
}

#[test]
fn an_answer_reads_against_what_the_method_returns() {
    let file = arbitrum_sdk();
    let prices = file["answers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["signature"].as_str().is_some_and(|s| s.contains("getPricesInWei")))
        .unwrap();
    let method = Method::new("getPricesInWei()", "uint256, uint256, uint256, uint256, uint256, uint256");
    let tokens = method.answer(&bytes(&prices["data"])).unwrap();
    assert_eq!(tokens.len(), 6);
    assert!(Method::new("f()", "uint64").answer(&[0xff; 32]).is_err(), "a word wider than its type");
    assert!(Method::new("f()", "uint7").answer(&[0; 32]).is_err(), "not a type");
}
