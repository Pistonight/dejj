use cu::pre::*;
use exstructs::algorithm::FullQualPermutater;
use exstructs::{FullQualName, FullQualNameMap, GoffMap, HType, HTypeData, NamespacedName, NamespacedTemplatedGoffName, NamespacedTemplatedName, Struct};
use tyyaml::Tree;

use crate::hstage::optimize::{OptimizeContext, util};
use crate::stages::HStage;

/// Based on number of union members, optimize the union
pub fn number_of_members(
    stage: &mut HStage,
    ctx: &OptimizeContext,
) -> cu::Result<bool> {
    for (k, t) in &stage.types {
        let HType::Union(HTypeData { data, .. }) = t else {
            continue;
        };
        let k = *k;
        match data.members.len() {
            // Eliminate unions with fewer than 2 members
            0 => {
                // empty union is the same as an empty struct - a ZST (zero sized type, which has a
                // sizeof() of 1
                cu::ensure!(
                    data.byte_size == 1,
                    "expect empty union to be a ZST, but its size is {}",
                    data.byte_size
                )?;
                cu::trace!("removing empty union {k}");
                stage.types.entry(k).and_modify(|x| {
                    let data = x.as_union_mut().unwrap();
                    let fqnames = std::mem::take(&mut data.fqnames);
                    let template_args = std::mem::take(&mut data.data.template_args);
                    *x = HType::Struct(HTypeData {
                        fqnames,
                        data: Struct::zst_with_templates(template_args),
                    });
                });
                return Ok(true);
            }
            1 => {
                // a union with only one member is equivalent to that member
                let member = &data.members[0];
                if !util::check_eliminate(stage, k, &member.ty, ctx)? {
                    continue;
                }
                cu::trace!("removing single-member union {k}");
                // must clone so we can re-borrow stage as mutable
                let member = member.clone();
                util::eliminate_unchecked_and_give_names_to_base(
                    stage, k, &member.ty
                )?;
                return Ok(true);
            }
            2 => {
                // 2 members, we optimize using the following heuristic:
                // - if one member is a base type and the other is a composite type
                // - if the base type has the same size as the union
                // then, eliminate this union to be the base type
                let member1 = &data.members[0];
                let member2 = &data.members[1];
                let member1_is_basety = matches!(member1.ty, Tree::Base(k) if !k.is_prim());
                let member2_is_basety = matches!(member2.ty, Tree::Base(k) if !k.is_prim());
                if member1_is_basety == member2_is_basety {
                    continue;
                }

                let mut m = 2;
                if member1_is_basety && stage.sizes.get_tree(&member1.ty)? == data.byte_size {
                    m = 0;
                } else if member2_is_basety && stage.sizes.get_tree(&member2.ty)? == data.byte_size {
                    m = 1;
                }
                if m == 2 {
                    continue;
                }
                // eliminate to m
                let member = &data.members[m];
                if !util::check_eliminate(stage, k, &member.ty, ctx)? {
                    continue;
                }
                cu::trace!("removing dual-member union {k} as member {m}");
                // must clone so we can re-borrow stage as mutable
                let member = member.clone();
                util::eliminate_unchecked_and_give_names_to_base(
                    stage, k, &member.ty
                )?;
                return Ok(true);

            }
            _ => {}
        }
    }
    Ok(false)
}

/// If all members of the union are of the same type, optimize the union out
pub fn same_type_members(
    stage: &mut HStage,
    ctx: &OptimizeContext,
) -> cu::Result<bool> {
    'outmost: for (k, t) in &stage.types {
        let HType::Union(HTypeData { data, .. }) = t else {
            continue;
        };
        let k = *k;
        let Some(member1) = data.members.first() else {
            continue;
        };
        for member in data.members.iter().skip(1) {
            if member1.ty != member.ty {
                continue 'outmost;
            }
        }
        let member = member1;
        if !util::check_eliminate(stage, k, &member.ty, ctx)? {
            continue;
        }
        cu::debug!("removing all-same-type-member union {k}");
        // must clone so we can re-borrow stage as mutable
        let member = member.clone();
        util::eliminate_unchecked_and_give_names_to_base(
            stage, k, &member.ty
        )?;
        return Ok(true);
    }
    Ok(false)
}

/// Pick union member based on config
pub fn pick_member(
    stage: &mut HStage,
    ctx: &OptimizeContext,
) -> cu::Result<bool> {
    let rules = &stage.config.extract.type_optimizer.pick_union_member;
    if rules.is_empty() {
        // save the cost of computing permutated names
        return Ok(false);
    }

    let fullqual_names = util::compute_fqnames(stage)?;
    let mut permutater = FullQualPermutater::new(&fullqual_names);

    for rule in rules {
        let error_prefix = format!(
            "type-optimizer.pick-union-member rule '{}'",
            rule.regex
        );
        let goff_iter = stage.types.iter().filter_map(|(k, v)| {
            if matches!(v, HType::Union(_)) {
                Some(k)
            } else {
                None
            }
        });
        let matched = cu::check!(util::match_unique_fqname(&mut permutater, &rule.regex, goff_iter), "{error_prefix} failed to match")?;
        let Some((k, name)) = matched else {
            continue;
        };
        let data = &stage.types.get(&k).unwrap().as_union_unchecked().data;
        let error_prefix = format!(
            "{error_prefix} matched name {name:?} in type {k}, but",
        );

        let mut matched = true;
        if rule.members.len() == data.members.len() {
            for (member_name, member) in std::iter::zip(&rule.members, &data.members) {
                match &member.name {
                    Some(n) => {
                        if member_name != n.as_ref() {
                            matched = false;
                        }
                    }
                    None => {
                        if !member_name.is_empty() {
                            matched = false;
                        }
                    }
                }
            }
        }
        if !matched {
            cu::bail!("{error_prefix} the members don't match: {:?} != {:#?}", rule.members, data.members);
        }
        let member = cu::check!(data.members.get(rule.pick), "{error_prefix} the rule has an out of bound pick")?;

        if data.byte_size != stage.sizes.get_tree(&member.ty)? {
            cu::bail!("{error_prefix} the picked member and the union have different sizes");
        }

        if !util::check_eliminate(stage, k, &member.ty, ctx)? {
            cu::bail!("{error_prefix} the type cannot be eliminated");
        }
        cu::debug!("picking member {} for union {} ({})", rule.members[rule.pick], k, name);
        // must clone so we can re-borrow stage as mutable
        let member = member.clone();
        util::eliminate_unchecked_and_give_names_to_base(
            stage, k, &member.ty
        )?;
        return Ok(true);
    }

    Ok(false)
}
