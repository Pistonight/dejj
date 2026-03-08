use std::collections::{BTreeMap, BTreeSet};

use cu::pre::*;
use exstructs::{
    FullQualName, Goff, GoffBuckets, GoffMap, GoffPair, GoffSet, HType, HTypeData, MType,
    MTypeData, Struct, algorithm,
};
use tyyaml::Tree;

use crate::stages::HStage;

mod util;

/// Optimize (simplify) type layouts
pub fn run(mut stage: HStage) -> cu::Result<HStage> {
    Ok(stage)
}
