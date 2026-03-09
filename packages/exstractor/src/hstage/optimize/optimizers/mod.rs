use crate::hstage::optimize::Optimizer;
use crate::hstage::optimize::util::make_optimizer;

mod opt_struct;
mod opt_union;

pub static OPTIMIZERS: &[Optimizer] = &[
    // the optimizers from config take precedence,
    // otherwise, the configureed types might be eliminated
    // by another optimizer
    make_optimizer!(opt_union::pick_member),
    make_optimizer!(opt_struct::enumeratorize),
    make_optimizer!(opt_struct::single_member),
    make_optimizer!(opt_union::number_of_members),
    make_optimizer!(opt_union::same_type_members),
    // optimize_single_base_member_struct,
];
