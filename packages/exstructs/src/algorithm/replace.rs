//! Replace a goff with a Tree of goffs

use cu::pre::*;
use tyyaml::Tree;

use crate::{
    FullQualName, Goff, HType, HTypeData, Member, NameSeg, Namespace, NamespacedName,
    NamespacedTemplatedGoffName, NamespacedTemplatedName, Struct, SymbolInfo, TemplateArg, Union,
    VtableEntry,
};

impl HType {
    pub fn replace(&mut self, k: Goff, replacement: &Tree<Goff>) -> cu::Result<bool> {
        let mut changed = false;
        match self {
            Self::Prim(_) => {}
            Self::Enum(HTypeData { fqnames, .. })
            | Self::Union(HTypeData { fqnames, .. })
            | Self::Struct(HTypeData { fqnames, .. }) => {
                fqnames.retain_mut(|name| {
                    // replace goff in the name
                    match name.replace(k, replacement) {
                        Ok(c) => {
                            changed |= c;
                            true
                        }
                        // now that a referenced type needs to be eliminated
                        // and this name contains it, delete this name
                        Err(_) => false,
                    }
                });
            }
        }
        match self {
            Self::Prim(_) => {}
            Self::Enum(_) => {}
            Self::Union(data) => {
                changed |= data.data.replace(k, replacement)?;
            }
            Self::Struct(data) => {
                changed |= data.data.replace(k, replacement)?;
            }
        }
        Ok(changed)
    }
}

impl Union {
    pub fn replace(&mut self, k: Goff, replacement: &Tree<Goff>) -> cu::Result<bool> {
        let mut changed = false;
        for member in &mut self.members {
            changed |= cu::check!(
                member.replace(k, replacement),
                "failed to replace type in union member"
            )?;
        }
        for targ in &mut self.template_args {
            changed |= cu::check!(
                targ.replace(k, replacement),
                "failed to replace type in union template args"
            )?;
        }
        Ok(changed)
    }
}
impl Struct {
    pub fn replace(&mut self, k: Goff, replacement: &Tree<Goff>) -> cu::Result<bool> {
        let mut changed = false;
        for (_, ventry) in &mut self.vtable {
            changed |= cu::check!(
                ventry.replace(k, replacement),
                "failed to replace type in vtable"
            )?;
        }
        for member in &mut self.members {
            changed |= cu::check!(
                member.replace(k, replacement),
                "failed to replace type in struct member"
            )?;
        }
        for targ in &mut self.template_args {
            changed |= cu::check!(
                targ.replace(k, replacement),
                "failed to replace type in struct template args"
            )?;
        }
        Ok(changed)
    }
}

impl Member {
    pub fn replace(&mut self, k: Goff, replacement: &Tree<Goff>) -> cu::Result<bool> {
        cu::check!(
            tree_replace(&mut self.ty, k, replacement),
            "failed to replace member type"
        )
    }
}

impl VtableEntry {
    pub fn replace(&mut self, k: Goff, replacement: &Tree<Goff>) -> cu::Result<bool> {
        let mut changed = false;
        for targ in &mut self.function_types {
            changed |= cu::check!(
                tree_replace(targ, k, replacement),
                "failed to replace vtable function type"
            )?;
        }
        Ok(changed)
    }
}

impl SymbolInfo {
    pub fn replace(&mut self, k: Goff, replacement: &Tree<Goff>) -> cu::Result<bool> {
        let mut changed = cu::check!(
            tree_replace(&mut self.ty, k, replacement),
            "failed to replace symbol type"
        )?;
        for targ in &mut self.template_args {
            changed |= cu::check!(
                targ.replace(k, replacement),
                "failed to replace symbol template arg type"
            )?;
        }
        Ok(changed)
    }
}

impl FullQualName {
    pub fn replace(&mut self, k: Goff, replacement: &Tree<Goff>) -> cu::Result<bool> {
        match self {
            FullQualName::Name(n) => n.replace(k, replacement),
            FullQualName::Goff(n) => n.replace(k, replacement),
        }
    }
}

impl NamespacedTemplatedName {
    pub fn replace(&mut self, k: Goff, replacement: &Tree<Goff>) -> cu::Result<bool> {
        let mut changed = self.base.replace(k, replacement)?;
        for targ in &mut self.templates {
            changed |= targ.replace(k, replacement)?;
        }
        Ok(changed)
    }
}
impl NamespacedTemplatedGoffName {
    pub fn replace(&mut self, k: Goff, replacement: &Tree<Goff>) -> cu::Result<bool> {
        let mut changed = self.base.replace(k, replacement)?;
        for targ in &mut self.templates {
            changed |= targ.replace(k, replacement)?;
        }
        Ok(changed)
    }
}
impl NamespacedName {
    pub fn replace(&mut self, k: Goff, replacement: &Tree<Goff>) -> cu::Result<bool> {
        self.0.replace(k, replacement)
    }
}
impl Namespace {
    pub fn replace(&mut self, k: Goff, replacement: &Tree<Goff>) -> cu::Result<bool> {
        let mut changed = false;
        for seg in &mut self.0 {
            changed |= seg.replace(k, replacement)?;
        }
        Ok(changed)
    }
}
impl NameSeg {
    pub fn replace(&mut self, k: Goff, replacement: &Tree<Goff>) -> cu::Result<bool> {
        match self {
            NameSeg::Name(_) => Ok(false),
            NameSeg::Type(goff, _) | NameSeg::Subprogram(goff, _, _) => {
                if *goff != k {
                    return Ok(false);
                }
                let Tree::Base(replacement) = replacement else {
                    cu::bail!(
                        "cannot replace {k} with {replacement:?} because it is used in a fullqual name"
                    );
                };
                *goff = *replacement;
                Ok(true)
            }
            NameSeg::Anonymous => Ok(false),
        }
    }
}

impl TemplateArg<Goff> {
    pub fn replace(&mut self, k: Goff, replacement: &Tree<Goff>) -> cu::Result<bool> {
        let TemplateArg::Type(tree) = self else {
            return Ok(false);
        };
        cu::check!(
            tree_replace(tree, k, replacement),
            "failed to replace template arg type"
        )
    }
}

impl TemplateArg<NamespacedTemplatedName> {
    pub fn replace(&mut self, k: Goff, replacement: &Tree<Goff>) -> cu::Result<bool> {
        let TemplateArg::Type(tree) = self else {
            return Ok(false);
        };
        cu::check!(
            tree_replace_name(tree, k, replacement),
            "failed to replace template arg NamespacedTemplatedName"
        )
    }
}

fn tree_replace(tree: &mut Tree<Goff>, k: Goff, replacement: &Tree<Goff>) -> cu::Result<bool> {
    let result = tree.to_replaced(|x| {
        if x == &k {
            Some(replacement.clone())
        } else {
            None
        }
    });
    let result = cu::check!(result, "tree_replace failed")?;
    match result {
        None => {
            // no replacements
            Ok(false)
        }
        Some(new) => {
            *tree = new;
            Ok(true)
        }
    }
}

fn tree_replace_name(
    tree: &mut Tree<NamespacedTemplatedName>,
    k: Goff,
    replacement: &Tree<Goff>,
) -> cu::Result<bool> {
    let result = tree.to_replaced(|x| {
        let mut replaced = x.clone();
        if let Ok(true) = replaced.replace(k, replacement) {
            Some(Tree::Base(replaced))
        } else {
            None
        }
    });
    let result = cu::check!(result, "tree_replace_name failed")?;
    match result {
        None => {
            // no replacements
            Ok(false)
        }
        Some(new) => {
            *tree = new;
            Ok(true)
        }
    }
}
