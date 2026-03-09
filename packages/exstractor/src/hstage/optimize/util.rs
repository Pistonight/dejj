use cu::pre::*;
use exstructs::{FullQualName, FullQualNameMap, Goff, GoffMap, GoffSet, NamespacedName, NamespacedTemplatedName};
use exstructs::algorithm::FullQualPermutater;
use regex::Regex;
use tyyaml::Tree;

use crate::stages::HStage;

/// Optimizatation function type
///
/// Each optimizer must only perform one change at a time, then all optimizers need
/// to be re-evaluated.
///
/// Let's say there are two optimizations A and B, both computed from the state S.
/// It's possible the state after applying just A - A(S), could have an optimization C
/// that if both changes are applied at once - B(A(S)), does not happen anymore
#[derive(Clone, Copy)]
pub struct Optimizer {
    /// Name of the optimizer
    pub name: &'static str,
    /// Optimize function type
    pub f: fn(&mut HStage, &OptimizeContext) -> cu::Result<bool /* changed */>
}
impl Optimizer {
    pub fn run(self, stage: &mut HStage, ctx: &OptimizeContext) -> cu::Result<bool> {
        (self.f)(stage, ctx)
    }
}
macro_rules! make_optimizer {
    ($fn:expr) => {
        $crate::hstage::optimize::Optimizer { name: stringify!($fn), f: $fn }
    }
}
pub(crate) use make_optimizer;

#[derive(Default)]
pub struct OptimizeContext {
    /// Type goffs that cannot be replaced with a tree
    pub non_eliminateable: GoffSet,
}

#[cu::context("failed to eliminate and merge with base (type={elim_k}, replace={replace:#?})")]
pub fn eliminate_unchecked_and_give_names_to_base(
    stage: &mut HStage, elim_k: Goff, replace: &Tree<Goff>
) -> cu::Result<()> {
    eliminate_unchecked(stage, elim_k, replace)?;
    // remove this type in the stage
    let t = cu::check!(stage.types.remove(&elim_k), "unexpected: type {elim_k} was already removed")?;
    // give the names to inner type
    if let Tree::Base(member_goff) = &replace {
        let mut fqnames = t.into_fqnames()?;
        // also replace the type within names of itself
        fqnames.retain_mut(|name| {
            // replace goff in the name
            name.replace(elim_k, replace).is_ok()
        });
        give_names_to_base(stage, *member_goff, &fqnames)?;
    }
    Ok(())
}

#[cu::context("failed to check_eliminate (type={elim_k}, replace={replace:#?})")]
pub fn check_eliminate(
    stage: &HStage, elim_k: Goff, replace: &Tree<Goff>, ctx: &OptimizeContext
) -> cu::Result<bool> {
    if !matches!(replace, Tree::Base(_)) {
        // replacing with a composite type
        if ctx.non_eliminateable.contains(&elim_k) {
            // the base type is not eliminatable, ignore it
            return Ok(false);
        }
    }
    if replace.contains(&elim_k) {
        cu::bail!("replacement tree contains the type to replace: tree: {replace:#?}, contains: {elim_k}");
    }
    for (k, t) in &stage.types {
        if *k == elim_k {
            // this type will be removed
            return Ok(true);
        }
        if t.contains_goff(elim_k) {
            // some replacement can happen
            return Ok(true);
        }
    }
    for si in stage.symbols.values() {
        if si.contains_goff(elim_k)  {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Eliminate the type `elim_k`, replace all occurances with the type tree,
/// but does not remove or modify the type in the stage yet
///
/// Must call check_eliminate first to check if the type can be eliminated
#[cu::context("failed to eliminate_unchecked (type={elim_k}, replace={replace:#?})")]
pub fn eliminate_unchecked(
    stage: &mut HStage, elim_k: Goff, replace: &Tree<Goff>
) -> cu::Result<()> {
    for (k, t) in &mut stage.types {
        if *k == elim_k {
            // this type will be removed
            continue;
        }
        cu::check!(
            t.replace(elim_k, replace),
            "failed to replace {elim_k} with {replace:#?} (in type {k})"
        )?;
    }
    for si in stage.symbols.values_mut() {
        cu::check!(
            si.replace(elim_k, replace),
            "failed to replace type in symbol"
        )?;
    }
    Ok(())
}

/// Give the names of the derived type to its base type,
/// adding to the name graph of the derivation relationship,
/// as the derived type is about to be eliminated
#[cu::context("failed to give name to base (base={base_goff}, names={fqnames:#?})")]
pub fn give_names_to_base(
    stage: &mut HStage,
    base_goff: Goff,
    fqnames: &[FullQualName],
) -> cu::Result<bool> {
    if fqnames.is_empty() {
        // no names to give
        return Ok(false);
    }
    let base_t = cu::check!(stage.types.get_mut(&base_goff), "unexpected unlinked type {base_goff} while giving names to base")?;
    let Ok(base_fqnames) = base_t.fqnames() else {
        // base is a primitive
        return Ok(false);
    };
    for base in base_fqnames {
        for derived in fqnames {
            stage.name_graph.add_derived(derived, base)?;
        }
    }
    base_t.add_fqnames(fqnames);
    Ok(true)
}

/// Compute fqnames for name-based type optimizer rules
pub fn compute_fqnames(stage: &HStage) -> cu::Result<FullQualNameMap> {
    let mut fullqual_names = GoffMap::default();
    for (k, t) in &stage.types {
        if let Some(prim) = k.to_prim() {
            fullqual_names.insert(*k, 
                vec![FullQualName::Name(NamespacedTemplatedName::new(
                    NamespacedName::prim(prim),
                ))]
            );
            continue;
        };
        fullqual_names.insert(*k, t.fqnames()?.to_vec());
    }
    Ok(fullqual_names.into())
}

pub fn match_unique_fqname<'a>(
    permutater: &mut FullQualPermutater,
    regex: &Regex,
    goffs: impl Iterator<Item=&'a Goff>,
) -> cu::Result<Option<(Goff, String)>> {
    let matched = match_fqname(permutater, regex, goffs)?;
    if matched.is_empty() {
        return Ok(None);
    }
    if matched.len() > 1 {
        cu::bail!("multiple match found: {matched:#?}");
    }
    Ok(Some(matched.into_iter().next().unwrap()))
}
pub fn match_fqname<'a>(
    permutater: &mut FullQualPermutater,
    regex: &Regex,
    goffs: impl Iterator<Item=&'a Goff>,
) -> cu::Result<Vec<(Goff, String)>> {
    // there shouldn't be too many matches, most of the times only 1
    let mut matched = Vec::with_capacity(8);
    for k in goffs {
        let k = *k;
        let fullqual_names = permutater.permutated_fullqual_names(k)?;

        for name in fullqual_names {
            if regex.is_match(&name) {
                matched.push((k, name));
            }
        }
    }
    Ok(matched)
}
