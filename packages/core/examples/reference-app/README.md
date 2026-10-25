# Elysium reference app

Every part of df-elysium-core in one page, in the browser, on Elysium testnet. Leptos renders it client side; the SDK reads and sends through `fetch` (the `web` feature), straight to Kinetiq's node, HyperEVM's node and Conduit's Sourcify. No server, no bundler, no JavaScript dependencies.

```sh
just serve    # builds into dist/ and serves it on http://localhost:8080
```

Needs the `wasm32-unknown-unknown` target, `wasm-bindgen-cli` 0.2.129 (`cargo install wasm-bindgen-cli --version 0.2.129`) and `wasm-opt` from binaryen. `dist/` is a static site.

## Panels

| Panel | SDK |
|---|---|
| Account | a burner key (`LocalKey` behind the `Signer` trait) or the browser's wallet; `Client::hype` on both chains, `Client::nonce` |
| Chain | `Client::block` (head, HyperEVM block, ArbOS, send count), `nitro::gas_info`, `nitro::retryable::lifetime`, `bridge::claim::latest` for the confirmed outbox |
| Send HYPE | `Client::estimate` split into Elysium's execution and HyperEVM's data, `transfer::hype` |
| Activity | every transaction the page sent and how it ended |
| ERC-20 | five reads in one `eth_call` through `transfer::multicall`, `erc20::transfer` and `approve` in the token's exact units, and a token of your own (`deploy::create`) |
| Bridge | in: `bridge::deposit` simulated for its message number and Elysium hash, sent, followed with `Message` and `Deposit`; out: `bridge::withdraw`, the account's withdrawals from ArbSys's logs, `bridge::claim` and the claim sent on HyperEVM |
| Retryable ticket | `Retryable` priced from both chains, simulated for its ticket id, submitted, followed with `Ticket` to its redeem; `nitro::retryable::redeem` |
| Deploy | CREATE and CREATE2 through the deterministic deployer, each called; `deploy::verify::status` on Sourcify |
| Inspect | any hash: on Elysium its Nitro type, gas split, fee and withdrawals; on HyperEVM the messages it delivered and what each became |
| Live | `live::Heads` and `live::LogCursor`, polled |

## Signing

A burner key is 32 bytes from `crypto.getRandomValues`, kept in this origin's local storage so the page remembers it: testnet only. The SDK prepares, signs and sends with it, the nonce kept between sends.

A wallet (`window.ethereum`) signs instead: the page switches it to the right chain (adding Elysium or HyperEVM testnet when it does not know them) and hands it the call with the SDK's gas estimate and fee cap. The wallet keeps the nonce.

Moving HYPE in, submitting a ticket and claiming a withdrawal pay gas on HyperEVM, so the account needs HYPE there too.
