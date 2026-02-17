use std::path::{Path, PathBuf};

use cu::pre::*;

/// Config for project paths
///
/// For all paths, if it's a relative path, it's resolved relative to the directory
/// containing the config file
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PathsConfig {
    /// Path to the directory to invoke the build command
    pub build_dir: PathBuf,
    /// Path to the ELF file for extract.
    pub elf: PathBuf,
    /// Path to the output directory for the extract command.
    pub extract_output: PathBuf,
    /// Path to the compile_commands.json
    pub compdb: PathBuf,
    /// Path to include the system headers used by compile commands in compdb.
    ///
    /// This is needed since we need to use a newer clang with the -ast-dump=json
    /// option.
    pub system_header_paths: Vec<PathBuf>,

    /// Configuration for the functions CSV file
    ///
    /// **This is deprecated and the format for symbol listing will change in the future**
    pub functions_csv: SymListConfig,
    /// Configuration for the data CSV file
    ///
    /// **This is deprecated and the format for symbol listing will change in the future**
    pub data_csv: SymListConfig,
}

impl PathsConfig {
    pub fn resolve_paths(&mut self, base: &Path) -> cu::Result<()> {
        resolve_path(base, &mut self.build_dir)?;
        resolve_path(base, &mut self.elf)?;
        resolve_path(base, &mut self.extract_output)?;
        resolve_path(base, &mut self.compdb)?;
        self.system_header_paths
            .iter_mut()
            .map(|x| resolve_path(base, x))
            .collect::<Result<Vec<()>, _>>()?;
        resolve_path(base, &mut self.functions_csv.path)?;
        resolve_path(base, &mut self.data_csv.path)?;
        Ok(())
    }
}

/// Configuration for CSV data
///
/// **This is deprecated and the format for symbol listing will change in the future**
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SymListConfig {
    /// Path to the CSV file
    pub path: PathBuf,
    /// Base address for the address column
    pub base_address: u64,
    /// Which column is the address column, 0-indexed
    pub address_column: usize,
    /// Which column is the symbol column, 0-indexed
    pub symbol_column: usize,
    /// Skip first X rows when parsing
    #[serde(default)]
    pub skip_rows: usize,
}

fn resolve_path(base: &Path, path: &mut PathBuf) -> cu::Result<()> {
    if !path.is_absolute() {
        *path = base.join(&path).normalize()?;
    }
    Ok(())
}
