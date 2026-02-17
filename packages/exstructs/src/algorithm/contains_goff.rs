//! Check if a structure contains (references) a Goff directly

use crate::{Goff, Member, Struct, TemplateArg, Union, VtableEntry};

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

impl TemplateArg<Goff> {
    pub fn contains_goff(&self, k: Goff) -> bool {
        let TemplateArg::Type(tree) = self else {
            return false;
        };
        tree.contains(&k)
    }
}
