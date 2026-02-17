use std::collections::{BTreeMap, BTreeSet};

use cu::pre::*;
use tyyaml::Tree;

use super::super::bucket::GoffBuckets;
use super::super::deduper;
use super::pre::*;

/// Optimize (simplify) type layouts
pub fn optimize_layout(stage: &mut Stage1) -> cu::Result<()> {
    for optimize_fn in OPTIMIZERS {
        let mut ctx = OptimizeContext::default();
        for (k, t) in &stage.types {

        }
        let output = optimize_fn(stage, &ctx)?;

    }
    Ok(())
}

static OPTIMIZERS: &[fn(&Stage1, &OptimizeContext) -> cu::Result<OptimizeOutput>] = &[

    optimize_little_member_union,
    // optimize_single_type_union,
    // optimize_single_member_struct,
    // optimize_single_base_member_struct,

];

#[derive(Default)]
struct OptimizeContext {
    /// Type goffs that cannot be replaced with a tree
    non_replacable: GoffSet,
}

#[derive(Default)]
struct OptimizeOutput {
    /// Eliminate the type by substituting all occurrence with a type tree.
    /// This is applied first.
    eliminations: Eliminations,
    /// Change one type to another data. This is applied second
    changes: GoffMap<Type1>,
    /// Merge 2 types - the merge rules apply. This is applied last
    merges: Vec<GoffPair>,
}
impl OptimizeOutput {
    fn apply(self, stage: &mut Stage1) -> cu::Result<()> {
        for (k, tree) in self.eliminations.data {
        }
    }
}

#[derive(Default)]
struct Eliminations {
    data: GoffMap<Tree<Goff>>,
}

impl Eliminations {
    fn insert(&mut self, k: Goff, tree: Tree<Goff>) -> cu::Result<()> {
            use std::collections::btree_map::Entry;
        match self.data.entry(k) {
            Entry::Vacant(e) => {
                e.insert(tree);
            }
            Entry::Occupied(e) => {
                if e.get() != &tree {
                    cu::bail!("conflicting elimination for {k}: new={tree:#?}, old={:#?}", e.get());
                }
            }
        }
        Ok(())
    }
}

/// Eliminate unions with fewer than 2 members
fn optimize_little_member_union(stage: &Stage1, ctx: &OptimizeContext) -> cu::Result<OptimizeOutput> {
    let mut output = OptimizeOutput::default();
    for (k, t) in &stage.types {
        let Type1::Union(name, data, other_names) = t else {
            continue;
        };
        match data.members.len() {
            0 => {
                // empty union is the same as an empty struct - a ZST (zero sized type, which has a
                // sizeof() of 1
                cu::ensure!(data.byte_size == 1, "expect empty union to be a ZST, but its size is {}", data.byte_size)?;
                let new_data = Type0Struct { 
                    template_args: data.template_args.clone(), 
                    byte_size: 1, 
                    vtable: vec![], 
                    members: vec![]
                };
                output.changes.insert(*k, Type1::Struct(name.clone(), new_data, other_names.clone()));
            }
            1 => {
                let member = &data.members[0];
                if !helper::can_eliminate(*k, t, &member.ty, ctx) {
                    continue;
                }
                output.eliminations.insert(*k, member.ty.clone())?;
            }
            _ => {}
        }
    }
    Ok(output)
}

/// collapse the union if all members are the same type
fn optimize_single_type_union(stage: &Stage1, ctx: &OptimizeContext) -> cu::Result<OptimizeOutput> {
    let mut output = OptimizeOutput::default();
    for (k, t) in &stage.types {
        let Type1::Union(_, data, _) = t else {
            continue;
        };
        let mut the_type = None;
        for member in &data.members {
            match &the_type {
                None => {
                    the_type = Some(member.ty.clone());
                }
                Some(old) => {
                    if old != &member.ty {
                        continue;
                    }
                }
            }
        }
        let Some(only_type) = the_type else {
            continue;
        };
        if !helper::can_eliminate(*k, t, &only_type, ctx) {
            continue;
        }
        output.eliminations.insert(*k, only_type)?;
    }
    Ok(output)
}

mod helper {
    use super::*;
    pub fn can_eliminate(k: Goff, t: &Type1, replace: &Tree<Goff>, ctx: &OptimizeContext) -> bool {
        if t.is_layout_directly_recursive(k) {
            // if the type contains members that references itself, we cannot replace it
            // but mutual recursive references are fine
            return false;
        }
        if !matches!(replace, Tree::Base(_)) && ctx.non_replacable.contains(&k) {
            // the type is not replacable with a (non-base) tree
            return false;
        }
        // the replacement tree cannot recursively contain the type it tries to replace
        // (this shouldn't be legal to compile, but something could happen while
        // reducing the types
        let result = replace.for_each(|x| if *x == k {
            cu::bail!("recursive")
        } else {
                Ok(())
            });
        if result.is_err() {
            return false;
        }

        true
    }
}

// /// If a union has 2 members of the same size, pick one
// // #[distributed_slice(OPTIMIZERS)]
// fn optimize_two_members_union(compiler: &mut TypeCompilerUnit, _: bool) -> cu::Result<bool> {
//     let mut changed = false;
//     let mut to_check = vec![];
//     for bucket in compiler.compiled.buckets() {
//         let unit = &bucket.value;
//         let Some(TypeUnitData::Union(data)) = &unit.data else {
//             continue;
//         };
//         if data.members.len() != 2 {
//             continue;
//         }
//         let bucket_key = bucket.canonical_key();
//         let member_type = data.members[0].1;
//         let member_anon = data.members[0].0.is_none();
//         let member_type2 = data.members[1].1;
//         let member_anon2 = data.members[1].0.is_none();
//         to_check.push((bucket_key, member_type, member_type2, member_anon, member_anon2));
//     }
//
//     for (bucket_key, member_type, member_type2, member_anon, member_anon2) in to_check {
//         let member_size = compiler.resolve_size(member_type)?;
//         let member_size2 = compiler.resolve_size(member_type2)?;
//         let mut pick_member = 0;
//         if member_size != member_size2 {
//             // pick the larger member
//             if member_size > member_size2 {
//                 pick_member = 1;
//             } else {
//                 pick_member = 2;
//             }
//         }
//         // pick the anonymous member
//         if pick_member == 0 {
//             if member_anon {
//                 if !member_anon2 {
//                     pick_member = 1;
//                 }
//             } else if member_anon2 {
//                 pick_member = 2
//             }
//         }
//         // pick the "more complex" member
//         if pick_member == 0 {
//             let complexity1 = compiler.resolve_complexity(member_type)?;
//             let complexity2 = compiler.resolve_complexity(member_type2)?;
//             if complexity1 > complexity2 {
//                 pick_member = 1;
//             } else if complexity1 < complexity2 {
//                 pick_member = 2;
//             } else {
//                 cu::debug!("same complexity: {member_type} and {member_type2}: {complexity1}")
//             }
//         }
//         if pick_member == 0 {
//             continue;
//         }
//         if pick_member == 1 {
//             if member_type != bucket_key {
//                 let member_type_unit = compiler.compiled.get_unwrap(member_type)?;
//                 if let Some(member_data) = &member_type_unit.value.data {
//                     if !member_data.is_recursive_to(bucket_key) {
//                         compiler.merges.push((bucket_key, member_type));
//                         changed = true;
//                     }
//                 }
//             }
//             continue;
//         }
//         if member_type2 != bucket_key {
//             let member_type_unit = compiler.compiled.get_unwrap(member_type2)?;
//             if let Some(member_data) = &member_type_unit.value.data {
//                 if !member_data.is_recursive_to(bucket_key) {
//                     compiler.merges.push((bucket_key, member_type2));
//                     changed = true;
//                 }
//             }
//         }
//     }
//     Ok(changed)
// }

// /// Flatten the struct if it only has one member, and is non-recursive and non-virtual
// // #[distributed_slice(OPTIMIZERS)]
// fn optimize_single_member_struct(compiler: &mut TypeCompilerUnit, is_linking: bool) -> cu::Result<bool> {
//     // if !is_linking {
//     //     return Ok(false);
//     // }
//     let mut changed = false;
//     for bucket in compiler.compiled.buckets() {
//         let unit = &bucket.value;
//         let Some(TypeUnitData::Struct(data)) = &unit.data else {
//             continue;
//         };
//         if !data.vtable.entries.is_empty() {
//             continue;
//         }
//         if data.members.len() != 1 {
//             continue;
//         }
//         let bucket_key = bucket.canonical_key();
//         let member_type = data.members[0].ty;
//         let member_type_unit = compiler.compiled.get_unwrap(member_type)?;
//         if member_type != bucket_key {
//             if let Some(member_data) = &member_type_unit.value.data {
//                 if !member_data.is_recursive_to(bucket_key) {
//                     compiler.merges.push((bucket_key, member_type));
//                     changed = true;
//                 }
//             }
//         }
//     }
//     Ok(changed)
// }
//
// /// Inline the base struct members, if the derived class only has one field
// /// that is the base class
// // #[distributed_slice(OPTIMIZERS)]
// fn optimize_single_base_member_struct(compiler: &mut TypeCompilerUnit, is_linking: bool) -> cu::Result<bool> {
//     // if !is_linking {
//     //     return Ok(false);
//     // }
//     let mut changed = false;
//     for bucket in compiler.compiled.buckets() {
//         let unit = &bucket.value;
//         let Some(TypeUnitData::Struct(data)) = &unit.data else {
//             continue;
//         };
//         if data.members.len() != 1 {
//             continue;
//         }
//         let member = &data.members[0];
//         if !member.is_base() {
//             continue;
//         }
//         let bucket_key = bucket.canonical_key();
//         let base_type = data.members[0].ty;
//         let base_type_unit = compiler.compiled.get_unwrap(base_type)?;
//         let Some(base_data) = &base_type_unit.value.data else {
//             continue;
//         };
//         let members = if let TypeUnitData::Struct(base_data) = base_data {
//             cu::ensure!(
//                 base_data.byte_size == data.byte_size,
//                 "unexpected single base member struct having different size than its base member"
//             );
//             base_data.members.clone()
//         } else {
//             continue;
//         };
//         let mut new_data = data.clone();
//         new_data.members = members;
//         compiler.changes.insert(bucket_key, TypeUnitData::Struct(new_data));
//         changed = true;
//     }
//     Ok(changed)
// }
