//! Maps goffs in a data structure through a mapping function

use cu::pre::*;

use crate::{
    Enum, EnumUndeterminedSize, Goff, GoffMapFn, LType, LTypeData, LTypeDecl, MType, MTypeData,
    MTypeDecl, Member, NameSeg, Namespace, NamespacedName, NamespacedTemplatedName, Struct,
    SymbolInfo, TemplateArg, Union, VtableEntry,
};

pub trait MapGoff {
    fn map_goff(&mut self, f: &GoffMapFn) -> cu::Result<()>;
}

impl MType {
    pub fn map_goff<F: Fn(Goff) -> cu::Result<Goff>>(&mut self, f: F) -> cu::Result<()> {
        let f: GoffMapFn = Box::new(f);
        match self {
            Self::Prim(_) => {}
            Self::Enum(data) => {
                cu::check!(data.map_goff(&f), "failed to map MTYPE enum")?;
            }
            Self::EnumDecl(decl) => {
                cu::check!(decl.map_goff(&f), "failed to map MTYPE enum decl")?;
            }
            Self::Union(data) => {
                cu::check!(data.map_goff(&f), "failed to map MTYPE union")?;
            }
            Self::UnionDecl(decl) => {
                cu::check!(decl.map_goff(&f), "failed to map MTYPE union decl")?;
            }
            Self::Struct(data) => {
                cu::check!(data.map_goff(&f), "failed to map MTYPE struct")?;
            }
            Self::StructDecl(decl) => {
                cu::check!(decl.map_goff(&f), "failed to map MTYPE struct decl")?;
            }
        }
        Ok(())
    }
}

impl LType {
    pub fn map_goff<F: Fn(Goff) -> cu::Result<Goff>>(&mut self, f: F) -> cu::Result<()> {
        let f: GoffMapFn = Box::new(f);
        match self {
            Self::Prim(_) => {}
            Self::Typedef { name, target } => {
                cu::check!(name.map_goff(&f), "failed to map name for typedef")?;
                *target = cu::check!(f(*target), "failed to map typedef -> {target}")?;
            }
            Self::Alias(inner) => {
                *inner = cu::check!(f(*inner), "failed to map alias -> {inner}")?;
            }
            Self::Enum(data) => {
                cu::check!(data.map_goff(&f), "failed to map LTYPE enum")?;
            }
            Self::EnumDecl(decl) => {
                cu::check!(decl.map_goff(&f), "failed to map LTYPE enum decl")?;
            }
            Self::Union(data) => {
                cu::check!(data.map_goff(&f), "failed to map LTYPE union")?;
            }
            Self::UnionDecl(decl) => {
                cu::check!(decl.map_goff(&f), "failed to map LTYPE union decl")?;
            }
            Self::Struct(data) => {
                cu::check!(data.map_goff(&f), "failed to map LTYPE struct")?;
            }
            Self::StructDecl(decl) => {
                cu::check!(decl.map_goff(&f), "failed to map LTYPE struct decl")?;
            }
            Self::Tree(tree) => {
                cu::check!(
                    tree.for_each_mut(|r| {
                        *r = f(*r)?;
                        cu::Ok(())
                    }),
                    "failed to map LTYPE tree"
                )?;
            }
        }
        Ok(())
    }
}

impl<T: MapGoff> MTypeData<T> {
    pub fn map_goff(&mut self, f: &GoffMapFn) -> cu::Result<()> {
        if let Some(name) = &mut self.name {
            cu::check!(name.map_goff(&f), "failed to map name for MTYPE data")?;
        }
        for n in &mut self.decl_names {
            cu::check!(n.map_goff(&f), "failed to map decl name for MTYPE data")?;
        }
        cu::check!(self.data.map_goff(&f), "failed to map data for MTYPE")?;
        Ok(())
    }
}

impl MTypeDecl {
    pub fn map_goff(&mut self, f: &GoffMapFn) -> cu::Result<()> {
        cu::check!(self.name.map_goff(&f), "failed to map goff for MTYPE name")?;
        for n in &mut self.typedef_names {
            cu::check!(n.map_goff(&f), "failed to map typedef name for MTYPE")?;
        }
        Ok(())
    }
}

impl<T: MapGoff> LTypeData<T> {
    pub fn map_goff(&mut self, f: &GoffMapFn) -> cu::Result<()> {
        if let Some(name) = &mut self.name {
            cu::check!(name.map_goff(&f), "failed to map name for LTYPE data")?;
        }
        cu::check!(self.data.map_goff(&f), "failed to map data for LTYPE")?;
        Ok(())
    }
}

impl LTypeDecl {
    pub fn map_goff(&mut self, f: &GoffMapFn) -> cu::Result<()> {
        cu::check!(
            self.enclosing.map_goff(&f),
            "failed to map namespace for LTYPE decl"
        )?;
        cu::check!(
            self.name_with_tpl.map_goff(&f),
            "failed to map name for LTYPE decl"
        )?;
        Ok(())
    }
}

impl MapGoff for Enum {
    fn map_goff(&mut self, _: &GoffMapFn) -> cu::Result<()> {
        Ok(())
    }
}

impl MapGoff for EnumUndeterminedSize {
    fn map_goff(&mut self, _: &GoffMapFn) -> cu::Result<()> {
        Ok(())
    }
}

impl MapGoff for Union {
    fn map_goff(&mut self, f: &GoffMapFn) -> cu::Result<()> {
        for targ in &mut self.template_args {
            cu::check!(targ.map_goff(f), "failed to map union template args")?;
        }
        for member in &mut self.members {
            cu::check!(member.map_goff(f), "failed to map union members")?;
        }
        Ok(())
    }
}

impl MapGoff for Struct {
    fn map_goff(&mut self, f: &GoffMapFn) -> cu::Result<()> {
        for targ in &mut self.template_args {
            cu::check!(targ.map_goff(f), "failed to map struct template args")?;
        }
        for (_, ventry) in &mut self.vtable {
            cu::check!(ventry.map_goff(f), "failed to map struct vtable entries")?;
        }
        for member in &mut self.members {
            cu::check!(member.map_goff(f), "failed to map struct members")?;
        }
        Ok(())
    }
}

impl Member {
    pub fn map_goff(&mut self, f: &GoffMapFn) -> cu::Result<()> {
        cu::check!(
            self.ty.for_each_mut(|r| {
                *r = f(*r)?;
                cu::Ok(())
            }),
            "failed to map member type"
        )?;
        Ok(())
    }
}

impl VtableEntry {
    pub fn map_goff<F: Fn(Goff) -> cu::Result<Goff>>(&mut self, f: F) -> cu::Result<()> {
        for x in &mut self.function_types {
            cu::check!(
                x.for_each_mut(|r| {
                    *r = f(*r)?;
                    cu::Ok(())
                }),
                "failed to map vtable entry"
            )?;
        }
        Ok(())
    }
}

impl SymbolInfo {
    /// Run goff conversion on nested type data
    pub fn map_goff(&mut self, f: &GoffMapFn) -> cu::Result<()> {
        cu::check!(
            self.ty.for_each_mut(|r| {
                *r = f(*r)?;
                cu::Ok(())
            }),
            "failed to map symbol type"
        )?;

        for targ in &mut self.template_args {
            cu::check!(targ.map_goff(&f), "failed to map symbol template args")?;
        }

        Ok(())
    }
}

impl NamespacedTemplatedName {
    pub fn map_goff(&mut self, f: &GoffMapFn) -> cu::Result<()> {
        self.base.map_goff(f)?;
        for targ in &mut self.templates {
            targ.map_goff(&f)?;
        }
        Ok(())
    }
}

impl TemplateArg<Goff> {
    pub fn map_goff(&mut self, f: &GoffMapFn) -> cu::Result<()> {
        let Self::Type(tree) = self else {
            return Ok(());
        };
        cu::check!(
            tree.for_each_mut(|r| {
                *r = f(*r)?;
                cu::Ok(())
            }),
            "failed to map template arg type"
        )?;
        Ok(())
    }
}

impl TemplateArg<NamespacedTemplatedName> {
    /// Run goff conversion on nested type data
    pub fn map_goff(&mut self, f: &GoffMapFn) -> cu::Result<()> {
        let Self::Type(tree) = self else {
            return Ok(());
        };
        cu::check!(
            tree.for_each_mut(|r| { r.map_goff(f) }),
            "failed to map template arg name"
        )?;
        Ok(())
    }
}

impl NamespacedName {
    pub fn map_goff(&mut self, f: &GoffMapFn) -> cu::Result<()> {
        cu::check!(self.0.map_goff(f), "failed to map namespaced name")
    }
}

impl Namespace {
    pub fn map_goff(&mut self, f: &GoffMapFn) -> cu::Result<()> {
        for seg in &mut self.0 {
            seg.map_goff(f)?;
        }
        Ok(())
    }
}

impl NameSeg {
    pub fn map_goff(&mut self, f: &GoffMapFn) -> cu::Result<()> {
        match self {
            Self::Type(goff, _) => {
                *goff = cu::check!(f(*goff), "failed to map type in namespace")?;
            }
            Self::Subprogram(goff, _, _) => {
                *goff = cu::check!(f(*goff), "failed to map subprogram in namespace")?;
            }
            _ => {}
        }
        Ok(())
    }
}
