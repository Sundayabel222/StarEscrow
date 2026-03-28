use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde_json::{json, Value};

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
        #[arg(long)]
        freelancer: String,
        #[arg(long)]
        token: String,
        #[arg(long)]
        amount: i128,
        #[arg(long)]
        milestone: String,
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
        #[arg(long)]
        payer: String,
    },
    /// Interactive setup wizard: configure network, generate keypairs, fund via friendbot,
    /// write .env, and deploy the contract.
    Setup,
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
            invoke_stellar_cli(&cli.rpc_url, &cli.network_passphrase, &contract_id, &admin_secret, "pause", &[])?;
            output(as_json, json!({"status": "ok", "action": "pause"}), "Contract paused.");
        }
        Commands::Unpause { contract_id, admin_secret } => {
            invoke_stellar_cli(&cli.rpc_url, &cli.network_passphrase, &contract_id, &admin_secret, "unpause", &[])?;
            output(as_json, json!({"status": "ok", "action": "unpause"}), "Contract unpaused.");
        }
        Commands::Create { contract_id, payer_secret, freelancer, token, amount, milestone, deadline } => {
            let payer_addr = stellar_address_from_secret(&payer_secret)?;
            let deadline_str = deadline.map(|d| d.to_string()).unwrap_or_else(|| "null".into());
            invoke_stellar_cli(
                &cli.rpc_url, &cli.network_passphrase, &contract_id, &payer_secret, "create",
                &["--payer", &payer_addr, "--freelancer", &freelancer, "--token", &token,
                  "--amount", &amount.to_string(), "--milestone", &milestone, "--deadline", &deadline_str],
            )?;
            output(as_json,
                json!({"status":"ok","action":"create","contract_id":contract_id,"payer":payer_addr,
                       "freelancer":freelancer,"amount":amount,"milestone":milestone,"deadline":deadline}),
                "Escrow created. Funds locked.");
        }
        Commands::SubmitWork { contract_id, freelancer_secret } => {
            invoke_stellar_cli(&cli.rpc_url, &cli.network_passphrase, &contract_id, &freelancer_secret, "submit_work", &[])?;
            output(as_json, json!({"status":"ok","action":"submit_work"}), "Work submitted. Waiting for payer approval.");
        }
        Commands::TransferFreelancer { contract_id, freelancer_secret, new_freelancer } => {
            invoke_stellar_cli(
                &cli.rpc_url, &cli.network_passphrase, &contract_id, &freelancer_secret,
                "transfer_freelancer", &["--new-freelancer", &new_freelancer],
            )?;
            output(as_json,
                json!({"status":"ok","action":"transfer_freelancer","new_freelancer":new_freelancer}),
                &format!("Freelancer role transferred to {new_freelancer}."));
        }
        Commands::Approve { contract_id, payer_secret } => {
            invoke_stellar_cli(&cli.rpc_url, &cli.network_passphrase, &contract_id, &payer_secret, "approve", &[])?;
            output(as_json, json!({"status":"ok","action":"approve"}), "Payment released to freelancer.");
        }
        Commands::Cancel { contract_id, payer_secret } => {
            invoke_stellar_cli(&cli.rpc_url, &cli.network_passphrase, &contract_id, &payer_secret, "cancel", &[])?;
            output(as_json, json!({"status":"ok","action":"cancel"}), "Escrow cancelled. Funds refunded to payer.");
        }
        Commands::Expire { contract_id, payer_secret } => {
            invoke_stellar_cli(&cli.rpc_url, &cli.network_passphrase, &contract_id, &payer_secret, "expire", &[])?;
            output(as_json, json!({"status":"ok","action":"expire"}), "Escrow expired. Funds returned to payer.");
        }
        Commands::Status { contract_id } => {
            let raw = query_contract(&cli.rpc_url, &cli.network_passphrase, &contract_id, "get_escrow")?;
            if as_json {
                let parsed: Value = serde_json::from_str(raw.trim()).unwrap_or(Value::String(raw.trim().to_string()));
                println!("{}", serde_json::to_string_pretty(&json!({"status":"ok","escrow":parsed}))?);
            } else {
                println!("{}", raw.trim());
            }
        }
        Commands::List { contract_id, payer } => {
            list_escrows(&cli.rpc_url, &cli.network_passphrase, &contract_id, &payer, as_json)?;
        }
        Commands::Setup => {
            run_setup_wizard()?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Setup wizard
// ---------------------------------------------------------------------------

fn run_setup_wizard() -> Result<()> {
    use dialoguer::{Input, Select};

    println!("\n✦ StarEscrow Setup Wizard\n");

    // 1. Network selection
    let networks = ["testnet", "mainnet", "custom"];
    let net_idx = Select::new()
        .with_prompt("Select network")
        .items(&networks)
        .default(0)
        .interact()?;

    let (rpc_url, network_passphrase) = match net_idx {
        0 => (
            "https://soroban-testnet.stellar.org".to_string(),
            "Test SDF Network ; September 2015".to_string(),
        ),
        1 => (
            "https://soroban-mainnet.stellar.org".to_string(),
            "Public Global Stellar Network ; September 2015".to_string(),
        ),
        _ => {
            let rpc: String = Input::new().with_prompt("RPC URL").interact_text()?;
            let pass: String = Input::new().with_prompt("Network passphrase").interact_text()?;
            (rpc, pass)
        }
    };

    // 2. Keypair: generate or provide
    let use_existing = Select::new()
        .with_prompt("Keypair")
        .items(&["Generate new keypair", "Enter existing secret key"])
        .default(0)
        .interact()?
        == 1;

    let secret_key = if use_existing {
        Input::<String>::new().with_prompt("Secret key (S...)").interact_text()?
    } else {
        let out = std::process::Command::new("stellar")
            .args(["keys", "generate", "--no-fund", "setup-key"])
            .output()
            .context("stellar CLI not found")?;
        let secret = String::from_utf8_lossy(&out.stdout).trim().to_string();
        println!("Generated secret key: {secret}");
        secret
    };

    let address = stellar_address_from_secret(&secret_key)?;
    println!("Account address: {address}");

    // 3. Fund via Friendbot (testnet only)
    if net_idx == 0 {
        println!("Funding account via Friendbot…");
        let status = std::process::Command::new("curl")
            .args(["-s", "-o", "/dev/null", "-w", "%{http_code}",
                   &format!("https://friendbot.stellar.org?addr={address}")])
            .status()
            .context("curl not found")?;
        if status.success() {
            println!("Account funded.");
        } else {
            eprintln!("Warning: Friendbot request may have failed.");
        }
    }

    // 4. Deploy contract
    println!("Deploying StarEscrow contract…");
    let deploy_out = std::process::Command::new("stellar")
        .args([
            "contract", "deploy",
            "--wasm", "target/wasm32-unknown-unknown/release/escrow.wasm",
            "--source", &secret_key,
            "--rpc-url", &rpc_url,
            "--network-passphrase", &network_passphrase,
        ])
        .output()
        .context("stellar CLI not found")?;

    let contract_id = String::from_utf8_lossy(&deploy_out.stdout).trim().to_string();
    if contract_id.is_empty() {
        anyhow::bail!("Contract deployment failed:\n{}", String::from_utf8_lossy(&deploy_out.stderr));
    }
    println!("Contract deployed: {contract_id}");

    // 5. Write .env
    let env_content = format!(
        "ESCROW_CONTRACT_ID={contract_id}\nADMIN_SECRET={secret_key}\nPAYER_SECRET={secret_key}\nFREELANCER_SECRET=\nRPC_URL={rpc_url}\nNETWORK_PASSPHRASE={network_passphrase}\n"
    );
    std::fs::write(".env", &env_content).context("Failed to write .env")?;
    println!("\n.env written. Review and update FREELANCER_SECRET before use.\n");
    println!("Setup complete!");

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn output(as_json: bool, data: Value, human: &str) {
    if as_json {
        println!("{}", serde_json::to_string_pretty(&data).unwrap());
    } else {
        println!("{human}");
    }
}

fn list_escrows(rpc_url: &str, network_passphrase: &str, contract_id: &str, payer: &str, as_json: bool) -> Result<()> {
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
            println!("  [{}] contract={} milestone={} amount={} freelancer={}",
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

fn query_contract(rpc_url: &str, network_passphrase: &str, contract_id: &str, function: &str) -> Result<String> {
    let out = std::process::Command::new("stellar")
        .args([
            "contract", "invoke",
            "--id", contract_id,
            "--rpc-url", rpc_url,
            "--network-passphrase", network_passphrase,
            "--", function,
        ])
        .output()
        .context("stellar CLI not found — install from https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli")?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn invoke_stellar_cli(
    rpc_url: &str, network_passphrase: &str, contract_id: &str,
    secret: &str, function: &str, extra_args: &[&str],
) -> Result<()> {
    let mut args = vec![
        "contract", "invoke",
        "--id", contract_id,
        "--rpc-url", rpc_url,
        "--network-passphrase", network_passphrase,
        "--source", secret,
        "--", function,
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
