//! A retryable ticket: a call on Elysium sent and paid for from HyperEVM. Builds one (its
//! submission cost from HyperEVM's base fee, its gas from Elysium's NodeInterface, its fee cap
//! from Elysium's base fee) and simulates it on HyperEVM; or follows one from its HyperEVM
//! transaction to its ticket, its redeem and its state on Elysium:
//! `cargo run -p df-elysium-core --example retryable [-- --to 0x.. --value 0 | --track 0x.. | --send]`.
//!
//! `--send` submits from the test key, which needs HYPE on HyperEVM testnet for the deposit and
//! the gas.

#[path = "support/mod.rs"]
mod support;

use df_elysium_core::bridge::retryable::{CREATE_RETRYABLE_TICKET, submission_fee, submission_fee_read};
use df_elysium_core::bridge::{Message, Retryable, Ticket, apply_alias};
use df_elysium_core::client::native::Native;
use df_elysium_core::client::{BlockTag, CallRequest, Client, with_headroom};
use df_elysium_core::nitro::node::{RetryableEstimate, estimate_retryable_ticket};
use df_elysium_core::nitro::retryable::{self, REDEEM_SCHEDULED};
use df_elysium_core::utils::{Dec, hype, wei};
use df_elysium_core::{Address, B256, Network, U256, gas};

/// A ticket HyperEVM testnet delivered to Elysium testnet on 2026-10-25, redeemed on its own.
const RECORDED: &str = "0x398575757d6692027b4fb9fc32eb94cc4d9cfbde1dc2fc5b3a22ac6963082bb8";

/// HYPE's system address on HyperEVM: it holds HYPE, so a simulation from it never runs short.
const HYPE_SYSTEM: &str = "0x2222222222222222222222222222222222222222";

#[tokio::main]
async fn main() -> support::Result<()> {
    let args = support::Args::read();
    let network = Network::Testnet;
    let elysium = Client::reader(Native::new(), network);
    let parent = Client::parent(Native::new(), network, support::key()?);
    let me = parent.address();

    if args.flag("track") {
        let hash = args.value("track").filter(|value| value.starts_with("0x")).unwrap_or(RECORDED);
        return track(&elysium, &parent, hash.parse()?).await;
    }

    let to = support::address(&args, "to", me)?;
    let value: Dec = args.parsed("value", Dec::ZERO)?;
    let data: Vec<u8> = Vec::new();
    // HyperEVM's base fee prices the submission; twice it leaves room for a rise before it lands.
    let base = parent.gas_price().await?;
    let fee = submission_fee(data.len(), base);
    let inbox_fee = parent.read(&submission_fee_read(network, data.len(), base)).await?;
    let estimate = RetryableEstimate {
        sender: me,
        deposit: wei(Dec::ONE)?,
        to,
        l2_call_value: wei(value)?,
        excess_fee_refund_address: me,
        call_value_refund_address: me,
        data: data.clone(),
    };
    let estimated = elysium.rpc().estimate_gas(&CallRequest::of(None, &estimate_retryable_ticket(&estimate))).await?;
    let ticket = Retryable {
        to,
        l2_call_value: wei(value)?,
        max_submission_cost: fee.saturating_mul(U256::from_u64(2)),
        excess_fee_refund_address: me,
        call_value_refund_address: me,
        gas_limit: U256::from_u64(with_headroom(estimated)),
        max_fee_per_gas: gas::fees(elysium.gas_price().await?).max_fee,
        data,
    };
    let call = ticket.call(network).ok_or("the ticket's deposit does not fit 256 bits")?;
    println!("ticket from {me} on HyperEVM to {to} on Elysium, carrying {value} HYPE");
    println!(
        "  submission  {} wei at HyperEVM's base fee {base} (the inbox says {inbox_fee}), {} allowed",
        fee, ticket.max_submission_cost
    );
    println!(
        "  gas         {estimated} estimated on Elysium, {} allowed at {} wei",
        ticket.gas_limit, ticket.max_fee_per_gas
    );
    println!("  deposit     {} HYPE, what the HyperEVM transaction carries", hype(call.value)?);
    println!(
        "  call        createRetryableTicket on the inbox {}, {} bytes",
        network.contracts().inbox,
        call.data.len()
    );

    let funded = parent.balance(me).await? > call.value;
    let sender: Address = if funded { me } else { HYPE_SYSTEM.parse()? };
    if !funded {
        println!("  (the test key has no HYPE on HyperEVM: simulated from {sender} instead)");
    }
    let request = CallRequest::of(Some(sender), &call);
    let answer = parent.rpc().call(&request, BlockTag::Latest).await?;
    let number = CREATE_RETRYABLE_TICKET
        .answer(&answer)?
        .first()
        .and_then(|token| token.as_u256())
        .ok_or("no message number")?;
    let ahead = Ticket {
        message_number: number,
        from: apply_alias(sender),
        l1_base_fee: base,
        deposit: call.value,
        retryable: ticket,
    };
    println!("  simulated   message {number}, {} gas on HyperEVM", parent.rpc().estimate_gas(&request).await?);
    println!("  ticket id   {} if delivered as message {number} at this base fee", ahead.id(network.chain_id()));

    if !args.flag("send") {
        support::dry();
        return Ok(());
    }
    let hash = parent.send(call).await?;
    println!("\nsent on HyperEVM  {hash}");
    support::receipt(&parent, hash).await?;
    track(&elysium, &parent, hash).await
}

/// The tickets a HyperEVM transaction submitted, followed on Elysium.
async fn track<S>(elysium: &Client<Native>, parent: &Client<Native, S>, hash: B256) -> support::Result<()> {
    let network = elysium.network();
    let receipt = parent.receipt(hash).await?.ok_or("that transaction is not on HyperEVM")?;
    println!("HyperEVM {hash}  block {}", receipt.block_number.unwrap_or_default());
    for message in Message::in_logs(&receipt.logs, &network.contracts())? {
        let Some(ticket) = Ticket::from_message(&message) else {
            println!("  message {}  {:?}, not a ticket", message.number, message.kind);
            continue;
        };
        let id = ticket.id(network.chain_id());
        println!("  message {}  ticket {id}", message.number);
        println!(
            "    to {} carrying {} HYPE, {} gas",
            ticket.retryable.to,
            hype(ticket.retryable.l2_call_value)?,
            ticket.retryable.gas_limit
        );
        let Some(submitted) = elysium.receipt(id).await? else {
            println!("    not on Elysium yet");
            continue;
        };
        println!(
            "    submitted   block {}, type {:#x}",
            submitted.block_number.unwrap_or_default(),
            submitted.kind.unwrap_or_default()
        );
        for log in submitted.logs.iter().filter(|log| REDEEM_SCHEDULED.matches(log)) {
            let tokens = REDEEM_SCHEDULED.decode(log)?;
            let redeem =
                tokens.get(1).and_then(|token| token.as_bytes()).and_then(|bytes| <[u8; 32]>::try_from(bytes).ok());
            let redeem = B256(redeem.ok_or("RedeemScheduled names no redeem")?);
            match elysium.receipt(redeem).await? {
                Some(ran) => {
                    println!(
                        "    redeemed    {redeem}  block {}, succeeded {}",
                        ran.block_number.unwrap_or_default(),
                        ran.succeeded()
                    )
                }
                None => println!("    redeem      {redeem} scheduled, not yet run"),
            }
        }
        match elysium.read(&retryable::timeout(id)).await {
            Ok(timeout) => println!("    ticket      live until {timeout}: anyone may redeem it"),
            Err(error) if retryable::no_ticket(&error) => println!("    ticket      gone: redeemed or expired"),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
