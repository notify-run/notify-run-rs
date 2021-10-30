use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use google_authz::{Credentials, TokenSource};
use logging::init_logging;
use migrate::migrate;
use server::serve;
use tiny_firestore_odm::Database;

mod database;
mod logging;
mod migrate;
mod model;
mod rate_limiter;
mod server;
mod server_state;
mod vapid;

#[derive(Parser)]
struct Opts {
    #[clap(subcommand)]
    subcmd: SubCommand,
}

#[derive(Parser)]
enum SubCommand {
    Migrate {
        source: PathBuf,
    },
    Serve {
        #[clap(short, long)]
        port: Option<u16>,
    },
}

pub async fn get_creds_and_project() -> (TokenSource, String) {
    let creds = Credentials::default().await;
    let project_id = std::env::var("GCP_PROJECT_ID").expect("Expected GCP_PROJECT_ID env var.");

    (creds.into(), project_id)
}

async fn get_db() -> Database {
    let (token_source, project_id) = get_creds_and_project().await;
    Database::new(token_source, &project_id).await
}

#[tokio::main]
async fn main() -> Result<()> {
    init_logging();

    let opts = Opts::parse();

    let subcommand = opts.subcmd;

    match subcommand {
        SubCommand::Migrate { source } => {
            migrate(source, get_db().await).await?;
        }
        SubCommand::Serve { port } => {
            serve(port).await?;
        }
    }

    Ok(())
}
