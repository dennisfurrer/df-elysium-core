# Changelog

All notable changes to df-elysium-core.

## 0.0.3-beta

- Depends on df-hyperliquid-core 0.2.1. Its answers and decoded reads, such as `Log`, are `#[non_exhaustive]` from 0.2.0; nothing in this crate's API changed.
- The deploy docs say CreateX is deployed at its usual address, `0xba5Ed099633D3B313e4D5F7bdc1305d3c28ba5Ed`, on Elysium testnet. They said it was not.

## 0.0.2-beta

- Depends on df-hyperliquid-core 0.1.1, the first stable release, instead of 0.0.10-beta. No change to this crate's API was needed: every type and function it uses from df-hyperliquid-core is the same in 0.1.

## 0.0.1-beta

The first release.
