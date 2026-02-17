use cu::pre::*;
use dejj_utils::Config;

#[derive(clap::Parser, AsRef)]
pub struct CmdMain {
    /// TOML config file
    #[clap(short = 'C', long)]
    pub config: String,

    #[clap(subcommand)]
    #[as_ref(cu::cli::Flags)]
    pub cmd: CmdSubcommand,
}

#[derive(clap::Subcommand)]
pub enum CmdSubcommand {
    Extract(CmdExtract),
}

impl AsRef<cu::cli::Flags> for CmdSubcommand {
    fn as_ref(&self) -> &cu::cli::Flags {
        match self {
            Self::Extract(cmd) => cmd.as_ref(),
        }
    }
}

pub fn main(args: CmdMain) -> cu::Result<()> {
    let config = Config::load(args.config)?;

    match args.cmd {
        CmdSubcommand::Extract(_) => exstractor::run(config),
    }
}

/// Extract database artifacts from DWARF info from an ELF file
#[derive(Debug, clap::Parser, AsRef)]
pub struct CmdExtract {
    #[clap(flatten)]
    #[as_ref]
    pub common: cu::cli::Flags,
}
