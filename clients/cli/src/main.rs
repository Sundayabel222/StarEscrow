use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

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
    #[arg(
        long,
        default_value = "Test SDF Network ; September 2015"
    )]
    network_passphrase: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new escrow and lock funds
    Create {
        /// Contract ID (or set ESCROW_CONTRACT_ID env var)
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,

        /// Payer secret key (or set PAYER_SECRET env var)
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
    },

    /// Freelancer submits work
    SubmitWork {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,

        #[arg(long, env = "FREELANCER_SECRET")]
        freelancer_secret: String,
    },

    /// Payer approves milestone and releases payment
    Approve {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,

        #[arg(long, env = "PAYER_SECRET")]
        payer_secret: String,

        #[arg(long)]
        token: String,
    },

    /// Payer cancels escrow and gets refund (only before work submitted)
    Cancel {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,

        #[arg(long, env = "PAYER_SECRET")]
        payer_secret: String,

        #[arg(long)]
        token: String,
    },

    /// Read current escrow state
    Status {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Create {
            contract_id,
            payer_secret,
            freelancer,
            token,
            amount,
            milestone,
        } => {
            println!("Creating escrow on contract {contract_id}...");
            invoke_stellar_cli(
                &cli.rpc_url,
                &cli.network_passphrase,
                &contract_id,
                &payer_secret,
                "create",
                &[
                    "--payer",    &stellar_address_from_secret(&payer_secret)?,
                    "--freelancer", &freelancer,
                    "--token",    &token,
                    "--amount",   &amount.to_string(),
                    "--milestone", &milestone,
                ],
            )?;
            println!("Escrow created. Funds locked.");
        }

        Commands::SubmitWork { contract_id, freelancer_secret } => {
            println!("Submitting work...");
            invoke_stellar_cli(
                &cli.rpc_url,
                &cli.network_passphrase,
                &contract_id,
                &freelancer_secret,
                "submit_work",
                &[],
            )?;
            println!("Work submitted. Waiting for payer approval.");
        }

        Commands::Approve { contract_id, payer_secret, token } => {
            println!("Approving milestone and releasing payment...");
            invoke_stellar_cli(
                &cli.rpc_url,
                &cli.network_passphrase,
                &contract_id,
                &payer_secret,
                "approve",
                &["--token", &token],
            )?;
            println!("Payment released to freelancer.");
        }

        Commands::Cancel { contract_id, payer_secret, token } => {
            println!("Cancelling escrow...");
            invoke_stellar_cli(
                &cli.rpc_url,
                &cli.network_passphrase,
                &contract_id,
                &payer_secret,
                "cancel",
                &["--token", &token],
            )?;
            println!("Escrow cancelled. Funds refunded to payer.");
        }

        Commands::Status { contract_id } => {
            println!("Fetching escrow status for {contract_id}...");
            // Read-only call — no signing needed, use a dummy source
            let output = std::process::Command::new("stellar")
                .args([
                    "contract", "invoke",
                    "--id", &contract_id,
                    "--rpc-url", &cli.rpc_url,
                    "--network-passphrase", &cli.network_passphrase,
                    "--",
                    "get_escrow",
                ])
                .output()
                .context("stellar CLI not found — install from https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli")?;

            println!("{}", String::from_utf8_lossy(&output.stdout));
        }
    }

    Ok(())
}

/// Shell out to the Stellar CLI to invoke a contract function.
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

/// Derive the public address from a secret key using the stellar CLI.
fn stellar_address_from_secret(secret: &str) -> Result<String> {
    let output = std::process::Command::new("stellar")
        .args(["keys", "address", secret])
        .output()
        .context("stellar CLI not found")?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
