use crate::{
    Enum, FullQualName, MType, MTypeData, MTypeDecl, NamespacedName, NamespacedTemplatedGoffName,
    NamespacedTemplatedName, Struct, Union,
};

impl MType {
    /// Get all fully qualified names
    pub fn fullqual_names(&self) -> Vec<FullQualName> {
        match self {
            MType::Prim(prim) => {
                vec![FullQualName::Name(NamespacedTemplatedName::new(
                    NamespacedName::prim(*prim),
                ))]
            }
            MType::Enum(data) => data.fullqual_names(),
            MType::Union(data) => data.fullqual_names(),
            MType::Struct(data) => data.fullqual_names(),

            MType::EnumDecl(decl) | MType::UnionDecl(decl) | MType::StructDecl(decl) => {
                decl.fullqual_names()
            }
        }
    }
}

impl MTypeData<Enum> {
    fn fullqual_names(&self) -> Vec<FullQualName> {
        let mut names = Vec::with_capacity(self.name.is_some() as usize + self.decl_names.len());
        if let Some(name) = &self.name {
            names.push(FullQualName::Goff(NamespacedTemplatedGoffName {
                base: name.clone(),
                templates: vec![],
            }));
        }
        for n in &self.decl_names {
            names.push(FullQualName::Name(n.clone()));
        }
        names
    }
}

impl MTypeData<Union> {
    fn fullqual_names(&self) -> Vec<FullQualName> {
        let mut names = Vec::with_capacity(self.name.is_some() as usize + self.decl_names.len());
        if let Some(name) = &self.name {
            names.push(FullQualName::Goff(NamespacedTemplatedGoffName {
                base: name.clone(),
                templates: self.data.template_args.clone(),
            }));
        }
        for n in &self.decl_names {
            names.push(FullQualName::Name(n.clone()));
        }
        names
    }
}

impl MTypeData<Struct> {
    fn fullqual_names(&self) -> Vec<FullQualName> {
        let mut names = Vec::with_capacity(self.name.is_some() as usize + self.decl_names.len());
        if let Some(name) = &self.name {
            names.push(FullQualName::Goff(NamespacedTemplatedGoffName {
                base: name.clone(),
                templates: self.data.template_args.clone(),
            }));
        }
        for n in &self.decl_names {
            names.push(FullQualName::Name(n.clone()));
        }
        names
    }
}

impl MTypeDecl {
    fn fullqual_names(&self) -> Vec<FullQualName> {
        let mut names = Vec::with_capacity(1 + self.typedef_names.len());
        names.push(FullQualName::Name(self.name.clone()));
        for n in &self.typedef_names {
            names.push(FullQualName::Name(n.clone()));
        }
        names
    }
}
