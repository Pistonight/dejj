mod paths;
use std::path::Path;

pub use paths::*;
mod extract;
pub use extract::*;

use cu::pre::*;
use tyyaml::Prim;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub paths: PathsConfig,
    pub extract: ExtractConfig,
}

impl Config {
    /// Load config from a file
    pub fn load(path: impl AsRef<Path>) -> cu::Result<Self> {
        let path = path.as_ref();
        let file_content = cu::fs::read_string(path)?;
        let mut config = toml::parse::<Config>(&file_content)?;

        let base = path.parent_abs()?;
        config.paths.resolve_paths(&base)?;

        // validate [extract]
        if config.extract.build_command.is_empty() {
            cu::bail!("config.extract.build-command must be non-empty")
        }
        config.extract.name_resolution.test_rules()?;
        match config.extract.pointer_width {
            8 | 16 | 32 | 64 => {}
            _ => cu::bail!("invalid config.extract.pointer-width. must be 8, 16, 32 or 64"),
        }

        if Prim::Void == config.extract.ptmf_repr.0 {
            cu::bail!("PTMF repr type must be sized");
        }
        if config.extract.ptmf_repr.1 == 0 {
            cu::bail!("PTMF repr type must be non-zero size");
        }
        if Prim::Void == config.extract.ptmd_repr.0 {
            cu::bail!("PTMD repr type must be sized");
        }
        if config.extract.ptmd_repr.1 == 0 {
            cu::bail!("PTMD repr type must be non-zero size");
        }

        Ok(config)
    }
}
