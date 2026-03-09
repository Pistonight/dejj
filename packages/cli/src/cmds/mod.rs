use cu::pre::*;
use dejj_utils::Config;
static LOGO: &str = r" _____  ______    __    __  
/\  __-.\  ___\  /\ \  /\ \ 
\ \ \/\ \\  __\ _\_\ \_\_\ \
 \ \____- \_____\_____\_____\
  \/____/\/_____/_____/_____/";

/// Decompilation Judger
#[derive(clap::Parser, AsRef)]
#[clap(
    before_help = LOGO,
    bin_name = "dejj"
)]
pub struct CmdMain {
    /// Path to the TOML config file
    #[clap(short = 'C', long, default_value = "dejj.toml")]
    pub config: String,

    #[clap(subcommand)]
    pub cmd: Option<CmdSubcommand>,

    #[as_ref]
    #[clap(flatten)]
    flags: cu::cli::Flags,

    /// Print the version (same as the version subcommand)
    #[clap(short = 'V', long)]
    version: bool,
}
impl CmdMain {
    pub fn preprocess(&mut self) {
        if let Some(command) = &self.cmd {
            self.flags.merge(command.as_ref());
        }
    }
}

#[derive(clap::Subcommand)]
pub enum CmdSubcommand {
    Extract(CmdExtract),
    /// Print the version
    Version(cu::cli::Flags),
}

impl AsRef<cu::cli::Flags> for CmdSubcommand {
    fn as_ref(&self) -> &cu::cli::Flags {
        match self {
            Self::Extract(cmd) => cmd.as_ref(),
            Self::Version(cmd) => cmd.as_ref(),
        }
    }
}

pub fn main(args: CmdMain) -> cu::Result<()> {
    let run_version = args.version || matches!(&args.cmd, Some(CmdSubcommand::Version(_)));
    if run_version {
        cu::lv::disable_print_time();
        println!("{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let Some(cmd) = args.cmd else {
        cu::lv::disable_print_time();
        cu::cli::print_help::<CmdMain>(false);
        return Ok(());
    };

    let config = Config::load(args.config)?;

    match cmd {
        CmdSubcommand::Extract(_) => exstractor::run(config),
        CmdSubcommand::Version(_) => Ok(()),
    }
}

/// Extract database artifacts from DWARF info from an ELF file
#[derive(Debug, clap::Parser, AsRef)]
pub struct CmdExtract {
    #[clap(flatten)]
    #[as_ref]
    pub common: cu::cli::Flags,
}
