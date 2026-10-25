# Elysium reference app

Every part of df-elysium-core in one page, in the browser, on Elysium testnet. Leptos renders it client side; the SDK reads and sends through `fetch` (the `web` feature), straight to Kinetiq's node, HyperEVM's node and Conduit's Sourcify. No server, no bundler, no JavaScript dependencies.

## Run it

Once, the tools:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129
brew install just binaryen    # binaryen's wasm-opt shrinks the build; it is skipped when missing
```

Then, from the repo root:

```sh
cd packages/core/examples/reference-app
just serve
```

Open http://localhost:8080. `just serve` builds into `dist/` and serves it with `python3 -m http.server`; stop it with Ctrl-C. `just build` builds without serving; `dist/` is a static site any host can serve.

In the page:

1. On the Account card, press **New burner key** (or **Connect wallet**, or **Import a key**).
2. Fund it: **Faucet** opens Kinetiq's testnet faucet for HYPE on Elysium; the account's address is the card's first row.
3. Every other card's buttons open one tool each, at its own URL (`#/send`, `#/claims`, ...). **All tools** goes back.

Bridging in, retryable tickets and claims pay gas on HyperEVM, so they also need HYPE on HyperEVM testnet.

## Cards and tools

| Card | Tool | SDK |
|---|---|---|
| Account | | a burner key (`LocalKey` behind the `Signer` trait) or the browser's wallet; `Client::hype` on both chains, `Client::nonce` |
| | Import a key | `LocalKey::from_hex` |
| | Activity | every transaction the page sent and how it ended |
| Transfer | Send HYPE | `Client::estimate` split into Elysium's execution and HyperEVM's data, `transfer::hype` |
| | ERC-20 tokens | five reads in one `eth_call` through `transfer::multicall`, `erc20::transfer` and `approve` in the token's exact units, and a token of your own (`deploy::create`) |
| Bridge | Deposit from HyperEVM | `bridge::deposit` simulated for its message number and Elysium hash, sent, followed with `Message` and `Deposit` |
| | Withdraw to HyperEVM | `bridge::withdraw`, its leaf read with `Withdrawal` |
| | Claim withdrawals | the account's withdrawals from ArbSys's logs, `bridge::claim`, and the claim sent on HyperEVM |
| Retryable tickets | Submit a ticket | `Retryable` priced from both chains, simulated for its ticket id, submitted, followed with `Ticket` |
| | Follow or redeem | a ticket to its redeem, `nitro::retryable::timeout` and `redeem` |
| Contracts | Deploy a contract | CREATE and CREATE2 through the deterministic deployer, each called |
| | Verify on Sourcify | `deploy::verify::status` |
| Chain | Chain and gas | `Client::block` (head, HyperEVM block, ArbOS, send count), `nitro::gas_info`, `nitro::retryable::lifetime`, `bridge::claim::latest` for the confirmed outbox |
| | Inspect a transaction | any hash: on Elysium its Nitro type, gas split, fee and withdrawals; on HyperEVM the messages it delivered and what each became |
| | Live blocks | `live::Heads` and `live::LogCursor`, polled |

Every screen stays mounted: leave one mid-send and its outcome is there when you come back. A hidden screen reads nothing.

## Signing

A burner key is 32 bytes from `crypto.getRandomValues`, kept in this origin's local storage so the page remembers it: testnet only. The SDK prepares, signs and sends with it, the nonce kept between sends.

A wallet (`window.ethereum`) signs instead: the page switches it to the right chain (adding Elysium or HyperEVM testnet when it does not know them) and hands it the call with the SDK's gas estimate and fee cap. The wallet keeps the nonce.

Moving HYPE in, submitting a ticket and claiming a withdrawal pay gas on HyperEVM, so the account needs HYPE there too.
