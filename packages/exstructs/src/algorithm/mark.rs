//! Mark referenced types for sweeping

use crate::{
    Enum, EnumUndeterminedSize, FullQualName, Goff, GoffSet, HType, HTypeData, LType, LTypeData,
    LTypeDecl, MType, MTypeData, MTypeDecl, NameSeg, Namespace, NamespacedName,
    NamespacedTemplatedGoffName, NamespacedTemplatedName, Struct, SymbolInfo, TemplateArg, Union,
};

pub trait Mark {
    /// Mark referenced types (for GC, connected components calculation, ...)
    fn mark(&self, marked: &mut GoffSet);
}

impl HType {
    /// Mark referenced types
    pub fn mark(&self, _: Goff, marked: &mut GoffSet) {
        match self {
            Self::Prim(prim) => {
                marked.insert(Goff::prim(*prim));
            }
            Self::Enum(data) => data.mark(marked),
            Self::Union(data) => data.mark(marked),
            Self::Struct(data) => data.mark(marked),
        }
        // we never mark self at HStage, since if a type is never referenced
        // in a symbol, we should get rid of it
    }
}

impl MType {
    /// Mark referenced types
    ///
    /// self_goff is the goff of the MType being processed.
    /// global Prim and Decl MTypes are not considered strong refs
    pub fn mark(&self, self_goff: Goff, marked: &mut GoffSet) {
        match self {
            Self::Prim(prim) => {
                marked.insert(Goff::prim(*prim));
                return; // don't mark self
            }
            Self::EnumDecl(data) | Self::UnionDecl(data) | Self::StructDecl(data) => {
                data.mark(marked);
                return; // don't mark self
            }
            Self::Enum(data) => data.mark(marked),
            Self::Union(data) => data.mark(marked),
            Self::Struct(data) => data.mark(marked),
        }
        marked.insert(self_goff);
    }
}

impl LType {
    /// Mark referenced types for GC
    ///
    /// self_goff is the goff of the LType being processed.
    /// global Prim and Tree LTypes are not considered strong refs
    pub fn mark(&self, self_goff: Goff, marked: &mut GoffSet) {
        match self {
            Self::Prim(prim) => {
                marked.insert(Goff::prim(*prim));
                return; // don't mark self
            }
            Self::Tree(tree) => {
                let _: Result<_, _> = tree.for_each(|goff| {
                    marked.insert(*goff);
                    Ok(())
                });
                // don't mark self
                return;
            }
            Self::Typedef { name, target } => {
                name.mark(marked);
                marked.insert(*target);
            }
            Self::Enum(data) => data.mark(marked),
            Self::EnumDecl(decl) => decl.mark(marked),
            Self::Union(data) => data.mark(marked),
            Self::UnionDecl(decl) => decl.mark(marked),
            Self::Struct(data) => data.mark(marked),
            Self::StructDecl(decl) => decl.mark(marked),
            Self::Alias(goff) => {
                marked.insert(*goff);
            }
        }
        marked.insert(self_goff);
    }
}

impl<T: Mark> HTypeData<T> {
    pub fn mark(&self, marked: &mut GoffSet) {
        for n in &self.fqnames {
            n.mark(marked);
        }
        self.data.mark(marked);
    }
}

impl<T: Mark> MTypeData<T> {
    pub fn mark(&self, marked: &mut GoffSet) {
        if let Some(name) = &self.name {
            name.mark(marked);
        }
        for n in &self.decl_names {
            n.mark(marked);
        }
        self.data.mark(marked);
    }
}

impl MTypeDecl {
    pub fn mark(&self, marked: &mut GoffSet) {
        self.name.mark(marked);
        for n in &self.typedef_names {
            n.mark(marked);
        }
    }
}

impl<T: Mark> LTypeData<T> {
    pub fn mark(&self, marked: &mut GoffSet) {
        if let Some(name) = &self.name {
            name.mark(marked);
        }
        self.data.mark(marked);
    }
}

impl LTypeDecl {
    pub fn mark(&self, marked: &mut GoffSet) {
        self.enclosing.mark(marked);
        self.name_with_tpl.mark(marked);
    }
}

impl Mark for Enum {
    fn mark(&self, _: &mut GoffSet) {}
}

impl Mark for EnumUndeterminedSize {
    fn mark(&self, marked: &mut GoffSet) {
        if let Err(e) = self.byte_size_or_base {
            marked.insert(e);
        }
    }
}

impl Mark for Union {
    fn mark(&self, marked: &mut GoffSet) {
        for targ in &self.template_args {
            targ.mark(marked);
        }
        for member in &self.members {
            let _: Result<_, _> = member.ty.for_each(|goff| {
                marked.insert(*goff);
                Ok(())
            });
        }
    }
}

impl Mark for Struct {
    fn mark(&self, marked: &mut GoffSet) {
        for targ in &self.template_args {
            let TemplateArg::Type(tree) = targ else {
                continue;
            };
            let _: Result<_, _> = tree.for_each(|goff| {
                marked.insert(*goff);
                Ok(())
            });
        }
        for (_, ventry) in &self.vtable {
            for t in &ventry.function_types {
                let _: Result<_, _> = t.for_each(|goff| {
                    marked.insert(*goff);
                    Ok(())
                });
            }
        }
        for member in &self.members {
            let _: Result<_, _> = member.ty.for_each(|goff| {
                marked.insert(*goff);
                Ok(())
            });
        }
    }
}

impl SymbolInfo {
    pub fn mark(&self, marked: &mut GoffSet) {
        let _: Result<_, _> = self.ty.for_each(|goff| {
            marked.insert(*goff);
            Ok(())
        });
        for targ in &self.template_args {
            targ.mark(marked);
        }
    }
}

impl FullQualName {
    pub fn mark(&self, marked: &mut GoffSet) {
        match self {
            FullQualName::Name(n) => n.mark(marked),
            FullQualName::Goff(n) => n.mark(marked),
        }
    }
}

impl NamespacedTemplatedName {
    pub fn mark(&self, marked: &mut GoffSet) {
        self.base.mark(marked);
        for t in &self.templates {
            t.mark(marked);
        }
    }
}

impl NamespacedTemplatedGoffName {
    pub fn mark(&self, marked: &mut GoffSet) {
        self.base.mark(marked);
        for t in &self.templates {
            t.mark(marked);
        }
    }
}

impl TemplateArg<Goff> {
    pub fn mark(&self, marked: &mut GoffSet) {
        let TemplateArg::Type(tree) = self else {
            return;
        };
        let _: Result<_, _> = tree.for_each(|goff| {
            marked.insert(*goff);
            Ok(())
        });
    }
}

impl TemplateArg<NamespacedTemplatedName> {
    pub fn mark(&self, marked: &mut GoffSet) {
        let TemplateArg::Type(tree) = self else {
            return;
        };
        let _: Result<_, _> = tree.for_each(|name| {
            name.mark(marked);
            Ok(())
        });
    }
}

impl NamespacedName {
    pub fn mark(&self, marked: &mut GoffSet) {
        self.0.mark(marked);
    }
}

impl Namespace {
    pub fn mark(&self, marked: &mut GoffSet) {
        for seg in &self.0 {
            seg.mark(marked);
        }
    }
    pub fn mark_all(&self, marked: &mut GoffSet) {
        for seg in &self.0 {
            seg.mark(marked);
        }
    }
}

impl NameSeg {
    /// Mark referenced types for GC
    pub fn mark(&self, marked: &mut GoffSet) {
        if let NameSeg::Type(goff, _) = self {
            marked.insert(*goff);
        }
        // note we don't mark subprogram here
    }
    pub fn mark_all(&self, marked: &mut GoffSet) {
        match self {
            NameSeg::Type(goff, _) => {
                marked.insert(*goff);
            }
            NameSeg::Subprogram(goff, _, _) => {
                marked.insert(*goff);
            }
            NameSeg::Name(_) => {}
            NameSeg::Anonymous => {}
        }
    }
}
