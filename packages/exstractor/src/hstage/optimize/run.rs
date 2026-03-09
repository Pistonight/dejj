use std::sync::Arc;

use crate::hstage::optimize::{OPTIMIZERS, OptimizeContext};
use crate::stages::HStage;

/// Optimize (simplify) type layouts
pub fn run(mut stage: HStage) -> cu::Result<HStage> {
    let bar = cu::progress("stage2 -> stage3: optimizing layouts")
        .total(OPTIMIZERS.len())
        .eta(false)
        .percentage(false)
        .spawn();

    let mut changed = true;

    // the context only needs to be created once.
    // as optimization happens, some marked data will no longer be relevant,
    // but it's ok
    let mut ctx = OptimizeContext::default();
    for (k, t) in &stage.types {
        t.mark_non_eliminateable(*k, &mut ctx.non_eliminateable);
    }
    for si in stage.symbols.values() {
        si.mark_non_eliminateable(&mut ctx.non_eliminateable);
    }

    let mut next = 0;
    'outer: while changed {
        changed = false;
        for (i, optimizer) in OPTIMIZERS.iter().enumerate() {
            if i >= next {
                next = i + 1;
                cu::info!("running optimizer: {}", optimizer.name);
                cu::progress!(bar = next, "{}", optimizer.name);
            }
            if optimizer.run(&mut stage, &ctx)? {
                // after one optimization is made, re-start from the beginning
                // all optimizations
                changed = true;
                continue 'outer;
            }
        }
    }
    bar.done();

    Ok(stage)
}
