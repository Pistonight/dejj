use std::collections::BTreeSet;

use cu::pre::*;

use crate::{MType, MTypeData, MTypeDecl, NamespacedName, NamespacedTemplatedName, Struct};

impl MType {
    /// Create a merged type data
    pub fn merge_data(&self, other: &Self) -> cu::Result<Self> {
        fn select_name(
            a: &Option<NamespacedName>,
            b: &Option<NamespacedName>,
        ) -> Option<NamespacedName> {
            a.as_ref().or_else(|| b.as_ref()).cloned()
        }
        match (self, other) {
            (MType::Prim(a), MType::Prim(b)) => {
                cu::ensure!(a == b)?;
                Ok(MType::Prim(*a))
            }
            // prefer primitive types
            (MType::Prim(a), _) | (_, MType::Prim(a)) => Ok(MType::Prim(*a)),
            (MType::Enum(a), MType::Enum(b)) => {
                cu::ensure!(
                    a.data == b.data,
                    "cannot merge 2 enums of different enumerators or sizes"
                )?;
                let mut decl_names = BTreeSet::new();
                decl_names.extend(a.decl_names.clone());
                decl_names.extend(b.decl_names.clone());
                let name = select_name(&a.name, &b.name);
                let data = MTypeData {
                    name,
                    data: a.data.clone(),
                    decl_names: decl_names.into_iter().collect(),
                };
                Ok(MType::Enum(data))
            }
            (MType::Enum(a), MType::EnumDecl(b)) | (MType::EnumDecl(b), MType::Enum(a)) => {
                Ok(MType::Enum(a.merge_with_decl(b)))
            }
            (MType::EnumDecl(a), MType::EnumDecl(b)) => Ok(MType::EnumDecl(a.merge_with_decl(b))),
            // prefer enums over struct or union
            (MType::Enum(data), _) | (_, MType::Enum(data)) => Ok(MType::Enum(data.clone())),
            // enum decl should not be merged with other
            (MType::EnumDecl(_), _) | (_, MType::EnumDecl(_)) => {
                cu::bail!("enum declaration cannot be merged with non-enum");
            }
            (MType::Struct(a), MType::Struct(b)) => {
                let mut decl_names = BTreeSet::new();
                decl_names.extend(a.decl_names.clone());
                decl_names.extend(b.decl_names.clone());
                let data = cu::check!(
                    a.data.merge_data(&b.data),
                    "failed to get merged struct data"
                )?;
                let name = select_name(&a.name, &b.name);
                let data = MTypeData {
                    name,
                    data,
                    decl_names: decl_names.into_iter().collect(),
                };
                Ok(MType::Struct(data))
            }
            (MType::Struct(a), MType::StructDecl(b)) | (MType::StructDecl(b), MType::Struct(a)) => {
                Ok(MType::Struct(a.merge_with_decl(b)))
            }
            (MType::StructDecl(a), MType::StructDecl(b)) => {
                Ok(MType::StructDecl(a.merge_with_decl(b)))
            }
            // prefer struct over union
            (MType::Struct(data), _) | (_, MType::Struct(data)) => Ok(MType::Struct(data.clone())),
            // struct decl should not be merged with other
            (MType::StructDecl(_), _) | (_, MType::StructDecl(_)) => {
                cu::bail!("struct declaration cannot be merged with union");
            }
            (MType::Union(a), MType::Union(b)) => {
                let mut decl_names = BTreeSet::new();
                decl_names.extend(a.decl_names.clone());
                decl_names.extend(b.decl_names.clone());
                let name = select_name(&a.name, &b.name);
                let data = MTypeData {
                    name,
                    data: a.data.clone(),
                    decl_names: decl_names.into_iter().collect(),
                };
                Ok(MType::Union(data))
            }
            (MType::Union(a), MType::UnionDecl(b)) | (MType::UnionDecl(b), MType::Union(a)) => {
                Ok(MType::Union(a.merge_with_decl(b)))
            }
            (MType::UnionDecl(a), MType::UnionDecl(b)) => {
                Ok(MType::UnionDecl(a.merge_with_decl(b)))
            }
        }
    }
}
impl<T: Clone> MTypeData<T> {
    pub fn merge_with_decl(&self, other: &MTypeDecl) -> Self {
        let mut decl_names = BTreeSet::new();
        decl_names.extend(self.decl_names.clone());
        decl_names.extend(other.typedef_names.clone());
        decl_names.insert(other.name.clone());
        Self {
            name: self.name.clone(),
            data: self.data.clone(),
            decl_names: decl_names.into_iter().collect(),
        }
    }
}

impl MTypeDecl {
    pub fn merge_with_decl(&self, other: &Self) -> Self {
        let (name, name2) = Self::select_decl_name(&self.name, &other.name);
        let mut typedef_names = BTreeSet::new();
        typedef_names.extend(self.typedef_names.clone());
        typedef_names.extend(other.typedef_names.clone());
        typedef_names.insert(name2);
        typedef_names.remove(&name);
        Self {
            name,
            typedef_names: typedef_names.into_iter().collect(),
        }
    }
    fn select_decl_name(
        a: &NamespacedTemplatedName,
        b: &NamespacedTemplatedName,
    ) -> (NamespacedTemplatedName, NamespacedTemplatedName) {
        let min = a.min(b).clone();
        if a == &min {
            (min, b.clone())
        } else {
            (min, a.clone())
        }
    }
}

impl Struct {
    /// Create a merged type data
    pub fn merge_data(&self, other: &Self) -> cu::Result<Self> {
        // merge vtables
        let mut new_vtable = self.vtable.clone();
        for (i, other_entry) in &other.vtable {
            if other_entry.is_dtor() {
                if let Some((_, self_entry)) = self.vtable.iter().find(|(_, e)| e.is_dtor()) {
                    cu::ensure!(
                        other_entry.name == self_entry.name,
                        "cannot merge vtable dtor entries of different names: {:?} and {:?}",
                        other_entry.name,
                        self_entry.name
                    )?;
                } else {
                    new_vtable.push((*i, other_entry.clone()));
                }
                continue;
            }
            if let Some((_, self_entry)) =
                self.vtable.iter().find(|(j, se)| !se.is_dtor() && i == j)
            {
                cu::ensure!(
                    other_entry.name == self_entry.name,
                    "cannot merge vtable entries of different names, at index {i}: {:?} and {:?}",
                    other_entry.name,
                    self_entry.name
                )?;
            } else {
                new_vtable.push((*i, other_entry.clone()));
            }
        }
        new_vtable.sort_by_key(|x| x.0);
        Ok(Self {
            template_args: self.template_args.clone(),
            byte_size: self.byte_size,
            vtable: new_vtable,
            members: self.members.clone(),
        })
    }
}
