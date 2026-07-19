//! Command-line surface. Conforms to **piekstra-cli/1** (see cli-common's
//! `DESIGN.md`): the global flags come from the shared [`CommonArgs`], and the
//! standard `auth` / `config` / `self-update` / `completions` / `info` / `api`
//! commands are spelled exactly as the spec requires.

use clap::{Parser, Subcommand};
use clap_complete::Shell;
use pk_cli_core::CommonArgs;
use pk_cli_http::ApiArgs;

/// Look up Palm Beach County, FL property records from the command line.
///
/// Owner, mailing address, parcel control number, market and assessed values,
/// exemptions, sale history, and building details — all of it anonymous public
/// records served by the county Property Appraiser. There is no account, no API
/// key, and nothing to log into.
#[derive(Parser, Debug)]
#[command(name = "pbca", version, about, long_about = None)]
pub struct Cli {
    #[command(flatten)]
    pub common: CommonArgs,

    #[command(subcommand)]
    pub command: Command,
}

/// The parcel-control-number argument shared by the per-parcel commands.
#[derive(clap::Args, Debug)]
pub struct PcnArg {
    /// Parcel control number, dashed or bare
    /// (`12-34-56-78-90-123-4567` or `12345678901234567`).
    #[arg(value_name = "PCN", env = "PBCA_PCN")]
    pub pcn: String,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Find parcels by situs address, or by owner name with `--owner`.
    ///
    /// Matching is a server-side prefix match on the county's uppercased
    /// fields, so `pbca search "main st"` finds `100 EXAMPLE ST`. Owner names
    /// are stored surname-first (`SMITH JOHN`).
    #[command(visible_alias = "ls")]
    Search {
        /// Address fragment, or owner name with `--owner`.
        #[arg(value_name = "QUERY")]
        query: String,

        /// Search owner names instead of situs addresses.
        #[arg(long)]
        owner: bool,

        /// Only parcels in this municipality, e.g. `JUPITER`. Defaults to the
        /// `municipality` config key when set.
        #[arg(short, long, value_name = "NAME", env = "PBCA_MUNICIPALITY")]
        municipality: Option<String>,

        /// Maximum matches to return. Defaults to the `limit` config key, or 25.
        #[arg(long, value_name = "N")]
        limit: Option<u32>,
    },

    /// Show the full record for one parcel.
    Parcel {
        #[command(flatten)]
        pcn: PcnArg,

        /// Also fetch building structure (year built, square footage) from the
        /// county's detail page. Costs one extra request.
        #[arg(long)]
        details: bool,
    },

    /// List every recorded sale for a parcel, newest first.
    ///
    /// The county's parcel layer carries only the most recent sale; this reads
    /// the full history from the detail page.
    Sales {
        #[command(flatten)]
        pcn: PcnArg,

        /// Only the most recent N sales.
        #[arg(long, value_name = "N")]
        limit: Option<u32>,
    },

    /// Show the year-by-year value and tax history for a parcel.
    Taxes {
        #[command(flatten)]
        pcn: PcnArg,

        /// Only the most recent N tax years.
        #[arg(long, value_name = "N")]
        limit: Option<u32>,
    },

    /// Show building structure: year built, living area, and square footage.
    Building(PcnArg),

    /// Machine-readable capability discovery (cli-info/v1).
    Info,

    /// Report credential state. This tool needs none — it reads public records.
    #[command(subcommand)]
    Auth(AuthCmd),

    /// Non-secret settings.
    #[command(subcommand)]
    Config(ConfigCmd),

    /// Raw ArcGIS passthrough, for fields this CLI does not model.
    ///
    /// Paths resolve against the county's ArcGIS `Parcels` folder, e.g.
    /// `pbca api GET '/PARCEL_INFO/FeatureServer/4/query?where=1%3D1&returnCountOnly=true&f=json'`.
    Api(ApiArgs),

    /// Update pbca to the latest release from GitHub.
    #[command(name = "self-update")]
    SelfUpdate(SelfUpdateArgs),

    /// Print a shell completion script (e.g. `pbca completions zsh`).
    Completions {
        /// Shell to generate completions for.
        #[arg(value_enum)]
        shell: Shell,
    },
}

/// Re-exported so `main` can pass the family updater its own argument type.
pub use pk_cli_selfupdate::SelfUpdateArgs;

#[derive(Subcommand, Debug)]
pub enum AuthCmd {
    /// Report credential/session state (auth-status/v1). Always usable.
    Status,
}

#[derive(Subcommand, Debug)]
pub enum ConfigCmd {
    /// Print the resolved config file path.
    Path,
    /// Show the effective configuration.
    Show,
    /// Set a config key, e.g. `pbca config set municipality JUPITER`.
    Set { key: String, value: String },
    /// Remove a config key.
    Unset { key: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_an_address_search() {
        let cli = Cli::parse_from(["pbca", "search", "main st"]);
        match cli.command {
            Command::Search {
                query,
                owner,
                limit,
                ..
            } => {
                assert_eq!(query, "main st");
                assert!(!owner);
                // Unset here so the config default can win at resolve time.
                assert_eq!(limit, None);
            }
            other => panic!("expected search, got {other:?}"),
        }
    }

    #[test]
    fn owner_search_is_a_flag_not_a_separate_query_slot() {
        let cli = Cli::parse_from(["pbca", "search", "--owner", "smith j"]);
        match cli.command {
            Command::Search { query, owner, .. } => {
                assert_eq!(query, "smith j");
                assert!(owner);
            }
            other => panic!("expected search, got {other:?}"),
        }
    }

    #[test]
    fn json_is_global_and_works_after_the_subcommand() {
        let cli = Cli::parse_from(["pbca", "parcel", "12345678901234567", "--json"]);
        assert!(cli.common.json);
    }

    #[test]
    fn self_update_check_does_not_install() {
        let cli = Cli::parse_from(["pbca", "self-update", "--check"]);
        match cli.command {
            Command::SelfUpdate(args) => assert!(args.check),
            other => panic!("expected self-update, got {other:?}"),
        }
    }

    #[test]
    fn ls_aliases_search() {
        assert!(matches!(
            Cli::parse_from(["pbca", "ls", "main st"]).command,
            Command::Search { .. }
        ));
    }
}
