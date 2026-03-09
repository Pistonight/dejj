use cu::pre::*;
use exstructs::algorithm::FullQualPermutater;
use exstructs::{HType, HTypeData};
use tyyaml::Tree;

use crate::hstage::optimize::{OptimizeContext, util};
use crate::stages::HStage;

/// Eliminate structs with only one member
pub fn single_member(stage: &mut HStage, ctx: &OptimizeContext) -> cu::Result<bool> {
    for (k, t) in &stage.types {
        let HType::Struct(HTypeData { data, .. }) = t else {
            continue;
        };
        if data.members.len() != 1 {
            continue;
        }
        if !data.vtable.is_empty() {
            continue;
        }
        let member = &data.members[0];
        // for single member, it should equal to size of the member
        let self_size = data.byte_size;

        let member_size = stage.sizes.get_tree(&member.ty)?;
        cu::ensure!(
            self_size == member_size,
            "{k}: self_size={self_size}, member_size={member_size}"
        )?;
        let k = *k;
        if !util::check_eliminate(stage, k, &member.ty, ctx)? {
            continue;
        }
        cu::trace!("removing single-member struct {k}");
        // must clone so we can re-borrow stage as mutable
        let member = member.clone();
        // note that we give the name to the member anyway, even though
        // it might not be declared as base
        util::eliminate_unchecked_and_give_names_to_base(stage, k, &member.ty)?;
        return Ok(true);
    }
    Ok(false)
}

/// Eliminate struct/union into the associate Enum type
pub fn enumeratorize(stage: &mut HStage, ctx: &OptimizeContext) -> cu::Result<bool> {
    let rules = &stage.config.extract.type_optimizer.enumeratorize;
    if rules.is_empty() {
        // save the cost of computing permutated names
        return Ok(false);
    }

    let fullqual_names = util::compute_fqnames(stage)?;
    let mut permutater = FullQualPermutater::new(&fullqual_names);
    for rule in rules {
        let error_prefix = format!(
            "type-optimizer.enumeratorize rule struct='{}' enum='{}'",
            rule.struct_regex(),
            rule.enum_regex(),
        );

        // find an enum that matches the rule
        let enum_goff_iter = stage.types.iter().filter_map(|(k, v)| {
            if matches!(v, HType::Enum(_)) {
                Some(k)
            } else {
                None
            }
        });
        let matched_enum = cu::check!(
            util::match_unique_fqname(&mut permutater, rule.enum_regex(), enum_goff_iter),
            " failed to match an enum"
        )?;
        let Some((enum_k, enum_name)) = matched_enum else {
            // the rule did not match any enum
            continue;
        };
        let error_prefix2 =
            format!("{error_prefix} matched name {enum_name} in enum {enum_k}, but");

        // match all structs/unions
        let struct_goff_iter = stage.types.iter().filter_map(|(k, v)| {
            if matches!(v, HType::Struct(_) | HType::Union(_)) {
                Some(k)
            } else {
                None
            }
        });
        let matched_structs = cu::check!(
            util::match_fqname(&mut permutater, rule.struct_regex(), struct_goff_iter),
            "{error_prefix2} failed to match struct/unions"
        )?;
        // we only produce one change at a time, so only look at the first match
        let Some((struct_k, struct_name)) = matched_structs.into_iter().next() else {
            continue;
        };
        let error_prefix = format!(
            "{error_prefix} matched name {enum_name} in enum {enum_k} and name {struct_name} in struct/union {struct_k}, but"
        );

        let replace_tree = Tree::Base(enum_k);
        if !util::check_eliminate(stage, struct_k, &replace_tree, ctx)? {
            cu::bail!("{error_prefix} the struct/union cannot be eliminated");
        }
        cu::debug!("enumeratorize {struct_k} ({struct_name}) into {enum_k} ({enum_name})");
        util::eliminate_unchecked_and_give_names_to_base(stage, struct_k, &replace_tree)?;
        return Ok(true);
    }
    Ok(false)
}
