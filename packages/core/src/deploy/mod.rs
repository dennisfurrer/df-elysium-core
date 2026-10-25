//! Contracts on Elysium: a creation and where it lands (CREATE from the sender's nonce, CREATE2
//! through the deterministic deployer at its usual address), Stylus programs activated after
//! they are deployed ([`crate::nitro::wasm`]), and source verification on Conduit's Sourcify
//! ([`verify`]). CreateX is not deployed.
//!
//! Elysium runs current EVM semantics: Kinetiq's own contracts verify with solc 0.8.37,
//! `evmVersion` osaka.

pub mod verify;

use crate::client::Call;
use crate::utils::{Address, keccak256};
use crate::{B256, U256};

pub use crate::nitro::wasm::{activate_program, activation_quote};
pub use df_hyperliquid_core::client::evm::tx::create_address;

/// The deterministic deployer (Arachnid's), predeployed at its usual address: it creates
/// `salt || init code` with CREATE2.
pub const CREATE2_DEPLOYER: Address = match Address::parse_const("0x4e59b44847b379578588920ca78fbf26c0b4956c") {
    Some(address) => address,
    None => Address::ZERO,
};

/// A creation: the init code with its ABI-encoded constructor arguments after it. The contract
/// lands at [`create_address`] of the sender and its nonce.
#[must_use]
pub fn create(init_code: &[u8], constructor_args: &[u8]) -> Call {
    let mut data = init_code.to_vec();
    data.extend_from_slice(constructor_args);
    Call { to: None, value: U256::ZERO, data }
}

/// A creation through [`CREATE2_DEPLOYER`], landing at [`create2_address`] whoever sends it.
#[must_use]
pub fn create2(salt: B256, init_code: &[u8]) -> Call {
    let mut data = salt.0.to_vec();
    data.extend_from_slice(init_code);
    Call { to: Some(CREATE2_DEPLOYER), value: U256::ZERO, data }
}

/// Where CREATE2 puts a contract: `keccak256(0xff || deployer || salt || keccak256(init code))`,
/// last twenty bytes.
#[must_use]
pub fn create2_address(deployer: Address, salt: B256, init_code_hash: B256) -> Address {
    let mut preimage = Vec::with_capacity(85);
    preimage.push(0xff);
    preimage.extend_from_slice(deployer.bytes());
    preimage.extend_from_slice(&salt.0);
    preimage.extend_from_slice(&init_code_hash.0);
    let hash = keccak256(&preimage);
    let mut bytes = [0u8; 20];
    bytes.copy_from_slice(&hash[12..]);
    Address::from_bytes(bytes)
}

/// The hash CREATE2 commits to.
#[must_use]
pub fn init_code_hash(init_code: &[u8]) -> B256 {
    B256(keccak256(init_code))
}

#[cfg(test)]
#[path = "tests/deploy.rs"]
mod tests;
