use exstructs::{GoffMap, HType, HTypeData, MType, Struct};

use crate::stages::{HStage, MStage};
use cu::pre::*;

mod optimize_layout;

pub fn from_mstage(stage: MStage) -> cu::Result<HStage> {
    let mut stage = convert_from_mstage(stage);
    cu::check!(
        optimize_layout::run(&mut stage),
        "failed to optimize type layouts"
    )?;
    Ok(stage)
}

fn convert_from_mstage(stage: MStage) -> HStage {
    let mut types = GoffMap::default();
    let mut sizes = GoffMap::default();
    for (k, t) in stage.types {
        let fqnames = t.fullqual_names();
        let t = match t {
            MType::Prim(prim) => HType::Prim(prim),
            MType::Enum(data) => HType::Enum(HTypeData {
                fqnames,
                data: data.data,
            }),
            MType::Union(data) => HType::Union(HTypeData {
                fqnames,
                data: data.data,
            }),
            MType::Struct(data) => HType::Struct(HTypeData {
                fqnames,
                data: data.data,
            }),
            MType::EnumDecl(_) | MType::UnionDecl(_) | MType::StructDecl(_) => {
                HType::Struct(HTypeData {
                    fqnames,
                    data: Struct::zst(),
                })
            }
        };
        let s = match &t {
            HType::Prim(prim) => prim.byte_size(),
            HType::Enum(data) => Some(data.data.byte_size),
            HType::Union(data) => Some(data.data.byte_size),
            HType::Struct(data) => Some(data.data.byte_size),
        };
        types.insert(k, t);
        sizes.insert(k, s);
    }

    HStage {
        types,
        sizes: sizes.into(),
        config: stage.config,
        symbols: stage.symbols,
        name_graph: Default::default(),
    }
}
