//! Solana program-event trigger stream.
//!
//! Peer of [`evm_stream`](super::evm_stream) and [`cosmos_stream`](super::cosmos_stream).
//! Subscribes to a Solana RPC WebSocket via `logs_subscribe` filtered to
//! mentions of the configured program ids, then yields [`StreamTriggers::Solana`]
//! items annotated with the replay-identity tuple `(slot, signature,
//! instruction_index, inner_instruction_index, log_index)` from the SVM
//! design doc.
//!
//! v1 scope:
//! - `logs_subscribe` with `RpcTransactionLogsFilter::Mentions(program_ids)`.
//! - Per-trigger filter matching (Anchor discriminator on `Program data:`
//!   lines, or substring match on `Program log:` lines) lives in the
//!   dispatcher; this stream emits one [`SolanaStreamLog`] per log line so
//!   the lookup table can do the per-trigger filter check.
//! - Reconnection: bounded exponential backoff with jitter, matching the
//!   ATProto Jetstream stream pattern. We do not surface partial-progress
//!   cursors — `solana-pubsub-client` reconnects re-subscribe from the
//!   tip; replay protection is the caller's job via the replay-identity
//!   tuple.

use std::pin::Pin;
use std::time::Duration;

use futures::{Stream, StreamExt};
use solana_client::nonblocking::pubsub_client::PubsubClient;
use solana_commitment_config::{CommitmentConfig, CommitmentLevel};
use solana_rpc_client_api::config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter};
use tokio::time::sleep;
use tracing::{error, info, warn};
use utils::telemetry::TriggerMetrics;
use wavs_types::{ChainKey, SolanaAddress, SolanaCommitment};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use wavs_types::SolanaEventFilter;

use crate::subsystems::trigger::error::TriggerError;
use crate::subsystems::trigger::streams::StreamTriggers;

/// Parsed instruction context for a single log line.
///
/// Solana log streams interleave many transactions / instructions per
/// subscription; this struct carries the index information the dispatcher
/// needs to construct the replay-identity tuple.
#[derive(Debug, Clone)]
pub struct SolanaStreamLog {
    /// The slot the transaction was observed in (from the subscription
    /// notification `context.slot`).
    pub slot: u64,
    /// Transaction signature, base58-encoded.
    pub signature: String,
    /// 0-based index of the top-level instruction that produced this log.
    pub instruction_index: u32,
    /// If the log came from an inner instruction (CPI), the 0-based index
    /// of that inner instruction within the outer instruction; `None` for
    /// top-level instructions.
    pub inner_instruction_index: Option<u32>,
    /// 0-based index of this log line within the transaction's log array.
    pub log_index: u32,
    /// Program id that emitted the log (parsed from the most recent
    /// `Program {programId} invoke [N]` marker, or the subscription's
    /// filter program id if parsing failed).
    pub program_id: SolanaAddress,
    /// The raw log line as returned by the validator. The dispatcher tests
    /// per-trigger filters against this — Anchor discriminator matching
    /// runs on the base64 payload following the `Program data:` prefix,
    /// `LogContains` runs on the full line.
    pub raw_log: String,
}

/// Configuration handed to [`start_solana_stream`].
#[derive(Debug, Clone)]
pub struct SolanaStreamConfig {
    /// WebSocket endpoint URL (e.g. `wss://api.devnet.solana.com`).
    pub ws_endpoint: String,
    /// Program ids the subscription should filter on
    /// (`RpcTransactionLogsFilter::Mentions`).
    pub program_ids: Vec<SolanaAddress>,
    /// Default commitment level for the subscription.
    pub commitment: SolanaCommitment,
}

/// Maximum number of reconnect attempts before yielding an error and
/// returning. Matches the ATProto Jetstream stream.
const MAX_RECONNECTS: u32 = 10;
/// Exponential backoff base delay.
const BASE_DELAY: Duration = Duration::from_secs(1);
/// Exponential backoff cap.
const MAX_DELAY: Duration = Duration::from_secs(60);

/// Start a Solana logs subscription against `config.ws_endpoint`.
///
/// Returns a `Stream` of [`StreamTriggers::Solana`] items. The stream
/// reconnects on transport / subscription errors with bounded exponential
/// backoff. After [`MAX_RECONNECTS`] failed reconnects, it yields a final
/// error and terminates.
pub async fn start_solana_stream(
    chain: ChainKey,
    config: SolanaStreamConfig,
    metrics: TriggerMetrics,
) -> Result<Pin<Box<dyn Stream<Item = Result<StreamTriggers, TriggerError>> + Send>>, TriggerError>
{
    if config.program_ids.is_empty() {
        return Err(TriggerError::Config(format!(
            "Solana stream for chain {chain} started with no program ids; \
             subscribing to all logs is intentionally not supported"
        )));
    }

    let stream = async_stream::stream! {
        let mut reconnect_count: u32 = 0;

        // The pubsub client owns the websocket and the active subscription
        // handle; we recreate it on every reconnect.
        loop {
            info!(
                chain = %chain,
                endpoint = %config.ws_endpoint,
                program_ids = config.program_ids.len(),
                "Connecting to Solana logs subscription"
            );

            let client = match PubsubClient::new(&config.ws_endpoint).await {
                Ok(client) => client,
                Err(err) => {
                    error!(
                        chain = %chain,
                        error = ?err,
                        "Failed to connect to Solana pubsub websocket"
                    );
                    metrics.increment_total_errors("solana_pubsub_connect");
                    if let Some(delay) = backoff_delay(reconnect_count) {
                        warn!(chain = %chain, ?delay, "reconnecting Solana pubsub");
                        sleep(delay).await;
                        reconnect_count += 1;
                        continue;
                    } else {
                        yield Err(TriggerError::Config(format!(
                            "Solana pubsub for chain {chain} exhausted reconnects: {err}"
                        )));
                        return;
                    }
                }
            };

            let mentions = config
                .program_ids
                .iter()
                .map(|p| p.to_base58())
                .collect::<Vec<_>>();
            let filter = RpcTransactionLogsFilter::Mentions(mentions);
            let sub_config = RpcTransactionLogsConfig {
                commitment: Some(solana_commitment_to_config(config.commitment)),
            };

            let (mut sub_stream, _unsubscribe) =
                match client.logs_subscribe(filter, sub_config).await {
                    Ok(s) => s,
                    Err(err) => {
                        error!(
                            chain = %chain,
                            error = ?err,
                            "Failed to logs_subscribe on Solana pubsub"
                        );
                        metrics.increment_total_errors("solana_pubsub_subscribe");
                        if let Some(delay) = backoff_delay(reconnect_count) {
                            sleep(delay).await;
                            reconnect_count += 1;
                            continue;
                        } else {
                            yield Err(TriggerError::Config(format!(
                                "Solana logs_subscribe for chain {chain} exhausted reconnects: {err}"
                            )));
                            return;
                        }
                    }
                };

            // Successfully subscribed; reset the backoff counter.
            reconnect_count = 0;
            info!(chain = %chain, "Solana logs subscription active");

            // The default program id we attribute to logs is the
            // subscription's first program id, used as the fallback if
            // parsing the `Program {id} invoke [N]` markers fails for some
            // reason (it shouldn't, but defensive).
            let default_program_id = config.program_ids[0];

            while let Some(notification) = sub_stream.next().await {
                let slot = notification.context.slot;
                let signature = notification.value.signature;
                let logs = notification.value.logs;

                // Skip failed transactions. The design doc treats reorgs
                // above the chosen commitment as user error, but a failed
                // transaction at the chosen commitment is not a trigger —
                // the on-chain state did not change.
                if notification.value.err.is_some() {
                    tracing::debug!(
                        chain = %chain,
                        signature = %signature,
                        err = ?notification.value.err,
                        "Skipping failed Solana transaction"
                    );
                    continue;
                }

                let parsed = parse_transaction_logs(&logs, default_program_id);
                let parsed_logs: Vec<SolanaStreamLog> = parsed
                    .into_iter()
                    .map(|(log_index, ctx, raw_log)| SolanaStreamLog {
                        slot,
                        signature: signature.clone(),
                        instruction_index: ctx.instruction_index,
                        inner_instruction_index: ctx.inner_instruction_index,
                        log_index,
                        program_id: ctx.program_id,
                        raw_log,
                    })
                    .collect();

                if !parsed_logs.is_empty() {
                    yield Ok(StreamTriggers::Solana {
                        chain: chain.clone(),
                        slot,
                        logs: parsed_logs,
                    });
                }
            }

            // The subscription stream ended (server closed it, network
            // dropped, etc.); fall through to the reconnect loop.
            warn!(chain = %chain, "Solana logs subscription closed; reconnecting");
            metrics.increment_total_errors("solana_pubsub_disconnect");

            if let Some(delay) = backoff_delay(reconnect_count) {
                sleep(delay).await;
                reconnect_count += 1;
            } else {
                yield Err(TriggerError::Config(format!(
                    "Solana logs subscription for chain {chain} exhausted reconnects"
                )));
                return;
            }
        }
    };

    Ok(Box::pin(stream))
}

/// Translate a slice 1 [`SolanaCommitment`] to the `solana-client`
/// [`CommitmentConfig`] used by the RPC + pubsub clients.
pub fn solana_commitment_to_config(commitment: SolanaCommitment) -> CommitmentConfig {
    CommitmentConfig {
        commitment: match commitment {
            SolanaCommitment::Processed => CommitmentLevel::Processed,
            SolanaCommitment::Confirmed => CommitmentLevel::Confirmed,
            SolanaCommitment::Finalized => CommitmentLevel::Finalized,
        },
    }
}

/// Exponential backoff with jitter. Returns `None` if `attempt` has
/// exhausted [`MAX_RECONNECTS`].
fn backoff_delay(attempt: u32) -> Option<Duration> {
    if attempt >= MAX_RECONNECTS {
        return None;
    }
    let exp_delay = BASE_DELAY
        .checked_mul(2u32.saturating_pow(attempt))
        .unwrap_or(MAX_DELAY);
    let delay = std::cmp::min(exp_delay, MAX_DELAY);
    let jitter = Duration::from_millis(rand::random::<u64>() % 1000);
    Some(delay + jitter)
}

/// Parsed instruction context for a single log line, before being wrapped
/// into a [`SolanaStreamLog`].
#[derive(Debug, Clone, Copy)]
struct LogContext {
    instruction_index: u32,
    inner_instruction_index: Option<u32>,
    program_id: SolanaAddress,
}

/// Parse a transaction's log array and annotate each non-marker log line
/// with the `(instruction_index, inner_instruction_index, program_id)`
/// it came from.
///
/// Solana validator logs use this shape:
///
/// ```text
/// Program <id> invoke [1]      // top-level instruction starts
/// Program log: ...             // program output
/// Program data: <base64>       // anchor event payload (8-byte disc + payload)
/// Program <other> invoke [2]   // CPI
/// Program <other> success      // CPI ends
/// Program <id> success         // top-level ends
/// ```
///
/// `invoke [N]` markers nest by depth. We track:
/// - `top_level_counter`: incremented on every `invoke [1]`
/// - `inner_counter`: reset on every `invoke [1]`, incremented on
///   `invoke [N]` for N >= 2
/// - `program_stack`: pushed on every invoke, popped on success/failed,
///   so we know which program emitted the next log line
///
/// Returns `(log_index, context, raw_log)` for each log line that is not
/// itself a marker — markers are noisy and don't carry application data.
fn parse_transaction_logs(
    logs: &[String],
    default_program_id: SolanaAddress,
) -> Vec<(u32, LogContext, String)> {
    let mut out = Vec::with_capacity(logs.len());
    let mut top_level_counter: i32 = -1; // first invoke [1] -> 0
    let mut inner_counter: i32 = -1;
    let mut program_stack: Vec<SolanaAddress> = Vec::new();
    let mut current_depth: u32 = 0;

    for (log_index, line) in logs.iter().enumerate() {
        let log_index_u32 = log_index as u32;
        let trimmed = line.as_str();

        if let Some((program_id, depth)) = parse_invoke_marker(trimmed) {
            current_depth = depth;
            if depth == 1 {
                top_level_counter = top_level_counter.saturating_add(1);
                inner_counter = -1;
            } else {
                inner_counter = inner_counter.saturating_add(1);
            }
            program_stack.push(program_id);
            // Markers themselves are not yielded as content lines.
            continue;
        }

        if is_invoke_terminator(trimmed) {
            program_stack.pop();
            current_depth = current_depth.saturating_sub(1);
            continue;
        }

        // Non-marker log line. Attribute to the current invoke context.
        let program_id = program_stack.last().copied().unwrap_or(default_program_id);
        let instruction_index = u32::try_from(top_level_counter.max(0)).unwrap_or(0);
        let inner_instruction_index = if current_depth >= 2 {
            Some(u32::try_from(inner_counter.max(0)).unwrap_or(0))
        } else {
            None
        };

        out.push((
            log_index_u32,
            LogContext {
                instruction_index,
                inner_instruction_index,
                program_id,
            },
            line.clone(),
        ));
    }

    out
}

/// Match `Program <pubkey> invoke [<N>]` and return `(program_id, depth)`.
fn parse_invoke_marker(line: &str) -> Option<(SolanaAddress, u32)> {
    let rest = line.strip_prefix("Program ")?;
    let (program_str, rest) = rest.split_once(' ')?;
    let rest = rest.strip_prefix("invoke [")?;
    let (depth_str, _) = rest.split_once(']')?;
    let depth: u32 = depth_str.parse().ok()?;
    let program_id = SolanaAddress::from_base58(program_str).ok()?;
    Some((program_id, depth))
}

/// Test a [`SolanaEventFilter`] against a single raw log line.
///
/// - [`SolanaEventFilter::Discriminator`]: matches when the log is a
///   `Program data:` line whose base64-decoded payload starts with the
///   given discriminator bytes. This is the Anchor event convention —
///   Anchor emits its 8-byte discriminator + borsh-serialized payload as
///   a base64 string on a `Program data:` line.
/// - [`SolanaEventFilter::LogContains`]: substring match against the raw
///   log line (Anchor / non-Anchor agnostic).
///
/// Returns `(matched, decoded_payload)`. `decoded_payload` is `Some` only
/// for `Discriminator` matches on a parseable `Program data:` line so the
/// dispatcher can hand the decoded bytes to the component; for
/// `LogContains` matches we return `None` (the raw log line is what the
/// component cares about).
pub fn match_log_filter(filter: &SolanaEventFilter, raw_log: &str) -> Option<Option<Vec<u8>>> {
    match filter {
        SolanaEventFilter::Discriminator(disc) => {
            let payload_b64 = raw_log.strip_prefix("Program data: ")?;
            let decoded = BASE64_STANDARD.decode(payload_b64.trim()).ok()?;
            if decoded.len() >= disc.len() && decoded[..disc.len()] == disc[..] {
                Some(Some(decoded))
            } else {
                None
            }
        }
        SolanaEventFilter::LogContains(needle) => {
            if raw_log.contains(needle.as_str()) {
                Some(None)
            } else {
                None
            }
        }
    }
}

/// Match `Program <pubkey> success` or `Program <pubkey> failed: ...`.
fn is_invoke_terminator(line: &str) -> bool {
    if let Some(rest) = line.strip_prefix("Program ") {
        if let Some((_, tail)) = rest.split_once(' ') {
            return tail == "success" || tail.starts_with("failed");
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROG_A: &str = "11111111111111111111111111111111";
    // Token Program (well-known) — used as the inner-CPI target in tests.
    const PROG_B: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

    fn addr(s: &str) -> SolanaAddress {
        SolanaAddress::from_base58(s).unwrap()
    }

    #[test]
    fn parses_top_level_log() {
        let logs = vec![
            format!("Program {PROG_A} invoke [1]"),
            "Program log: hello".into(),
            format!("Program {PROG_A} success"),
        ];
        let parsed = parse_transaction_logs(&logs, addr(PROG_A));
        assert_eq!(parsed.len(), 1);
        let (log_index, ctx, raw) = &parsed[0];
        assert_eq!(*log_index, 1);
        assert_eq!(ctx.instruction_index, 0);
        assert_eq!(ctx.inner_instruction_index, None);
        assert_eq!(ctx.program_id, addr(PROG_A));
        assert_eq!(raw, "Program log: hello");
    }

    #[test]
    fn parses_inner_instruction_log() {
        let logs = vec![
            format!("Program {PROG_A} invoke [1]"),
            "Program log: outer".into(),
            format!("Program {PROG_B} invoke [2]"),
            "Program log: inner".into(),
            format!("Program {PROG_B} success"),
            "Program log: after_inner".into(),
            format!("Program {PROG_A} success"),
        ];
        let parsed = parse_transaction_logs(&logs, addr(PROG_A));
        assert_eq!(parsed.len(), 3);

        // Line 1: top-level
        assert_eq!(parsed[0].1.instruction_index, 0);
        assert_eq!(parsed[0].1.inner_instruction_index, None);
        assert_eq!(parsed[0].1.program_id, addr(PROG_A));

        // Line 3: inner CPI
        assert_eq!(parsed[1].1.instruction_index, 0);
        assert_eq!(parsed[1].1.inner_instruction_index, Some(0));
        assert_eq!(parsed[1].1.program_id, addr(PROG_B));

        // Line 5: back to top-level after CPI returned
        assert_eq!(parsed[2].1.instruction_index, 0);
        assert_eq!(parsed[2].1.inner_instruction_index, None);
        assert_eq!(parsed[2].1.program_id, addr(PROG_A));
    }

    #[test]
    fn parses_multiple_top_level_instructions() {
        let logs = vec![
            format!("Program {PROG_A} invoke [1]"),
            "Program log: ix0".into(),
            format!("Program {PROG_A} success"),
            format!("Program {PROG_A} invoke [1]"),
            "Program log: ix1".into(),
            format!("Program {PROG_A} success"),
        ];
        let parsed = parse_transaction_logs(&logs, addr(PROG_A));
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].1.instruction_index, 0);
        assert_eq!(parsed[1].1.instruction_index, 1);
    }

    #[test]
    fn handles_failed_inner_terminator() {
        let logs = vec![
            format!("Program {PROG_A} invoke [1]"),
            format!("Program {PROG_B} invoke [2]"),
            format!("Program {PROG_B} failed: custom error 0x1"),
            "Program log: outer continues".into(),
            format!("Program {PROG_A} success"),
        ];
        let parsed = parse_transaction_logs(&logs, addr(PROG_A));
        assert_eq!(parsed.len(), 1);
        // The line is after the CPI failed; depth should be back to 1.
        assert_eq!(parsed[0].1.inner_instruction_index, None);
        assert_eq!(parsed[0].1.program_id, addr(PROG_A));
    }

    #[test]
    fn unparseable_lines_fall_back_to_default_program() {
        // No invoke marker at all — the line still gets emitted but with
        // instruction_index 0 and the fallback program id.
        let logs = vec!["unexpected log shape".into()];
        let parsed = parse_transaction_logs(&logs, addr(PROG_A));
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].1.instruction_index, 0);
        assert_eq!(parsed[0].1.program_id, addr(PROG_A));
    }

    #[test]
    fn commitment_translation() {
        assert_eq!(
            solana_commitment_to_config(SolanaCommitment::Processed).commitment,
            CommitmentLevel::Processed
        );
        assert_eq!(
            solana_commitment_to_config(SolanaCommitment::Confirmed).commitment,
            CommitmentLevel::Confirmed
        );
        assert_eq!(
            solana_commitment_to_config(SolanaCommitment::Finalized).commitment,
            CommitmentLevel::Finalized
        );
    }

    #[test]
    fn backoff_delay_caps_at_max_reconnects() {
        assert!(backoff_delay(0).is_some());
        assert!(backoff_delay(MAX_RECONNECTS - 1).is_some());
        assert!(backoff_delay(MAX_RECONNECTS).is_none());
    }

    #[test]
    fn match_log_filter_discriminator_hit() {
        let payload = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0xff, 0xff];
        let b64 = BASE64_STANDARD.encode(&payload);
        let log = format!("Program data: {b64}");

        let disc = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let filter = SolanaEventFilter::Discriminator(disc.clone());

        let res = match_log_filter(&filter, &log).expect("should match");
        assert_eq!(res, Some(payload));
    }

    #[test]
    fn match_log_filter_discriminator_miss_wrong_disc() {
        let payload = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let b64 = BASE64_STANDARD.encode(&payload);
        let log = format!("Program data: {b64}");

        let filter = SolanaEventFilter::Discriminator(vec![0xaa; 8]);
        assert!(match_log_filter(&filter, &log).is_none());
    }

    #[test]
    fn match_log_filter_discriminator_miss_wrong_prefix() {
        // Not a `Program data:` line — discriminator filter should miss.
        let filter = SolanaEventFilter::Discriminator(vec![0x01; 8]);
        assert!(match_log_filter(&filter, "Program log: AQECAwQFBgcI").is_none());
    }

    #[test]
    fn match_log_filter_log_contains_hit() {
        let filter = SolanaEventFilter::LogContains("matched".to_string());
        let res = match_log_filter(&filter, "Program log: this matched the needle")
            .expect("should match");
        assert!(res.is_none());
    }

    #[test]
    fn match_log_filter_log_contains_miss() {
        let filter = SolanaEventFilter::LogContains("missing".to_string());
        assert!(match_log_filter(&filter, "Program log: nothing here").is_none());
    }
}
