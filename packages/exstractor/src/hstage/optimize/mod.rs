use std::collections::{BTreeMap, BTreeSet};

use cu::pre::*;
use exstructs::{
    FullQualName, HType, HTypeData, algorithm,
    Goff, GoffBuckets, GoffMap, GoffPair, GoffSet, MType, MTypeData, Struct
};
use tyyaml::Tree;

use crate::stages::HStage;

mod util;

/// Optimize (simplify) type layouts
pub fn run(mut stage: HStage) -> cu::Result<HStage> {
    let connected_keys = algorithm::calc_connected_components(&stage.types)?;
    cu::info!("there are {} connected components to optimize in the type graph", connected_keys.len());

    // split the types into components
    let mut components = Vec::with_capacity(connected_keys.len());
    for keys in &connected_keys {
        let mut map = GoffMap::new();
        for k in keys {
            let t = cu::check!(stage.types.remove(k), "unexpected unconnected type goff {k} while splitting type map")?;
            map.insert(*k, t);
        }
        components.push(map);
    }

    // the remaining must be primitives, put them into all submaps
    for component in &mut components {
        component.extend(stage.types.iter().map(|(k,v)| (*k, v.clone())));
    }

    // optimize each component in parallel
    // it's hard to parallelize each component because the types depend on each other
    // (and there could be circular references as well)

    let bar = cu::progress("stage2 -> stage3: optimizing layouts").spawn();

    let pool = cu::co::pool(-1);
    for component in components {
        pool.spawn(async move {
            optimize_component()
        })
    }

    todo!()
}

fn optimize_component(types: GoffMap<HType>) -> cu::Result<GoffMap<HType>> {
}
