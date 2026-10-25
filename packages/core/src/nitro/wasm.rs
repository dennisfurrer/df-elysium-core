//! Stylus: ArbWasm at `0x71` and ArbWasmCache at `0x72`. A WebAssembly program is deployed like
//! any contract, then activated once ([`activate_program`]), paying a data fee in HYPE that the
//! activation's own `eth_call` quotes; it then runs until it expires, unless kept alive. The
//! parameters (ink price, page costs, lifetimes) are the chain's, read here.

use df_hyperliquid_core::client::evm::abi::Token;

use crate::client::read::{Fields, one};
use crate::client::{Arg, Call, Event, Method, Read};
use crate::nitro::table::{ARB_WASM, ARB_WASM_CACHE};
use crate::utils::Address;
use crate::{B256, U256};

pub const ACTIVATE_PROGRAM: Method = Method::new("activateProgram(address)", "uint16 version, uint256 dataFee");
pub const STYLUS_VERSION: Method = Method::new("stylusVersion()", "uint16 version");
pub const CODEHASH_VERSION: Method = Method::new("codehashVersion(bytes32)", "uint16 version");
pub const CODEHASH_KEEPALIVE: Method = Method::new("codehashKeepalive(bytes32)", "");
pub const CODEHASH_ASM_SIZE: Method = Method::new("codehashAsmSize(bytes32)", "uint32 size");
pub const PROGRAM_VERSION: Method = Method::new("programVersion(address)", "uint16 version");
pub const PROGRAM_INIT_GAS: Method = Method::new("programInitGas(address)", "uint64 gas, uint64 gasWhenCached");
pub const PROGRAM_MEMORY_FOOTPRINT: Method = Method::new("programMemoryFootprint(address)", "uint16 footprint");
pub const PROGRAM_TIME_LEFT: Method = Method::new("programTimeLeft(address)", "uint64 secs");
pub const INK_PRICE: Method = Method::new("inkPrice()", "uint32 price");
pub const MAX_STACK_DEPTH: Method = Method::new("maxStackDepth()", "uint32 depth");
pub const FREE_PAGES: Method = Method::new("freePages()", "uint16 pages");
pub const PAGE_GAS: Method = Method::new("pageGas()", "uint16 gas");
pub const PAGE_RAMP: Method = Method::new("pageRamp()", "uint64 ramp");
pub const PAGE_LIMIT: Method = Method::new("pageLimit()", "uint16 limit");
pub const MIN_INIT_GAS: Method = Method::new("minInitGas()", "uint64 gas, uint64 cached");
pub const INIT_COST_SCALAR: Method = Method::new("initCostScalar()", "uint64 percent");
pub const EXPIRY_DAYS: Method = Method::new("expiryDays()", "uint16 days");
pub const KEEPALIVE_DAYS: Method = Method::new("keepaliveDays()", "uint16 days");
pub const BLOCK_CACHE_SIZE: Method = Method::new("blockCacheSize()", "uint16 count");
pub const ACTIVATION_GAS: Method = Method::new("activationGas()", "uint64 gas");

pub const IS_CACHE_MANAGER: Method = Method::new("isCacheManager(address)", "bool");
pub const ALL_CACHE_MANAGERS: Method = Method::new("allCacheManagers()", "address[] managers");
pub const CACHE_PROGRAM: Method = Method::new("cacheProgram(address)", "");
pub const EVICT_CODEHASH: Method = Method::new("evictCodehash(bytes32)", "");
pub const CODEHASH_IS_CACHED: Method = Method::new("codehashIsCached(bytes32)", "bool");

/// Every method of ArbWasm and ArbWasmCache this crate calls.
pub const METHODS: &[Method] = &[
    ACTIVATE_PROGRAM,
    STYLUS_VERSION,
    CODEHASH_VERSION,
    CODEHASH_KEEPALIVE,
    CODEHASH_ASM_SIZE,
    PROGRAM_VERSION,
    PROGRAM_INIT_GAS,
    PROGRAM_MEMORY_FOOTPRINT,
    PROGRAM_TIME_LEFT,
    INK_PRICE,
    MAX_STACK_DEPTH,
    FREE_PAGES,
    PAGE_GAS,
    PAGE_RAMP,
    PAGE_LIMIT,
    MIN_INIT_GAS,
    INIT_COST_SCALAR,
    EXPIRY_DAYS,
    KEEPALIVE_DAYS,
    BLOCK_CACHE_SIZE,
    ACTIVATION_GAS,
    IS_CACHE_MANAGER,
    ALL_CACHE_MANAGERS,
    CACHE_PROGRAM,
    EVICT_CODEHASH,
    CODEHASH_IS_CACHED,
];

pub const PROGRAM_ACTIVATED: Event = Event::new(
    "ProgramActivated(bytes32,bytes32,address,uint256,uint16)",
    "bytes32 indexed codehash, bytes32 moduleHash, address program, uint256 dataFee, uint16 version",
);
pub const PROGRAM_LIFETIME_EXTENDED: Event =
    Event::new("ProgramLifetimeExtended(bytes32,uint256)", "bytes32 indexed codehash, uint256 dataFee");

/// What an activation answers: the Stylus version it compiled for, and the data fee it charged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Activation {
    pub version: u16,
    /// In wei of HYPE.
    pub data_fee: U256,
}

/// Activates the program at `program`, paying `data_fee` wei: what [`activation_quote`] answers.
/// Any HYPE over the fee is refunded.
#[must_use]
pub fn activate_program(program: Address, data_fee: U256) -> Call {
    Call { to: Some(ARB_WASM), value: data_fee, data: ACTIVATE_PROGRAM.calldata(&[Arg::Address(program)]) }
}

/// The activation simulated: the version and data fee it would charge now. Sent with value, so
/// the fee check passes; `funds` must cover it.
#[must_use]
pub fn activation_quote(program: Address, funds: U256) -> Read<Activation> {
    Read::new(ARB_WASM, &ACTIVATE_PROGRAM, &[Arg::Address(program)], |answer| {
        let tokens = ACTIVATE_PROGRAM.answer(answer)?;
        let mut fields = Fields::of("activateProgram", &tokens);
        Ok(Activation { version: fields.u16()?, data_fee: fields.uint()? })
    })
    .with_value(funds)
}

/// Pays to keep a program's code activated a lifetime longer.
#[must_use]
pub fn codehash_keepalive(codehash: B256, data_fee: U256) -> Call {
    Call { to: Some(ARB_WASM), value: data_fee, data: CODEHASH_KEEPALIVE.calldata(&[Arg::Word(codehash)]) }
}

/// The Stylus version programs are compiled for now.
#[must_use]
pub fn stylus_version() -> Read<u16> {
    Read::new(ARB_WASM, &STYLUS_VERSION, &[], |answer| one(&STYLUS_VERSION, answer, Token::as_u16))
}

/// The version a program was activated at; reverts for one that is not.
#[must_use]
pub fn program_version(program: Address) -> Read<u16> {
    Read::new(ARB_WASM, &PROGRAM_VERSION, &[Arg::Address(program)], |answer| {
        one(&PROGRAM_VERSION, answer, Token::as_u16)
    })
}

#[must_use]
pub fn codehash_version(codehash: B256) -> Read<u16> {
    Read::new(ARB_WASM, &CODEHASH_VERSION, &[Arg::Word(codehash)], |answer| {
        one(&CODEHASH_VERSION, answer, Token::as_u16)
    })
}

/// The gas a program takes to start, cold and cached.
#[must_use]
pub fn program_init_gas(program: Address) -> Read<(u64, u64)> {
    Read::new(ARB_WASM, &PROGRAM_INIT_GAS, &[Arg::Address(program)], |answer| {
        let tokens = PROGRAM_INIT_GAS.answer(answer)?;
        let mut fields = Fields::of("programInitGas", &tokens);
        Ok((fields.u64()?, fields.u64()?))
    })
}

/// Seconds until a program expires.
#[must_use]
pub fn program_time_left(program: Address) -> Read<u64> {
    Read::new(ARB_WASM, &PROGRAM_TIME_LEFT, &[Arg::Address(program)], |answer| {
        one(&PROGRAM_TIME_LEFT, answer, Token::as_u64)
    })
}

/// The chain's Stylus parameters, each its own read.
#[must_use]
pub fn ink_price() -> Read<u32> {
    Read::new(ARB_WASM, &INK_PRICE, &[], |answer| one(&INK_PRICE, answer, Token::as_u32))
}

#[must_use]
pub fn max_stack_depth() -> Read<u32> {
    Read::new(ARB_WASM, &MAX_STACK_DEPTH, &[], |answer| one(&MAX_STACK_DEPTH, answer, Token::as_u32))
}

#[must_use]
pub fn free_pages() -> Read<u16> {
    Read::new(ARB_WASM, &FREE_PAGES, &[], |answer| one(&FREE_PAGES, answer, Token::as_u16))
}

#[must_use]
pub fn page_gas() -> Read<u16> {
    Read::new(ARB_WASM, &PAGE_GAS, &[], |answer| one(&PAGE_GAS, answer, Token::as_u16))
}

#[must_use]
pub fn page_limit() -> Read<u16> {
    Read::new(ARB_WASM, &PAGE_LIMIT, &[], |answer| one(&PAGE_LIMIT, answer, Token::as_u16))
}

/// Days a program lives after activation, and days before a keepalive may extend it.
#[must_use]
pub fn expiry_days() -> Read<u16> {
    Read::new(ARB_WASM, &EXPIRY_DAYS, &[], |answer| one(&EXPIRY_DAYS, answer, Token::as_u16))
}

#[must_use]
pub fn keepalive_days() -> Read<u16> {
    Read::new(ARB_WASM, &KEEPALIVE_DAYS, &[], |answer| one(&KEEPALIVE_DAYS, answer, Token::as_u16))
}

#[must_use]
pub fn block_cache_size() -> Read<u16> {
    Read::new(ARB_WASM, &BLOCK_CACHE_SIZE, &[], |answer| one(&BLOCK_CACHE_SIZE, answer, Token::as_u16))
}

/// The addresses allowed to cache and evict programs.
#[must_use]
pub fn cache_managers() -> Read<Vec<Address>> {
    Read::new(ARB_WASM_CACHE, &ALL_CACHE_MANAGERS, &[], |answer| {
        let tokens = ALL_CACHE_MANAGERS.answer(answer)?;
        Fields::of("allCacheManagers", &tokens).addresses()
    })
}

#[must_use]
pub fn is_cache_manager(address: Address) -> Read<bool> {
    Read::new(ARB_WASM_CACHE, &IS_CACHE_MANAGER, &[Arg::Address(address)], |answer| {
        one(&IS_CACHE_MANAGER, answer, Token::as_bool)
    })
}

#[must_use]
pub fn codehash_is_cached(codehash: B256) -> Read<bool> {
    Read::new(ARB_WASM_CACHE, &CODEHASH_IS_CACHED, &[Arg::Word(codehash)], |answer| {
        one(&CODEHASH_IS_CACHED, answer, Token::as_bool)
    })
}

/// Caches a program, so it starts at the cached init gas; a cache manager's alone.
#[must_use]
pub fn cache_program(program: Address) -> Call {
    Call { to: Some(ARB_WASM_CACHE), value: U256::ZERO, data: CACHE_PROGRAM.calldata(&[Arg::Address(program)]) }
}

/// Evicts a program's code from the cache; a cache manager's alone.
#[must_use]
pub fn evict_codehash(codehash: B256) -> Call {
    Call { to: Some(ARB_WASM_CACHE), value: U256::ZERO, data: EVICT_CODEHASH.calldata(&[Arg::Word(codehash)]) }
}

#[cfg(test)]
#[path = "tests/wasm.rs"]
mod tests;
