use std::sync::Arc;

use cu::pre::*;
use exstructs::{GoffMap, GoffSet, HType, HTypeData, MType, SizeMap, Struct};

use crate::stages::{HStage, MStage};

mod optimize;
mod split;
// mod optimize_layout;

pub async fn from_mstage(stage: MStage) -> cu::Result<HStage> {
    let stage = convert_from_mstage(stage)?;
    let mut stage = {
        cu::cli::set_thread_name("type-optimizer");
        let result = optimize::run(stage);
        cu::cli::reset_thread_name();
        result?
    };

    if stage
        .config
        .extract
        .type_optimizer
        .only_keep_referenced_from_symbols
    {
        // starting from types referenced by any symbols, only keep referenced types
        let mut marked = GoffSet::default();
        for symbol in stage.symbols.values() {
            symbol.mark(&mut marked);
        }
        let mut newly_marked = GoffSet::default();
        loop {
            newly_marked.clear();
            for k in &marked {
                let t = cu::check!(
                    stage.types.get(k),
                    "unexpected unlinked type {k} while sweeping hstage"
                )?;
                t.mark(*k, &mut newly_marked);
            }
            let len_before = marked.len();
            marked.extend(newly_marked.iter().copied());
            if marked.len() == len_before {
                break;
            }
        }
        stage.types.retain(|k, _| marked.contains(k));
    }
    Ok(stage)
}

fn convert_from_mstage(stage: MStage) -> cu::Result<HStage> {
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
    let sizes = SizeMap::new(
        sizes,
        stage.config.extract.pointer_size()?,
        stage.config.extract.ptmd_size()?,
        stage.config.extract.ptmf_size()?,
    );

    Ok(HStage {
        types,
        sizes: Arc::new(sizes),
        config: stage.config,
        symbols: stage.symbols,
        name_graph: Default::default(),
    })
}
