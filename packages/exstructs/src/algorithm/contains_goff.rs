//! Check if a structure contains (references) a Goff directly

use crate::{Goff, HType, Member, Struct, SymbolInfo, TemplateArg, Union, VtableEntry};

impl HType {
    pub fn contains_goff(&self, k: Goff) -> bool {
        match self {
            Self::Prim(_) => false,
            Self::Enum(_) => false,
            Self::Union(data) => data.data.contains_goff(k),
            Self::Struct(data) => data.data.contains_goff(k),
        }
    }
}

impl Union {
    pub fn contains_goff(&self, k: Goff) -> bool {
        for targ in &self.template_args {
            if targ.contains_goff(k) {
                return true;
            }
        }
        for member in &self.members {
            if member.contains_goff(k) {
                return true;
            }
        }
        false
    }
}

impl Struct {
    pub fn contains_goff(&self, k: Goff) -> bool {
        for (_, ventry) in &self.vtable {
            if ventry.contains_goff(k) {
                // note if vtable is not empty, this is likely true
                return true;
            }
        }
        for targ in &self.template_args {
            if targ.contains_goff(k) {
                return true;
            }
        }
        for member in &self.members {
            if member.contains_goff(k) {
                return true;
            }
        }
        false
    }
}

impl Member {
    pub fn contains_goff(&self, k: Goff) -> bool {
        self.ty.contains(&k)
    }
}

impl VtableEntry {
    pub fn contains_goff(&self, k: Goff) -> bool {
        for targ in &self.function_types {
            if targ.contains(&k) {
                return true;
            }
        }
        false
    }
}

impl SymbolInfo {
    pub fn contains_goff(&self, k: Goff) -> bool {
        if self.ty.contains(&k) {
            return true;
        }
        for targ in &self.template_args {
            if targ.contains_goff(k) {
                return true;
            }
        }
        false
    }
}

impl TemplateArg<Goff> {
    pub fn contains_goff(&self, k: Goff) -> bool {
        let TemplateArg::Type(tree) = self else {
            return false;
        };
        tree.contains(&k)
    }
}
