use std::{collections::BTreeMap, sync::Arc};

use cu::pre::*;
use exstructs::{GoffMap, algorithm};

use crate::stages::HStage;

/// Split one HStage into independent stages which can be processed
/// parallely
pub fn run(mut stage: HStage) -> cu::Result<Vec<HStage>> {
    let connected_components = algorithm::calc_connected_components(&stage.types, &stage.symbols)?;

    let mut split_stages = Vec::with_capacity(connected_components.len());
    for comp in &connected_components {
        let mut split_types = GoffMap::new();
        for k in &comp.types {
            let t = cu::check!(stage.types.remove(k), "unexpected unconnected type goff {k} while splitting type map")?;
            split_types.insert(*k, t);
        }
        let mut split_symbols = BTreeMap::new();
        for s in &comp.symbols {
            let (sym, info) = cu::check!(stage.symbols.remove_entry(s), "unexpected unconnected symbol {s} while splitting symbol map")?;
            split_symbols.insert(sym, info);
        }
        let split_stage = HStage { 
            types: split_types, 
            config: Arc::clone(&stage.config), 
            symbols: split_symbols, 
            sizes: Arc::clone(&stage.sizes),
            name_graph: stage.name_graph.clone()
        };
        split_stages.push(split_stage);
    }
    // there should be no symbols left
    cu::ensure!(stage.symbols.is_empty(), "{:?}", stage.symbols)?;

    // the remaining must be primitives
    for k in stage.types.keys() {
        cu::ensure!(k.is_prim(), "{k}")?;
    }
    // duplicate the primitive types into each split stage
    for split_stage in &mut split_stages {
        split_stage.types.extend(stage.types.iter().map(|(k,v)| (*k, v.clone())));
    }

    Ok(split_stages)
}
