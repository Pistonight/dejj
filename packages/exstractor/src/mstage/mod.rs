use exstructs::{GoffSet, MType, algorithm};

use crate::stages::MStage;

mod link_merge;

pub async fn link_mstages(mut stages: Vec<MStage>) -> cu::Result<MStage> {
    cu::ensure!(!stages.is_empty(), "no CUs to merge")?;
    let stage = {
        let total = stages.len() - 1;
        let bar = cu::progress("stage1 -> stage2: merging types")
            .total(total)
            .eta(false)
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
        stage
    };

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
