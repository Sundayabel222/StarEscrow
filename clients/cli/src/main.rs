use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde_json::{json, Value};
use std::thread;
use std::time::{Duration, Instant};
use std::io::{self, Write};
use chrono;
use ureq;
use sha2::{Sha256, Digest};
use std::fs;

/// StarEscrow CLI — interact with the escrow contract on Stellar Testnet.
///
/// Prerequisites:
///   - Stellar CLI installed: https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli
///   - Contract deployed and ESCROW_CONTRACT_ID set in env
///   - PAYER_SECRET and FREELANCER_SECRET set in env
#[derive(Parser)]
#[command(name = "star-escrow", version, about)]
struct Cli {
    /// Soroban RPC endpoint (default: Testnet)
    #[arg(long, default_value = "https://soroban-testnet.stellar.org")]
    rpc_url: String,

    /// Network passphrase
    #[arg(long, default_value = "Test SDF Network ; September 2015")]
    network_passphrase: String,

    /// Output results as JSON
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialise protocol config (admin, fee)
    Init {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,

        #[arg(long, env = "ADMIN_SECRET")]
        admin_secret: String,

        /// Fee in basis points (e.g. 100 = 1%)
        #[arg(long, default_value = "0")]
        fee_bps: u32,

        /// Fee collector Stellar address
        #[arg(long)]
        fee_collector: String,
    },

    /// Pause all state-changing operations (admin only)
    Pause {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,

        #[arg(long, env = "ADMIN_SECRET")]
        admin_secret: String,
    },

    /// Unpause the contract (admin only)
    Unpause {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,

        #[arg(long, env = "ADMIN_SECRET")]
        admin_secret: String,
    },

    /// Create a new escrow and lock funds
    Create {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,

        #[arg(long, env = "PAYER_SECRET")]
        payer_secret: String,

        /// Freelancer Stellar address
        #[arg(long)]
        freelancer: String,

        /// Token contract ID (use native XLM wrapper or a SAC address)
        #[arg(long)]
        token: String,

        /// Amount in stroops (1 XLM = 10_000_000)
        #[arg(long)]
        amount: i128,

        /// Milestone description
        #[arg(long)]
        milestone: String,

        /// Optional deadline as a ledger timestamp (Unix seconds).
        #[arg(long)]
        deadline: Option<u64>,
    },

    /// Freelancer submits work
    SubmitWork {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,

        #[arg(long, env = "FREELANCER_SECRET")]
        freelancer_secret: String,
    },

    /// Transfer freelancer role to a new address
    TransferFreelancer {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,

        #[arg(long, env = "FREELANCER_SECRET")]
        freelancer_secret: String,

        /// New freelancer Stellar address
        #[arg(long)]
        new_freelancer: String,
    },

    /// Payer approves milestone and releases payment
    Approve {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,

        #[arg(long, env = "PAYER_SECRET")]
        payer_secret: String,
    },

    /// Payer cancels escrow and gets refund (only before work submitted)
    Cancel {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,

        #[arg(long, env = "PAYER_SECRET")]
        payer_secret: String,
    },

    /// Payer reclaims funds after the deadline has passed
    Expire {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,

        #[arg(long, env = "PAYER_SECRET")]
        payer_secret: String,
    },

    /// Read current escrow status and full data
    Status {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,
    },

    /// List all escrows created by a payer address
    List {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,

        /// Payer Stellar address to filter by
        #[arg(long)]
        payer: String,
    },

    /// Poll contract status and notify of changes
    Watch {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,

        /// Polling interval in seconds
        #[arg(long, default_value = "10")]
        interval: u64,

        /// Exit after N seconds
        #[arg(long)]
        timeout: Option<u64>,
    },

    /// Fetch and display event history for the contract
    History {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,
    },

    /// Fund an account on Testnet using Friendbot
    Fund {
        /// Stellar address to fund
        #[arg(long)]
        address: String,
    },

    /// Verify that the deployed contract matches a local WASM file
    Verify {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,

        /// Path to the local WASM file
        #[arg(long)]
        wasm_path: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let as_json = cli.json;

    match cli.command {
        Commands::Init { contract_id, admin_secret, fee_bps, fee_collector } => {
            let admin_addr = stellar_address_from_secret(&admin_secret)?;
            invoke_stellar_cli(
                &cli.rpc_url, &cli.network_passphrase, &contract_id, &admin_secret,
                "init",
                &["--admin", &admin_addr, "--fee-bps", &fee_bps.to_string(), "--fee-collector", &fee_collector],
            )?;
            output(as_json, json!({"status": "ok", "action": "init"}), "Protocol initialised.");
        }

        Commands::Pause { contract_id, admin_secret } => {
            invoke_stellar_cli(
                &cli.rpc_url, &cli.network_passphrase, &contract_id, &admin_secret,
                "pause", &[],
            )?;
            output(as_json, json!({"status": "ok", "action": "pause"}), "Contract paused.");
        }

        Commands::Unpause { contract_id, admin_secret } => {
            invoke_stellar_cli(
                &cli.rpc_url, &cli.network_passphrase, &contract_id, &admin_secret,
                "unpause", &[],
            )?;
            output(as_json, json!({"status": "ok", "action": "unpause"}), "Contract unpaused.");
        }

        Commands::Create { contract_id, payer_secret, freelancer, token, amount, milestone, deadline } => {
            let payer_addr = stellar_address_from_secret(&payer_secret)?;
            let deadline_str = deadline.map(|d| d.to_string()).unwrap_or_else(|| "null".into());
            invoke_stellar_cli(
                &cli.rpc_url, &cli.network_passphrase, &contract_id, &payer_secret,
                "create",
                &["--payer", &payer_addr, "--freelancer", &freelancer, "--token", &token,
                  "--amount", &amount.to_string(), "--milestone", &milestone,
                  "--deadline", &deadline_str],
            )?;
            output(
                as_json,
                json!({"status": "ok", "action": "create", "contract_id": contract_id,
                       "payer": payer_addr, "freelancer": freelancer, "amount": amount,
                       "milestone": milestone, "deadline": deadline}),
                "Escrow created. Funds locked.",
            );
        }

        Commands::SubmitWork { contract_id, freelancer_secret } => {
            invoke_stellar_cli(
                &cli.rpc_url, &cli.network_passphrase, &contract_id, &freelancer_secret,
                "submit_work", &[],
            )?;
            output(as_json, json!({"status": "ok", "action": "submit_work"}), "Work submitted. Waiting for payer approval.");
        }

        Commands::TransferFreelancer { contract_id, freelancer_secret, new_freelancer } => {
            invoke_stellar_cli(
                &cli.rpc_url, &cli.network_passphrase, &contract_id, &freelancer_secret,
                "transfer_freelancer",
                &["--new-freelancer", &new_freelancer],
            )?;
            output(
                as_json,
                json!({"status": "ok", "action": "transfer_freelancer", "new_freelancer": new_freelancer}),
                &format!("Freelancer role transferred to {new_freelancer}."),
            );
        }

        Commands::Approve { contract_id, payer_secret } => {
            invoke_stellar_cli(
                &cli.rpc_url, &cli.network_passphrase, &contract_id, &payer_secret,
                "approve", &[],
            )?;
            output(as_json, json!({"status": "ok", "action": "approve"}), "Payment released to freelancer.");
        }

        Commands::Cancel { contract_id, payer_secret } => {
            invoke_stellar_cli(
                &cli.rpc_url, &cli.network_passphrase, &contract_id, &payer_secret,
                "cancel", &[],
            )?;
            output(as_json, json!({"status": "ok", "action": "cancel"}), "Escrow cancelled. Funds refunded to payer.");
        }

        Commands::Expire { contract_id, payer_secret } => {
            invoke_stellar_cli(
                &cli.rpc_url, &cli.network_passphrase, &contract_id, &payer_secret,
                "expire", &[],
            )?;
            output(as_json, json!({"status": "ok", "action": "expire"}), "Escrow expired. Funds returned to payer.");
        }

        Commands::Status { contract_id } => {
            let raw = query_contract(&cli.rpc_url, &cli.network_passphrase, &contract_id, "get_escrow")?;
            if as_json {
                // Parse the XDR/JSON output from stellar CLI and re-emit as JSON
                let parsed: Value = serde_json::from_str(raw.trim()).unwrap_or(Value::String(raw.trim().to_string()));
                println!("{}", serde_json::to_string_pretty(&json!({"status": "ok", "escrow": parsed}))?);
            } else {
                println!("{}", raw.trim());
            }
        }

        Commands::List { contract_id, payer } => {
            list_escrows(&cli.rpc_url, &cli.network_passphrase, &contract_id, &payer, as_json)?;
        }

        Commands::Watch { contract_id, interval, timeout } => {
            let start = Instant::now();
            let mut last_status = None;

            if !as_json {
                println!("Watching escrow {} (interval: {}s)...", contract_id, interval);
            }

            loop {
                let current_raw = query_contract(&cli.rpc_url, &cli.network_passphrase, &contract_id, "get_status")?;
                let current_status = current_raw.trim().to_string();

                if let Some(ref last) = last_status {
                    if *last != current_status {
                        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                        output(
                            as_json,
                            json!({"status": "changed", "new_status": current_status, "timestamp": timestamp}),
                            &format!("[{}] Status changed: {}", timestamp, current_status),
                        );
                        last_status = Some(current_status);
                    }
                } else {
                    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                    output(
                        as_json,
                        json!({"status": "initial", "current_status": current_status, "timestamp": timestamp}),
                        &format!("[{}] Initial status: {}", timestamp, current_status),
                    );
                    last_status = Some(current_status);
                }

                if let Some(t) = timeout {
                    if start.elapsed() >= Duration::from_secs(t) {
                        if !as_json {
                            println!("Timeout reached ({}s). Exiting.", t);
                        }
                        break;
                    }
                }

                thread::sleep(Duration::from_secs(interval));
            }
        }

        Commands::History { contract_id } => {
            let events = fetch_events(&cli.rpc_url, &cli.network_passphrase, &contract_id)?;
            if events.is_empty() {
                output(as_json, json!({"events": []}), "No events found.");
            } else if as_json {
                println!("{}", serde_json::to_string_pretty(&json!({"events": events}))?);
            } else {
                println!("Event history for contract {}:", contract_id);
                for (i, e) in events.iter().enumerate() {
                    let event_type = e["topic"][0].as_str().unwrap_or("unknown");
                    let timestamp = e["ledger_closed_at"].as_str().unwrap_or("unknown");
                    println!("  [{}] {} | {} | data={}", i + 1, timestamp, event_type, e["value"]);
                }
            }
        }

        Commands::Fund { address } => {
            if cli.network_passphrase != "Test SDF Network ; September 2015" {
                anyhow::bail!("Fund command is only available on Testnet.");
            }

            if !as_json {
                println!("Requesting XLM from Friendbot for {}...", address);
            }

            let url = format!("https://friendbot.stellar.org?addr={}", address);
            let resp = ureq::get(&url).call();

            match resp {
                Ok(_) => {
                    output(as_json, json!({"status": "ok", "address": address}), &format!("Successfully funded {}.", address));
                }
                Err(ureq::Error::Status(code, _)) => {
                    anyhow::bail!("Friendbot returned error code {}. The account might already be funded or the address is invalid.", code);
                }
                Err(e) => {
                    anyhow::bail!("Failed to call Friendbot: {}", e);
                }
            }
        }

        Commands::Verify { contract_id, wasm_path } => {
            if !as_json {
                println!("Verifying contract {} against {}...", contract_id, wasm_path);
            }

            // Fetch remote hash
            let remote_hash = fetch_remote_wasm_hash(&cli.rpc_url, &cli.network_passphrase, &contract_id)?;

            // Compute local hash
            let local_wasm = fs::read(&wasm_path).with_context(|| format!("Failed to read local WASM file at {}", wasm_path))?;
            let mut hasher = Sha256::new();
            hasher.update(&local_wasm);
            let local_hash = hex::encode(hasher.finalize());

            let matches = remote_hash == local_hash;

            if as_json {
                println!("{}", serde_json::to_string_pretty(&json!({
                    "contract_id": contract_id,
                    "wasm_path": wasm_path,
                    "remote_hash": remote_hash,
                    "local_hash": local_hash,
                    "matches": matches
                }))?);
            } else {
                println!("Remote Hash: {}", remote_hash);
                println!("Local Hash:  {}", local_hash);
                if matches {
                    println!("SUCCESS: Hashes match!");
                } else {
                    eprintln!("FAILURE: Hashes do not match!");
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}

/// Print human-readable or JSON output depending on the flag.
fn output(as_json: bool, data: Value, human: &str) {
    if as_json {
        println!("{}", serde_json::to_string_pretty(&data).unwrap());
    } else {
        println!("{human}");
    }
}

/// Query `escrow_created` events for a given payer and display results.
fn list_escrows(rpc_url: &str, network_passphrase: &str, contract_id: &str, payer: &str, as_json: bool) -> Result<()> {
    let events = fetch_events(rpc_url, network_passphrase, contract_id)?;

    let escrows: Vec<Value> = events
        .into_iter()
        .filter(|e| {
            e["topic"][0].as_str().unwrap_or("") == "escrow_created"
                && e["value"][0].as_str().unwrap_or("") == payer
        })
        .map(|e| json!({
            "contract_id": contract_id,
            "payer": e["value"][0],
            "freelancer": e["value"][1],
            "amount": e["value"][2],
            "milestone": e["value"][3],
        }))
        .collect();

    if as_json {
        println!("{}", serde_json::to_string_pretty(&json!({"escrows": escrows}))?);
    } else if escrows.is_empty() {
        println!("No escrows found for payer {payer}");
    } else {
        println!("Escrows for payer {payer}:");
        for (i, e) in escrows.iter().enumerate() {
            println!(
                "  [{}] contract={} milestone={} amount={} freelancer={}",
                i + 1,
                e["contract_id"].as_str().unwrap_or("-"),
                e["milestone"].as_str().unwrap_or("-"),
                e["amount"],
                e["freelancer"].as_str().unwrap_or("-"),
            );
        }
    }

    Ok(())
}

fn fetch_events(rpc_url: &str, network_passphrase: &str, contract_id: &str) -> Result<Vec<Value>> {
    let out = std::process::Command::new("stellar")
        .args([
            "contract", "events",
            "--id", contract_id,
            "--rpc-url", rpc_url,
            "--network-passphrase", network_passphrase,
            "--output", "json",
        ])
        .output()
        .context("stellar CLI not found — install from https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli")?;

    let raw = String::from_utf8_lossy(&out.stdout);
    let events: Vec<Value> = serde_json::from_str(&raw).unwrap_or_default();
    Ok(events)
}

fn fetch_remote_wasm_hash(rpc_url: &str, network_passphrase: &str, contract_id: &str) -> Result<String> {
    let out = std::process::Command::new("stellar")
        .args([
            "contract", "fetch",
            "--id", contract_id,
            "--rpc-url", rpc_url,
            "--network-passphrase", network_passphrase,
            "--output", "wasm",
        ])
        .output()
        .context("Failed to fetch contract WASM from network")?;

    if !out.status.success() {
        anyhow::bail!("stellar contract fetch failed: {}", String::from_utf8_lossy(&out.stderr));
    }

    let mut hasher = Sha256::new();
    hasher.update(&out.stdout);
    let result = hasher.finalize();
    Ok(hex::encode(result))
}

fn query_contract(rpc_url: &str, network_passphrase: &str, contract_id: &str, function: &str) -> Result<String> {
    let out = std::process::Command::new("stellar")
        .args([
            "contract", "invoke",
            "--id", contract_id,
            "--rpc-url", rpc_url,
            "--network-passphrase", network_passphrase,
            "--",
            function,
        ])
        .output()
        .context("stellar CLI not found — install from https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli")?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn invoke_stellar_cli(
    rpc_url: &str,
    network_passphrase: &str,
    contract_id: &str,
    secret: &str,
    function: &str,
    extra_args: &[&str],
) -> Result<()> {
    let mut args = vec![
        "contract", "invoke",
        "--id", contract_id,
        "--rpc-url", rpc_url,
        "--network-passphrase", network_passphrase,
        "--source", secret,
        "--",
        function,
    ];
    args.extend_from_slice(extra_args);

    let status = std::process::Command::new("stellar")
        .args(&args)
        .status()
        .context("stellar CLI not found — install from https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli")?;

    if !status.success() {
        anyhow::bail!("stellar CLI exited with status {status}");
    }
    Ok(())
}

fn stellar_address_from_secret(secret: &str) -> Result<String> {
    let out = std::process::Command::new("stellar")
        .args(["keys", "address", secret])
        .output()
        .context("stellar CLI not found")?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
