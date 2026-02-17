//! Add merge dependencies if self and other are compatible for merging, return an error if not compatible

use tyyaml::Tree;

use cu::pre::*;

use crate::algorithm::merge::MergeTask;
use crate::{Goff, MType, Member, Struct, TemplateArg, Union, VtableEntry};

impl MType {
    pub fn add_merge_deps(&self, other: &Self, task: &mut MergeTask) -> cu::Result<()> {
        match (self, other) {
            (MType::Prim(a), MType::Prim(b)) => {
                cu::ensure!(a == b)?;
            }
            (MType::Enum(a), MType::Enum(b)) => {
                cu::ensure!(
                    a.data == b.data,
                    "cannot merge 2 enums of different enumerators or sizes"
                )?;
            }
            (MType::Enum(_), MType::EnumDecl(_)) => {}
            (MType::EnumDecl(_), MType::Enum(_)) => {}
            (MType::EnumDecl(_), MType::EnumDecl(_)) => {}

            (MType::Union(a), MType::Union(b)) => {
                a.data.add_merge_deps(&b.data, task)?;
            }

            (MType::Union(_), MType::UnionDecl(_)) => {}
            (MType::UnionDecl(_), MType::Union(_)) => {}
            (MType::UnionDecl(_), MType::UnionDecl(_)) => {}

            (MType::Struct(a), MType::Struct(b)) => {
                a.data.add_merge_deps(&b.data, task)?;
            }
            (MType::Struct(_), MType::StructDecl(_)) => {}
            (MType::StructDecl(_), MType::Struct(_)) => {}
            (MType::StructDecl(_), MType::StructDecl(_)) => {}

            _ => {
                cu::bail!("cannot merge 2 different types");
            }
        }

        Ok(())
    }
}

impl Union {
    pub fn add_merge_deps(&self, other: &Self, task: &mut MergeTask) -> cu::Result<()> {
        cu::ensure!(
            self.byte_size == other.byte_size,
            "unions of different sizes cannot be merged"
        )?;
        cu::ensure!(
            self.template_args.len() == other.template_args.len(),
            "unions of different template arg count cannot be merged"
        )?;
        for (a, b) in std::iter::zip(&self.template_args, &other.template_args) {
            cu::check!(
                a.add_merge_deps(b, task),
                "add_merge_deps failed for union template args"
            )?;
        }
        cu::ensure!(
            self.members.len() == other.members.len(),
            "unions of different member count cannot be merged"
        )?;
        for (a, b) in std::iter::zip(&self.members, &other.members) {
            cu::check!(
                a.add_merge_deps(b, task),
                "add_merge_deps failed for union members"
            )?;
        }
        Ok(())
    }
}

impl Struct {
    pub fn add_merge_deps(&self, other: &Self, task: &mut MergeTask) -> cu::Result<()> {
        cu::ensure!(
            self.byte_size == other.byte_size,
            "structs of different sizes cannot be merged (0x{:x} != 0x{:x})",
            self.byte_size,
            other.byte_size
        )?;
        cu::ensure!(
            self.template_args.len() == other.template_args.len(),
            "structs of different template arg count cannot be merged"
        )?;
        for (a, b) in std::iter::zip(&self.template_args, &other.template_args) {
            cu::check!(
                a.add_merge_deps(b, task),
                "add_merge_deps failed for struct template args"
            )?;
        }
        // for vtable, we might not have the full vtable until all CUs are merged,
        // so we can only check if there are any existing conflicts
        for (i, entry) in &self.vtable {
            if entry.is_dtor() {
                if let Some((_, other_entry)) = other.vtable.iter().find(|(_, e)| e.is_dtor()) {
                    cu::check!(
                        entry.add_merge_deps(other_entry, task),
                        "add_merge_deps failed for vtable dtor entry, {entry:#?}"
                    )?;
                }
                continue;
            }
            if let Some((_, other_entry)) =
                other.vtable.iter().find(|(x, oe)| !oe.is_dtor() && x == i)
            {
                cu::check!(
                    entry.add_merge_deps(other_entry, task),
                    "add_merge_deps failed for vtable entry i={i}, a={entry:#?}, b={other_entry:#?}"
                )?;
            }
        }

        cu::ensure!(
            self.members.len() == other.members.len(),
            "structs of different member count cannot be merged"
        )?;
        for (a, b) in std::iter::zip(&self.members, &other.members) {
            cu::check!(
                a.add_merge_deps(b, task),
                "add_merge_deps failed for struct members"
            )?;
        }

        Ok(())
    }
}

impl Member {
    pub fn add_merge_deps(&self, other: &Self, task: &mut MergeTask) -> cu::Result<()> {
        cu::ensure!(
            self.offset == other.offset,
            "members of different offsets cannot be merged"
        )?;
        cu::ensure!(
            self.name == other.name,
            "members of different names cannot be merged"
        )?;
        cu::ensure!(
            self.special == other.special,
            "members of different special types cannot be merged"
        )?;
        cu::check!(
            tree_add_merge_deps(&self.ty, &other.ty, task),
            "add_merge_deps failed for member"
        )
    }
}

impl VtableEntry {
    pub fn add_merge_deps(&self, other: &Self, task: &mut MergeTask) -> cu::Result<()> {
        cu::ensure!(
            self.name == other.name,
            "vtable entries of different names cannot be merged"
        )?;
        cu::ensure!(
            self.function_types.len() == other.function_types.len(),
            "vtable entries of different type lengths cannot be merged"
        )?;
        for (a, b) in std::iter::zip(&self.function_types, &other.function_types) {
            cu::check!(
                tree_add_merge_deps(a, b, task),
                "add_merge_deps failed for vtable types"
            )?;
        }
        Ok(())
    }
}

impl TemplateArg<Goff> {
    pub fn add_merge_deps(&self, other: &Self, task: &mut MergeTask) -> cu::Result<()> {
        match (self, other) {
            (TemplateArg::Const(a), TemplateArg::Const(b)) => {
                cu::ensure!(
                    a == b,
                    "value template arg of different value cannot be merged"
                )?;
            }
            (TemplateArg::Type(a), TemplateArg::Type(b)) => {
                tree_add_merge_deps(a, b, task)?;
            }
            (TemplateArg::StaticConst, TemplateArg::StaticConst) => {}
            _ => {
                cu::bail!("different template args cannot be merged");
            }
        }
        Ok(())
    }
}

/// Add dependencies for merging A and B as type trees
fn tree_add_merge_deps(a: &Tree<Goff>, b: &Tree<Goff>, task: &mut MergeTask) -> cu::Result<()> {
    match (a, b) {
        (Tree::Base(a), Tree::Base(b)) => task.add_dep(*a, *b),
        (Tree::Array(a, len_a), Tree::Array(b, len_b)) => {
            cu::ensure!(
                len_a == len_b,
                "array types of different length cannot be merged"
            )?;
            cu::check!(
                tree_add_merge_deps(a, b, task),
                "add_merge_deps failed for array element type"
            )?;
        }
        (Tree::Ptr(a), Tree::Ptr(b)) => {
            cu::check!(
                tree_add_merge_deps(a, b, task),
                "add_merge_deps failed for pointee type"
            )?;
        }
        (Tree::Sub(args_a), Tree::Sub(args_b)) => {
            cu::ensure!(
                args_a.len() == args_b.len(),
                "subroutine types of different length cannot be merged"
            )?;
            for (a, b) in std::iter::zip(args_a, args_b) {
                cu::check!(
                    tree_add_merge_deps(a, b, task),
                    "add_merge_deps failed for subroutine arg or ret type"
                )?;
            }
        }
        (Tree::Ptmd(base_a, a), Tree::Ptmd(base_b, b)) => {
            task.add_dep(*base_a, *base_b);
            cu::check!(
                tree_add_merge_deps(a, b, task),
                "add_merge_deps failed for ptmd pointee type"
            )?;
        }
        (Tree::Ptmf(base_a, a), Tree::Ptmf(base_b, b)) => {
            cu::ensure!(
                a.len() == b.len(),
                "ptmf subroutine types of different length cannot be merged"
            )?;
            task.add_dep(*base_a, *base_b);
            for (a, b) in std::iter::zip(a, b) {
                cu::check!(
                    tree_add_merge_deps(a, b, task),
                    "add_merge_deps failed for ptmf subroutine arg or ret type"
                )?;
            }
        }
        _ => {
            cu::bail!("different tree shapes cannot be merged")
        }
    }
    Ok(())
}
