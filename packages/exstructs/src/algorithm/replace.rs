//! Replace a goff with a Tree of goffs

use cu::pre::*;
use tyyaml::Tree;

use crate::{Goff, HType, MType, Member, Struct, SymbolInfo, TemplateArg, Union, VtableEntry};

impl HType {
    pub fn replace(&mut self, k: Goff, replacement: &Tree<Goff>) -> cu::Result<bool> {
        match self {
            Self::Prim(_) => {}
            Self::Enum(_) => {}
            Self::Union(data) => return data.data.replace(k, replacement),
            Self::Struct(data) => return data.data.replace(k, replacement),
        }
        Ok(false)
    }
}

impl MType {
    pub fn replace(&mut self, k: Goff, replacement: &Tree<Goff>) -> cu::Result<bool> {
        match self {
            Self::Prim(_) => {}
            Self::Enum(_) => {}
            Self::Union(data) => return data.data.replace(k, replacement),
            Self::Struct(data) => return data.data.replace(k, replacement),
            Self::EnumDecl(_) => {}
            Self::UnionDecl(_) => {}
            Self::StructDecl(_) => {}
        }
        Ok(false)
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
