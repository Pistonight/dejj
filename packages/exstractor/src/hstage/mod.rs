use std::sync::Arc;

use exstructs::{GoffMap, HType, HTypeData, MType, SizeMap, Struct};

use crate::stages::{HStage, MStage};
use cu::pre::*;

mod optimize;
mod split;
// mod optimize_layout;

pub async fn from_mstage(stage: MStage) -> cu::Result<HStage> {
    let stage = convert_from_mstage(stage)?;
    let stages = cu::check!(split::run(stage), "failed to split hstage")?;
    cu::info!(
        "there are {} connected components to optimize in the type graph",
        stages.len()
    );

    // optimize each component in parallel
    // it's hard to parallelize each component because the types depend on each other
    // (and there could be circular references as well)
    let stage = {
        let bar = cu::progress("stage2 -> stage3: optimizing layouts")
            .total(stages.len())
            .spawn();
        let pool = cu::co::pool(-1);
        let mut handles = Vec::with_capacity(stages.len());
        for stage in stages {
            let handle = pool.spawn(async move { optimize::run(stage) });
            handles.push(handle);
        }

        let mut stage = None;
        let mut set = cu::co::set(handles);
        while let Some(result) = set.next().await {
            let result = cu::check!(result?, "failed to optimize types")?;
            cu::progress!(bar += 1);
            match &mut stage {
                None => {
                    stage = Some(result);
                }
                Some(stage) => {
                    stage.types.extend(result.types);
                    stage.symbols.extend(result.symbols);
                    stage.name_graph.extend(&result.name_graph);
                }
            }
        }
        cu::check!(stage, "unexpected: there are no stages to optimize")?
    };

    // cu::unimplemented!()
    //
    // cu::check!(
    //     optimize_layout::run(&mut stage),
    //     "failed to optimize type layouts"
    // )?;
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
