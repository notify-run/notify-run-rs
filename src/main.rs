use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser};
use google_authz::Credentials;
use logging::init_logging;
use migrate::migrate;
use tiny_firestore_odm::Database;

mod model;
mod migrate;
mod logging;

#[derive(Parser)]
struct Opts {
    #[clap(subcommand)]
    subcmd: SubCommand,

}

#[derive(Parser)]
enum SubCommand {
    Migrate {
        source: PathBuf,
    }
}

async fn get_db() -> Database {
    let creds = Credentials::default().await;
    let project_id = std::env::var("GCP_PROJECT_ID").expect("Expected GCP_PROJECT_ID env var.");
    Database::new(creds.into(), &project_id).await
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
    }

    Ok(())
}
