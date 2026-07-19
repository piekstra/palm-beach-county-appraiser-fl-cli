//! Thin binary: parse arguments and dispatch into the library's `commands`.
//! All logic lives in the `palm_beach_county_appraiser_fl` crate (see
//! `src/lib.rs`).

use clap::Parser;

use palm_beach_county_appraiser_fl::cli::{AuthCmd, Cli, Command};
use palm_beach_county_appraiser_fl::commands::{self, Ctx};
use palm_beach_county_appraiser_fl::error::AppError;

fn run(cli: &Cli) -> Result<(), AppError> {
    // `completions` needs only the clap command definition — no client, no
    // config — so it is handled before any of that is built.
    if let Command::Completions { shell } = &cli.command {
        return commands::completions::run(*shell);
    }

    let ctx = Ctx::new(cli)?;
    match &cli.command {
        Command::Search {
            query,
            owner,
            municipality,
            limit,
        } => commands::search::run(&ctx, query, *owner, municipality.as_deref(), *limit),
        Command::Parcel { pcn, details } => commands::parcel::run(&ctx, &pcn.pcn, *details),
        Command::Sales { pcn, limit } => commands::sales::run(&ctx, &pcn.pcn, *limit),
        Command::Taxes { pcn, limit } => commands::taxes::run(&ctx, &pcn.pcn, *limit),
        Command::Building(pcn) => commands::building::run(&ctx, &pcn.pcn),
        Command::Info => commands::info::run(&ctx),
        Command::Auth(AuthCmd::Status) => commands::auth::status(&ctx),
        Command::Config(cmd) => commands::config::run(&ctx, cmd),
        Command::Api(args) => commands::api::run(&ctx, args),
        Command::SelfUpdate(args) => commands::self_update::run(&ctx, args),
        Command::Completions { .. } => unreachable!("handled above"),
    }
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(&cli) {
        std::process::exit(pk_cli_core::output::fail(&e, cli.common.json));
    }
}
