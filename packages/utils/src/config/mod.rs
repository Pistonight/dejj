use std::collections::BTreeSet;
use std::path::Path;

mod paths;
pub use paths::*;
mod extract;
pub use extract::*;

use cu::pre::*;
use tyyaml::Prim;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub hash: u64,
    pub paths: PathsConfig,
    pub extract: ExtractConfig,
}

impl Config {
    /// Load config from a file
    pub fn load(path: impl AsRef<Path>) -> cu::Result<Self> {
        let path = path.as_ref();
        let file_content = cu::fs::read_string(path)?;
        let hash = fxhash::hash64(&file_content);
        let mut config = toml::parse::<Config>(&file_content)?;
        config.hash = hash;

        let base = path.parent_abs()?;
        config.paths.resolve_paths(&base)?;

        // validate [extract]
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
        if config.extract.build_command.is_empty() {
            cu::bail!("config.extract.build-command must be non-empty")
        }
        config.extract.name_resolution.test_rules()?;

        let mut seen_regex = BTreeSet::new();
        for rule in &config.extract.type_optimizer.pick_union_member {
            if !seen_regex.insert(rule.regex.to_str()) {
                cu::bail!("pick-union-member rule has duplicate rule: {}", rule.regex);
            }
            if rule.members.is_empty() {
                cu::bail!("pick-union-member rule has empty members: {}", rule.regex);
            }
            if rule.pick >= rule.members.len() {
                cu::bail!(
                    "pick-union-member rule has a pick out of bound: {}",
                    rule.regex
                );
            }
        }

        Ok(config)
    }
}
