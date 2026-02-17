use std::collections::BTreeSet;

use cu::pre::*;
use tyyaml::Tree;

use crate::{
    Enum, Goff, GoffMap, MType, MTypeData, MTypeDecl, NameSeg, Namespace, NamespacedName,
    NamespacedTemplatedName, Struct, TemplateArg, Union,
};

pub fn make_structured_name_permutater(types: &GoffMap<MType>) -> StructuredNamePermutater {
    let mut structured_names: GoffMap<Vec<StructuredName>> = GoffMap::default();
    for (k, t) in types {
        match t {
            MType::Prim(prim) => {
                let e = structured_names.entry(*k).or_default();
                e.push(StructuredName::Name(NamespacedTemplatedName::new(
                    NamespacedName::prim(*prim),
                )));
            }
            MType::Enum(data) => {
                let e = structured_names.entry(*k).or_default();
                data.add_structured_names(e);
            }
            MType::Union(data) => {
                let e = structured_names.entry(*k).or_default();
                data.add_structured_names(e);
            }
            MType::Struct(data) => {
                let e = structured_names.entry(*k).or_default();
                data.add_structured_names(e);
            }
            MType::EnumDecl(decl) | MType::UnionDecl(decl) | MType::StructDecl(decl) => {
                let e = structured_names.entry(*k).or_default();
                decl.add_structured_names(e);
            }
        }
    }
    StructuredNamePermutater::new(structured_names)
}

impl MTypeData<Enum> {
    fn add_structured_names(&self, structured_names: &mut Vec<StructuredName>) {
        if let Some(name) = &self.name {
            structured_names.push(StructuredName::Goff(name.clone(), vec![]));
        }
        for n in &self.decl_names {
            structured_names.push(StructuredName::Name(n.clone()));
        }
    }
}

impl MTypeData<Union> {
    fn add_structured_names(&self, structured_names: &mut Vec<StructuredName>) {
        if let Some(name) = &self.name {
            structured_names.push(StructuredName::Goff(
                name.clone(),
                self.data.template_args.clone(),
            ));
        }
        for n in &self.decl_names {
            structured_names.push(StructuredName::Name(n.clone()));
        }
    }
}

impl MTypeData<Struct> {
    fn add_structured_names(&self, structured_names: &mut Vec<StructuredName>) {
        if let Some(name) = &self.name {
            structured_names.push(StructuredName::Goff(
                name.clone(),
                self.data.template_args.clone(),
            ));
        }
        for n in &self.decl_names {
            structured_names.push(StructuredName::Name(n.clone()));
        }
    }
}

impl MTypeDecl {
    fn add_structured_names(&self, structured_names: &mut Vec<StructuredName>) {
        let name = StructuredName::Name(self.name.clone());
        structured_names.push(name);
        for n in &self.typedef_names {
            structured_names.push(StructuredName::Name(n.clone()));
        }
    }
}

pub struct StructuredNamePermutater {
    names: GoffMap<Vec<StructuredName>>,
    cache: GoffMap<BTreeSet<String>>,
}

impl StructuredNamePermutater {
    pub fn new(names: GoffMap<Vec<StructuredName>>) -> Self {
        Self {
            names,
            cache: Default::default(),
        }
    }
    pub fn structured_names(&self, goff: Goff) -> &[StructuredName] {
        self.names.get(&goff).unwrap()
    }
    pub fn permutated_string_reprs_goff(&mut self, goff: Goff) -> cu::Result<BTreeSet<String>> {
        if let Some(x) = self.cache.get(&goff) {
            return Ok(x.clone());
        }
        let mut output = BTreeSet::new();
        let names = cu::check!(
            self.names.get(&goff),
            "did not resolve structured name for type {goff}"
        )?
        .clone();
        if names.is_empty() {
            return Ok(output);
        }
        // insert empty set into the map, since there can be self-referencing names
        // for example
        // struct Foo {
        // using SelfType = Foo;
        // };
        self.cache.insert(goff, Default::default());
        for n in &names {
            let permutated = n.permutated_string_reprs(self)?;
            output.extend(permutated);
        }
        if output.is_empty() {
            // do not cache and discard this attempt if empty
            self.cache.remove(&goff);
            return Ok(output);
        }
        self.cache.insert(goff, output.clone());

        Ok(output)
    }
}

/// Structured name data that generates all string representations of this name
/// by permutating each segment in the namespaced name
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StructuredName {
    Name(NamespacedTemplatedName),
    Goff(NamespacedName, Vec<TemplateArg<Goff>>),
}

impl StructuredName {
    pub fn permutated_string_reprs(
        &self,
        permutater: &mut StructuredNamePermutater,
    ) -> cu::Result<BTreeSet<String>> {
        match self {
            Self::Name(name) => name.permutated_string_reprs(permutater),
            Self::Goff(base, templates) => {
                let base_names = cu::check!(
                    base.permutated_string_reprs(permutater),
                    "failed to compute base permutations for goff-based base name"
                )?;
                if templates.is_empty() {
                    return Ok(base_names);
                }
                let mut template_names = Vec::with_capacity(templates.len());
                for t in templates {
                    let n = cu::check!(
                        t.permutated_string_reprs(permutater),
                        "failed to compute template permutations for goff-based namespaced templated name {t:?}"
                    )?;
                    template_names.push(n);
                }
                let template_name_perms = permute(&template_names);
                let mut output = BTreeSet::new();
                for base in &base_names {
                    for templates in &template_name_perms {
                        output.insert(format!("{base}<{}>", templates.join(", ")));
                    }
                }
                Ok(output)
            }
        }
    }
}

impl NamespacedTemplatedName {
    pub fn permutated_string_reprs(
        &self,
        permutater: &mut StructuredNamePermutater,
    ) -> cu::Result<BTreeSet<String>> {
        let base_names = cu::check!(
            self.base.permutated_string_reprs(permutater),
            "failed to compute base permutations for namespaced templated name"
        )?;
        if self.templates.is_empty() {
            return Ok(base_names);
        }
        let mut template_names = Vec::with_capacity(self.templates.len());
        for t in &self.templates {
            let n = cu::check!(
                t.permutated_string_reprs(permutater),
                "failed to compute template permutations for namespaced templated name"
            )?;
            template_names.push(n);
        }
        let template_names = permute(&template_names);
        let mut output = BTreeSet::new();
        for base in base_names {
            for templates in &template_names {
                output.insert(format!("{base}<{}>", templates.join(", ")));
            }
        }
        Ok(output)
    }
}
impl TemplateArg<Goff> {
    pub fn permutated_string_reprs(
        &self,
        permutater: &mut StructuredNamePermutater,
    ) -> cu::Result<BTreeSet<String>> {
        match self {
            TemplateArg::Const(x) => Ok(std::iter::once(x.to_string()).collect()),
            TemplateArg::Type(tree) => tree_goff_permutated_string_reprs(tree, permutater),
            TemplateArg::StaticConst => Ok(std::iter::once("[static]".to_string()).collect()),
        }
    }
}

impl TemplateArg<NamespacedTemplatedName> {
    pub fn permutated_string_reprs(
        &self,
        permutater: &mut StructuredNamePermutater,
    ) -> cu::Result<BTreeSet<String>> {
        match self {
            TemplateArg::Const(x) => Ok(std::iter::once(x.to_string()).collect()),
            TemplateArg::Type(tree) => tree_name_permutated_string_reprs(tree, permutater),
            TemplateArg::StaticConst => Ok(std::iter::once("[static]".to_string()).collect()),
        }
    }
}

fn tree_goff_permutated_string_reprs(
    tree: &Tree<Goff>,
    permutater: &mut StructuredNamePermutater,
) -> cu::Result<BTreeSet<String>> {
    match tree {
        Tree::Base(k) => permutater.permutated_string_reprs_goff(*k),
        Tree::Array(base, len) => {
            let base_names = cu::check!(
                tree_goff_permutated_string_reprs(base, permutater),
                "failed to compute array base permutations"
            )?;
            Ok(base_names
                .into_iter()
                .map(|x| format!("{x}[{len}]"))
                .collect())
        }
        Tree::Ptr(pointee) => {
            if let Tree::Sub(args) = pointee.as_ref() {
                let mut inner_names = Vec::with_capacity(args.len());
                for a in args {
                    let n = cu::check!(
                        tree_goff_permutated_string_reprs(a, permutater),
                        "failed to compute permutations for subroutine type"
                    )?;
                    inner_names.push(n);
                }
                let mut output = BTreeSet::default();
                for arg_names in permute(&inner_names) {
                    let n = format!("{}(*)({})", arg_names[0], arg_names[1..].join(", "));
                    output.insert(n);
                }
                Ok(output)
            } else {
                let base_names = cu::check!(
                    tree_goff_permutated_string_reprs(pointee, permutater),
                    "failed to compute pointee permutations"
                )?;
                Ok(base_names.into_iter().map(|x| format!("{x}*")).collect())
            }
        }
        Tree::Sub(args) => {
            let mut inner_names = Vec::with_capacity(args.len());
            for a in args {
                let n = cu::check!(
                    tree_goff_permutated_string_reprs(a, permutater),
                    "failed to compute permutations for subroutine type"
                )?;
                inner_names.push(n);
            }
            let mut output = BTreeSet::default();
            for arg_names in permute(&inner_names) {
                let n = format!("{}({})", arg_names[0], arg_names[1..].join(", "));
                output.insert(n);
            }
            Ok(output)
        }
        Tree::Ptmd(base, pointee) => {
            let base_names = cu::check!(
                permutater.permutated_string_reprs_goff(*base),
                "failed to compute ptmd base permutations"
            )?;
            let pointee_names = cu::check!(
                tree_goff_permutated_string_reprs(pointee, permutater),
                "failed to compute ptmd pointee permutations"
            )?;
            let mut output = BTreeSet::default();
            for base_n in base_names {
                for pointee_n in &pointee_names {
                    output.insert(format!("{pointee_n} {base_n}::*"));
                }
            }
            Ok(output)
        }
        Tree::Ptmf(base, args) => {
            let base_names = cu::check!(
                permutater.permutated_string_reprs_goff(*base),
                "failed to compute ptmf base permutations"
            )?;
            let mut inner_names = Vec::with_capacity(args.len());
            for a in args {
                let n = cu::check!(
                    tree_goff_permutated_string_reprs(a, permutater),
                    "failed to compute permutations for ptmf subroutine args"
                )?;
                inner_names.push(n);
            }
            let arg_names = permute(&inner_names);

            let mut output = BTreeSet::default();
            for base_n in base_names {
                for arg_n in &arg_names {
                    let retty = &arg_n[0];
                    output.insert(format!("{retty} ({base_n}::*)({})", arg_n[1..].join(", ")));
                }
            }
            Ok(output)
        }
    }
}

fn tree_name_permutated_string_reprs(
    tree: &Tree<NamespacedTemplatedName>,
    permutater: &mut StructuredNamePermutater,
) -> cu::Result<BTreeSet<String>> {
    match tree {
        Tree::Base(name) => name.permutated_string_reprs(permutater),
        Tree::Array(name, len) => {
            let base_names = cu::check!(
                tree_name_permutated_string_reprs(name, permutater),
                "failed to compute array base permutations"
            )?;
            Ok(base_names
                .into_iter()
                .map(|x| format!("{x}[{len}]"))
                .collect())
        }
        Tree::Ptr(name) => {
            if let Tree::Sub(args) = name.as_ref() {
                let mut inner_names = Vec::with_capacity(args.len());
                for a in args {
                    let n = cu::check!(
                        tree_name_permutated_string_reprs(a, permutater),
                        "failed to compute permutations for subroutine type"
                    )?;
                    inner_names.push(n);
                }
                let mut output = BTreeSet::default();
                for arg_names in permute(&inner_names) {
                    let n = format!("{}(*)({})", arg_names[0], arg_names[1..].join(", "));
                    output.insert(n);
                }
                Ok(output)
            } else {
                let base_names = cu::check!(
                    tree_name_permutated_string_reprs(name, permutater),
                    "failed to compute pointee permutations"
                )?;
                Ok(base_names.into_iter().map(|x| format!("{x}*")).collect())
            }
        }
        Tree::Sub(args) => {
            let mut inner_names = Vec::with_capacity(args.len());
            for a in args {
                let n = cu::check!(
                    tree_name_permutated_string_reprs(a, permutater),
                    "failed to compute permutations for subroutine type"
                )?;
                inner_names.push(n);
            }
            let mut output = BTreeSet::default();
            for arg_names in permute(&inner_names) {
                let n = format!("{}({})", arg_names[0], arg_names[1..].join(", "));
                output.insert(n);
            }
            Ok(output)
        }
        Tree::Ptmd(base, pointee) => {
            let base_names = cu::check!(
                base.permutated_string_reprs(permutater),
                "failed to compute ptmd base permutations"
            )?;
            let pointee_names = cu::check!(
                tree_name_permutated_string_reprs(pointee, permutater),
                "failed to compute ptmd pointee permutations"
            )?;
            let mut output = BTreeSet::default();
            for base_n in base_names {
                for pointee_n in &pointee_names {
                    output.insert(format!("{pointee_n} {base_n}::*"));
                }
            }
            Ok(output)
        }
        Tree::Ptmf(base, args) => {
            let base_names = cu::check!(
                base.permutated_string_reprs(permutater),
                "failed to compute ptmf base permutations"
            )?;
            let mut inner_names = Vec::with_capacity(args.len());
            for a in args {
                let n = cu::check!(
                    tree_name_permutated_string_reprs(a, permutater),
                    "failed to compute permutations for ptmf subroutine args"
                )?;
                inner_names.push(n);
            }
            let arg_names = permute(&inner_names);

            let mut output = BTreeSet::default();
            for base_n in base_names {
                for arg_n in &arg_names {
                    let retty = &arg_n[0];
                    output.insert(format!("{retty} ({base_n}::*)({})", arg_n[1..].join(", ")));
                }
            }
            Ok(output)
        }
    }
}

fn permute(input: &[BTreeSet<String>]) -> Vec<Vec<String>> {
    match input.len() {
        0 => vec![],
        1 => input[0].iter().map(|x| vec![x.to_string()]).collect(),
        len => {
            let recur_output = permute(&input[..len - 1]);
            let mut output = Vec::with_capacity(recur_output.len() * len);
            for last in input.last().unwrap() {
                for prev in &recur_output {
                    output.push(
                        prev.iter()
                            .cloned()
                            .chain(std::iter::once(last.clone()))
                            .collect(),
                    );
                }
            }
            output
        }
    }
}

impl NamespacedName {
    pub fn permutated_string_reprs(
        &self,
        permutater: &mut StructuredNamePermutater,
    ) -> cu::Result<BTreeSet<String>> {
        if self.0.is_empty() {
            return Ok(std::iter::once(self.basename().to_string()).collect());
        }
        let namespaces = self.0.permutated_string_reprs(permutater)?;
        Ok(namespaces
            .into_iter()
            .map(|x| format!("{x}::{}", self.1))
            .collect())
    }
}

impl Namespace {
    pub fn permutated_string_reprs(
        &self,
        permutater: &mut StructuredNamePermutater,
    ) -> cu::Result<BTreeSet<String>> {
        let mut output = BTreeSet::new();
        for n in &self.0 {
            match n {
                NameSeg::Name(s) => {
                    if output.is_empty() {
                        output = std::iter::once(s.to_string()).collect();
                    } else {
                        output = output.into_iter().map(|x| format!("{x}::{s}")).collect();
                    }
                }
                NameSeg::Type(k, _) => {
                    // the type repr contains the namespace, so we can discard the previous
                    output = permutater.permutated_string_reprs_goff(*k)?;
                    // if the type returns empty names, it means the type is being resolved
                    // recursively, so we discard this name by returning empty
                    if output.is_empty() {
                        return Ok(output);
                    }
                }
                NameSeg::Subprogram(_, name, is_linkage_name) => {
                    if *is_linkage_name {
                        output = std::iter::once(name.to_string()).collect();
                    } else {
                        output = output
                            .into_iter()
                            .map(|x| format!("{x}::(function {name})"))
                            .collect();
                    }
                }
                NameSeg::Anonymous => {}
            }
        }
        Ok(output)
    }
}
