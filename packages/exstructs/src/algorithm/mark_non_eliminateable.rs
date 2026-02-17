//! Mark Goffs that cannot be eliminated
//!
//! - A PTM base type must be a struct/union, so it cannot be eliminated
//! - A struct/class with vtable cannot be eliminated
//! - A type that directly references itself cannot be eliminated

use crate::{Goff, GoffSet, MType, Member, Struct, SymbolInfo, TemplateArg, Union, VtableEntry};

impl MType {
    pub fn mark_non_eliminateable(&self, self_goff: Goff, marked: &mut GoffSet) {
        match self {
            Self::Prim(_) => {}
            Self::Enum(_) => {}
            Self::Union(data) => {
                data.data.mark_non_eliminateable(self_goff, marked);
            }
            Self::Struct(data) => {
                data.data.mark_non_eliminateable(self_goff, marked);
            }
            // declarations can always be eliminated
            Self::EnumDecl(_) => {}
            Self::UnionDecl(_) => {}
            Self::StructDecl(_) => {}
        }
    }
}

impl Union {
    pub fn mark_non_eliminateable(&self, self_goff: Goff, marked: &mut GoffSet) {
        // self-referential types cannot be eliminated
        if self.contains_goff(self_goff) {
            marked.insert(self_goff);
        }
        for targ in &self.template_args {
            targ.mark_non_eliminateable(marked);
        }
        for member in &self.members {
            member.mark_non_eliminateable(marked);
        }
    }
}

impl Struct {
    pub fn mark_non_eliminateable(&self, self_goff: Goff, marked: &mut GoffSet) {
        // self-referential types cannot be eliminated
        if self.contains_goff(self_goff) {
            marked.insert(self_goff);
        }
        // types with vtable cannot be eliminated
        if !self.vtable.is_empty() {
            marked.insert(self_goff);
        }
        for targ in &self.template_args {
            targ.mark_non_eliminateable(marked);
        }
        for (_, ventry) in &self.vtable {
            ventry.mark_non_eliminateable(marked);
        }
        for member in &self.members {
            member.mark_non_eliminateable(marked);
        }
    }
}

impl Member {
    pub fn mark_non_eliminateable(&self, marked: &mut GoffSet) {
        self.ty.for_each_ptm_base(|x| {
            marked.insert(*x);
        });
    }
}

impl VtableEntry {
    pub fn mark_non_eliminateable(&self, marked: &mut GoffSet) {
        for targ in &self.function_types {
            targ.for_each_ptm_base(|x| {
                marked.insert(*x);
            });
        }
    }
}

impl SymbolInfo {
    pub fn mark_non_eliminateable(&self, marked: &mut GoffSet) {
        self.ty.for_each_ptm_base(|x| {
            marked.insert(*x);
        });
        for targ in &self.template_args {
            targ.mark_non_eliminateable(marked);
        }
    }
}

impl TemplateArg<Goff> {
    pub fn mark_non_eliminateable(&self, marked: &mut GoffSet) {
        let TemplateArg::Type(tree) = self else {
            return;
        };
        tree.for_each_ptm_base(|x| {
            marked.insert(*x);
        });
    }
}
