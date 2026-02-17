use exstructs::{GoffSet, MType, algorithm};

use crate::stages::MStage;

mod link_merge;
mod optimize_layout;

pub async fn to_hstage(stages: Vec<MStage>) -> cu::Result<()> {
    let mut stage = link_mstages(stages).await?;
    optimize_layout::run(&mut stage)?;

    cu::hint!("after optimization:");
    cu::print!("{:#?}", stage.types);
    let mut enum_count = 0;
    let mut union_count = 0;
    let mut struct_count = 0;
    let mut enum_decl_count = 0;
    let mut union_decl_count = 0;
    let mut struct_decl_count = 0;
    for t in stage.types.values() {
        match t {
            MType::Prim(_) => {}
            MType::Enum(_) => enum_count += 1,
            MType::Union(_) => union_count += 1,
            MType::UnionDecl(_) => union_decl_count += 1,
            MType::Struct(_) => struct_count += 1,
            MType::EnumDecl(_) => enum_decl_count += 1,
            MType::StructDecl(_) => struct_decl_count += 1,
        }
    }
    cu::print!("enum_count: {enum_count}");
    cu::print!("union_count: {union_count}");
    cu::print!("struct_count: {struct_count}");
    cu::print!("enum_decl_count: {enum_decl_count}");
    cu::print!("union_decl_count: {union_decl_count}");
    cu::print!("struct_decl_count: {struct_decl_count}");
    cu::hint!("done");
    //TODO
    Ok(())
}

async fn link_mstages(mut stages: Vec<MStage>) -> cu::Result<MStage> {
    cu::ensure!(!stages.is_empty(), "no CUs to merge")?;
    let stage = {
        let total = stages.len() - 1;
        let bar = cu::progress("stage1 -> stage2: merging types")
            .keep(false)
            .total(total)
            .spawn();
        let pool = cu::co::pool(-1);
        let mut handles = Vec::with_capacity(total / 2 + 1);
        while let Some(handle) = spawn_task(&mut stages, &pool) {
            handles.push(handle);
        }

        let mut set = cu::co::set(handles);
        while let Some(result) = set.next().await {
            let merged = result??;
            cu::progress!(bar += 1);
            stages.push(merged);
            if let Some(handle) = spawn_task(&mut stages, &pool) {
                set.add(handle);
            }
        }

        let mut stage = stages.into_iter().next().unwrap();

        let mut marked = GoffSet::default();
        for symbol in stage.symbols.values() {
            symbol.mark(&mut marked);
        }
        algorithm::mark_and_sweep(marked, &mut stage.types, MType::mark);
        cu::info!("stage2: merged into {} types", stage.types.len());
        stage
    };

    // cu::print!("{:#?}", stage.types);
    //
    let mut enum_count = 0;
    let mut union_count = 0;
    let mut struct_count = 0;
    let mut enum_decl_count = 0;
    let mut union_decl_count = 0;
    let mut struct_decl_count = 0;
    for t in stage.types.values() {
        match t {
            MType::Prim(_) => {}
            MType::Enum(_) => enum_count += 1,
            MType::Union(_) => union_count += 1,
            MType::UnionDecl(_) => union_decl_count += 1,
            MType::Struct(_) => struct_count += 1,
            MType::EnumDecl(_) => enum_decl_count += 1,
            MType::StructDecl(_) => struct_decl_count += 1,
        }
    }
    cu::print!("enum_count: {enum_count}");
    cu::print!("union_count: {union_count}");
    cu::print!("struct_count: {struct_count}");
    cu::print!("enum_decl_count: {enum_decl_count}");
    cu::print!("union_decl_count: {union_decl_count}");
    cu::print!("struct_decl_count: {struct_decl_count}");

    Ok(stage)
}

fn spawn_task(
    stages: &mut Vec<MStage>,
    pool: &cu::co::Pool,
) -> Option<cu::co::Handle<cu::Result<MStage>>> {
    if stages.len() <= 1 {
        return None;
    }
    let unit_a = stages.pop().unwrap();
    let unit_b = stages.pop().unwrap();
    let handle = pool.spawn(async move { link_merge::link_merge(unit_a, unit_b) });
    Some(handle)
}
