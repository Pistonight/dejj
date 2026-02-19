use std::collections::{BTreeMap, BTreeSet};

use cu::pre::*;
use exstructs::{FullQualName, HType, HTypeData, algorithm};
use exstructs::{Goff, GoffBuckets, GoffMap, GoffPair, GoffSet, MType, MTypeData, Struct};
use tyyaml::Tree;

use crate::stages::HStage;

/// Optimize (simplify) type layouts
pub fn run(stage: &mut HStage) -> cu::Result<()> {
    let mut changed = true;
    let bar = cu::progress("optimizing type layouts").spawn();
    let mut pass = 1;
    while changed {
        cu::progress!(bar, "pass {pass}");
        changed = false;
        for optimize_fn in OPTIMIZERS {
            let mut ctx = OptimizeContext::default();
            for (k, t) in &stage.types {
                t.mark_non_eliminateable(*k, &mut ctx.non_eliminateable);
            }
            for si in stage.symbols.values() {
                si.mark_non_eliminateable(&mut ctx.non_eliminateable);
            }
            let output = optimize_fn(stage, &ctx)?;
            if output.apply(stage)? {
                changed = true;
                break; // restart optimization passes if something was optimized
            }
        }
        if changed {
            let deduped = algorithm::dedupe(
                std::mem::take(&mut stage.types),
                GoffBuckets::default(),
                &mut stage.symbols,
                None,
                |data, buckets| data.map_goff(|k| Ok(buckets.primary_fallback(k))),
            );
            let deduped = cu::check!(deduped, "optimize_layout: dedupe failed")?;
            stage.types = deduped;
        }
        pass += 1;
    }
    bar.done();
    Ok(())
}

static OPTIMIZERS: &[fn(&HStage, &OptimizeContext) -> cu::Result<OptimizeOutput>] = &[
    optimize_union_fewer_than_2_members,
    // optimize_single_member_struct,
    // optimize_single_type_union,
    // optimize_single_base_member_struct,
];

#[derive(Default)]
struct OptimizeContext {
    /// Type goffs that cannot be replaced with a tree
    non_eliminateable: GoffSet,
}

type ChangeFn = Box<dyn FnOnce(HType) -> cu::Result<HType>>;

#[derive(Default)]
struct OptimizeOutput {
    /// Change one type to another data. This is applied first
    changes: GoffMap<Vec<ChangeFn>>,
    /// Eliminate the type by substituting all occurrence with a type tree.
    /// This is applied first.
    eliminations: Eliminations,
    // /// Merge 2 types - the merge rules apply. This is applied last
    // merges: Vec<GoffPair>,
    /// Add `first` is a derived name of `second`
    add_is_derived_names: Vec<(FullQualName, FullQualName)>,
}
impl OptimizeOutput {
    fn change(&mut self, k: Goff, change: HType) {
        self.change_fn(k, move |_| Ok(change))
    }
    fn change_fn(&mut self, k: Goff, change_fn: impl FnOnce(HType) -> cu::Result<HType> + 'static) {
        self.changes.entry(k).or_default().push(Box::new(change_fn))
    }
    /// Apply the optimizations, return true if anything changed
    fn apply(self, stage: &mut HStage) -> cu::Result<bool> {
        cu::debug!(
            "applying optimizations change={}, eliminations={}",
            self.changes.len(),
            self.eliminations.data.len()
        );
        // change the types
        let mut changed = false;
        for (k, changes) in self.changes {
            let old = cu::check!(
                stage.types.get_mut(&k),
                "unlinked type {k} when trying to change"
            )?;
            let mut temp = old.clone();
            for change_fn in changes {
                temp = cu::check!(
                    change_fn(temp),
                    "error running change_fn during optimization"
                )?;
            }
            if old != &temp {
                changed = true;
                *old = temp;
            }
        }
        if !self.eliminations.data.is_empty() {
            let bar = cu::progress("applying eliminations")
                .total(self.eliminations.data.len())
                .keep(false)
                .spawn();

            for (k, replacement) in &self.eliminations.data {
                if replacement.contains(&k) {
                    cu::bail!("the replacement recursively contains the type to replace");
                }
                for (j, t) in &mut stage.types {
                    if j == k {
                        continue;
                    }
                    changed |= cu::check!(
                        t.replace(*k, replacement),
                        "failed to replace {k} with {replacement:#?} (in type {j})"
                    )?;
                }
                for si in stage.symbols.values_mut() {
                    changed |= cu::check!(
                        si.replace(*k, replacement),
                        "failed to replace type in symbol"
                    )?;
                }
                cu::progress!(bar += 1);
            }
        }
        // remove types that are eliminated
        for k in self.eliminations.data.keys() {
            stage.types.remove(k);
        }
        // edit name graph
        for (derived, base) in self.add_is_derived_names {
            changed |= stage.name_graph.add_derived(&derived, &base)?;
        }
        Ok(changed)
    }
}

#[derive(Debug, Default)]
struct Eliminations {
    data: GoffMap<Tree<Goff>>,
}

impl Eliminations {
    fn insert(&mut self, k: Goff, tree: Tree<Goff>, ctx: &OptimizeContext) -> cu::Result<()> {
        if !matches!(tree, Tree::Base(_)) {
            // replacing with a composite type
            if ctx.non_eliminateable.contains(&k) {
                // not eliminatable, ignore
                return Ok(());
            }
        }
        use std::collections::btree_map::Entry;
        match self.data.entry(k) {
            Entry::Vacant(e) => {
                e.insert(tree);
            }
            Entry::Occupied(e) => {
                if e.get() != &tree {
                    cu::bail!(
                        "conflicting elimination for {k}: new={tree:#?}, old={:#?}",
                        e.get()
                    );
                }
            }
        }
        Ok(())
    }
}

/// Eliminate unions with fewer than 2 members
fn optimize_union_fewer_than_2_members(
    stage: &HStage,
    ctx: &OptimizeContext,
) -> cu::Result<OptimizeOutput> {
    let mut output = OptimizeOutput::default();
    for (k, t) in &stage.types {
        let HType::Union(HTypeData { fqnames, data }) = t else {
            continue;
        };
        match data.members.len() {
            0 => {
                output.change_fn(*k, move |t| {
                    let HType::Union(HTypeData { fqnames, data }) = t else {
                        cu::bail!("expected a union: {t:#?}");
                    };
                    // empty union is the same as an empty struct - a ZST (zero sized type, which has a
                    // sizeof() of 1
                    cu::ensure!(
                        data.byte_size == 1,
                        "expect empty union to be a ZST, but its size is {}",
                        data.byte_size
                    )?;
                    Ok(HType::Struct(HTypeData {
                        fqnames,
                        data: Struct::zst_with_templates(data.template_args),
                    }))
                });
            }
            1 => {
                // a union with only one member is equivalent to that member
                let member = &data.members[0];
                // give inner type names
                if let Tree::Base(member_goff) = &member.ty {
                    let member_t = stage.types.get(member_goff).unwrap();
                    if let Ok(member_fqnames) = member_t.fqnames() {
                        for base in member_fqnames {
                            for derived in fqnames {
                                output
                                    .add_is_derived_names
                                    .push((derived.clone(), base.clone()));
                            }
                        }
                        let fqnames = fqnames.clone();
                        output.change_fn(*member_goff, move |mut t| {
                            t.add_fqnames(fqnames);
                            Ok(t)
                        });
                    }
                }
                output.eliminations.insert(*k, member.ty.clone(), ctx)?;
            }
            _ => {}
        }
    }
    Ok(output)
}

// /// Eliminate structs with only one member
// fn optimize_single_member_struct(
//     stage: &MStage,
//     ctx: &OptimizeContext,
// ) -> cu::Result<OptimizeOutput> {
//     let mut output = OptimizeOutput::default();
//     for (k, t) in &stage.types {
//         let MType::Struct(MTypeData { data, .. }) = t else {
//             continue;
//         };
//         if data.members.len() != 1 {
//             continue;
//         }
//         if !data.vtable.is_empty() {
//             continue;
//         }
//         let member = &data.members[0];
//         if member.is_base() {
//             continue;
//         }
//         // give inner type name if it's anonymous
//         if let Tree::Base(member_goff) = &member.ty {
//             helper::assign_name_if_anon(stage, *member_goff, t, &mut output);
//         }
//         output.eliminations.insert(*k, member.ty.clone(), ctx)?;
//     }
//     Ok(output)
// }

// mod helper {
//     use super::*;
//
//     pub fn assign_name_if_anon(
//         stage: &MStage,
//         k: Goff,
//         donor: &MType,
//         output: &mut OptimizeOutput,
//     ) {
//         let Some(t) = stage.types.get(&k) else {
//             return;
//         };
//         let (donor_name, donor_decl_names, donor_templates) = match donor {
//             MType::Prim(_) => return,
//             MType::Enum(data) => (&data.name, &data.decl_names, vec![]),
//             MType::EnumDecl(_) => return,
//             MType::Union(data) => (
//                 &data.name,
//                 &data.decl_names,
//                 data.data.template_args.clone(),
//             ),
//             MType::UnionDecl(_) => return,
//             MType::Struct(data) => (
//                 &data.name,
//                 &data.decl_names,
//                 data.data.template_args.clone(),
//             ),
//             MType::StructDecl(_) => return,
//         };
//         let Some(donor_name) = donor_name else {
//             return;
//         };
//         let (name, decl_names) = match t {
//             MType::Prim(_) => return,
//             MType::Enum(data) => (&data.name, &data.decl_names),
//             MType::EnumDecl(_) => return,
//             MType::Union(data) => (&data.name, &data.decl_names),
//             MType::UnionDecl(_) => return,
//             MType::Struct(data) => (&data.name, &data.decl_names),
//             MType::StructDecl(_) => return,
//         };
//         if name.is_none() && decl_names.is_empty() {
//             let name = donor_name.clone();
//             let decl_names = donor_decl_names.clone();
//             output.change(k, move |mut t| {
//                 let (data_name, data_decl_names, data_template_args) = match &mut t {
//                     MType::Prim(_) => return t,
//                     MType::Enum(data) => (&mut data.name, &mut data.decl_names, None),
//                     MType::Union(data) => (
//                         &mut data.name,
//                         &mut data.decl_names,
//                         Some(&mut data.data.template_args),
//                     ),
//                     MType::Struct(data) => (
//                         &mut data.name,
//                         &mut data.decl_names,
//                         Some(&mut data.data.template_args),
//                     ),
//                     MType::EnumDecl(_) => return t,
//                     MType::UnionDecl(_) => return t,
//                     MType::StructDecl(_) => return t,
//                 };
//                 *data_name = Some(name.clone());
//                 let mut new_decl_names = BTreeSet::new();
//                 new_decl_names.extend(data_decl_names.clone());
//                 new_decl_names.extend(decl_names);
//                 *data_decl_names = new_decl_names.into_iter().collect();
//                 if let Some(template_args) = data_template_args {
//                     *template_args = donor_templates;
//                 }
//                 t
//             })
//         }
//     }
// }

// /// collapse the union if all members are the same type
// fn optimize_single_type_union(stage: &MStage, ctx: &OptimizeContext) -> cu::Result<OptimizeOutput> {
//     let mut output = OptimizeOutput::default();
//     for (k, t) in &stage.types {
//         let Type1::Union(_, data, _) = t else {
//             continue;
//         };
//         let mut the_type = None;
//         for member in &data.members {
//             match &the_type {
//                 None => {
//                     the_type = Some(member.ty.clone());
//                 }
//                 Some(old) => {
//                     if old != &member.ty {
//                         continue;
//                     }
//                 }
//             }
//         }
//         let Some(only_type) = the_type else {
//             continue;
//         };
//         if !helper::can_eliminate(*k, t, &only_type, ctx) {
//             continue;
//         }
//         output.eliminations.insert(*k, only_type)?;
//     }
//     Ok(output)
// }

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
