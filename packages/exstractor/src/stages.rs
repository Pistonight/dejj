use std::collections::BTreeMap;
use std::sync::Arc;

use cu::pre::*;

use dejj_utils::Config;
use exstructs::{GoffMap, LType, MType, NamespaceMaps, SymbolInfo};

/// Mid-level (M) type stage
pub struct MStage {
    pub offset: usize,
    pub name: String,
    pub types: GoffMap<MType>,
    pub config: Arc<Config>,
    pub symbols: BTreeMap<String, SymbolInfo>,
}

impl MStage {
    /// Link 2 stages together to become 1 stage
    pub fn link(mut self, other: Self) -> cu::Result<Self> {
        self.types.extend(other.types);
        for s in other.symbols.into_values() {
            if let Some(symbol) = self.symbols.get_mut(&s.link_name) {
                cu::check!(
                    symbol.link(&s),
                    "failed to link symbol across CU: {}",
                    other.name
                )?;
            } else {
                self.symbols.insert(s.link_name.to_string(), s);
            }
        }
        Ok(Self {
            offset: 0,
            name: String::new(),
            types: self.types,
            config: self.config,
            symbols: self.symbols,
        })
    }
}

/// Low-level (L) type stage
pub struct LStage {
    pub offset: usize,
    pub name: String,
    pub types: GoffMap<LType>,
    pub config: Arc<Config>,
    pub ns: NamespaceMaps,
    pub symbols: BTreeMap<String, SymbolInfo>,
}
