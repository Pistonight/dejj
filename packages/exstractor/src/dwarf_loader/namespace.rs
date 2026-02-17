use std::collections::BTreeMap;

use cu::pre::*;
use exstructs::{Goff, GoffMap, NameSeg, Namespace, NamespaceMaps};
use gimli::constants::*;

use crate::dwarf::{self, DieNode, Unit};

#[derive(Default)]
struct LoadNamespaceCtx {
    // the difference between qualifier and namespace
    // is that qualifier contains types/subprograms,
    // while namespace only contains namespaces
    current_qualifier: NamespaceStack,
    current_namespace: NamespaceStack,
    offset_to_ns: GoffMap<Namespace>,
    offset_to_qual: GoffMap<Namespace>,
}

impl LoadNamespaceCtx {
    fn register_current_at_offset(&mut self, off: Goff) {
        self.offset_to_qual
            .insert(off, self.current_qualifier.curr());
        self.offset_to_ns.insert(off, self.current_namespace.curr());
    }
}

#[derive(Default)]
struct NamespaceStack {
    stack: Vec<NameSeg>,
}
impl NamespaceStack {
    pub fn push(&mut self, s: NameSeg) {
        self.stack.push(s);
    }

    pub fn pop(&mut self) {
        self.stack.pop();
    }

    pub fn curr(&self) -> Namespace {
        Namespace(self.stack.clone())
    }
}

/// Load the namespaces in this compilation unit as a global offset map
pub fn load_namespaces(unit: &Unit) -> cu::Result<NamespaceMaps> {
    cu::debug!("loading namespaces for {unit}");
    let mut ctx = LoadNamespaceCtx::default();
    cu::check!(
        load_namespaces_root(unit, &mut ctx),
        "failed to load namespaces for {unit}"
    )?;
    let mut by_src_map: BTreeMap<String, Namespace> = Default::default();
    for namespace in ctx.offset_to_qual.values() {
        use std::collections::btree_map::Entry;

        if namespace.contains_anonymous() {
            continue;
        }
        let Ok(source) = namespace.to_cpp_typedef_source() else {
            continue;
        };

        match by_src_map.entry(source) {
            Entry::Vacant(e) => {
                e.insert(namespace.clone());
            }
            Entry::Occupied(e) => {
                let existing = e.get();
                cu::ensure!(
                    existing.source_segs_equal(namespace),
                    "namespace with the same source does not have the same segments: {existing:?} and {namespace:?}"
                )?;
            }
        }
    }
    Ok(NamespaceMaps {
        qualifiers: ctx.offset_to_qual,
        namespaces: ctx.offset_to_ns,
        by_src: by_src_map,
    })
}

fn load_namespaces_root(unit: &Unit, ctx: &mut LoadNamespaceCtx) -> cu::Result<()> {
    let mut tree = unit.tree()?;
    let root = tree.root()?;
    load_namespace_recur(root, ctx)?;
    Ok(())
}

fn load_namespace_recur(node: DieNode<'_, '_>, ctx: &mut LoadNamespaceCtx) -> cu::Result<()> {
    let entry = node.entry();
    let offset = entry.goff();
    let tag = entry.tag();
    if dwarf::is_type_tag(tag) {
        // types could be defined inside a type
        ctx.register_current_at_offset(offset);
        // only push the qualifier stack for types
        match entry.name_opt()? {
            Some(name) => {
                ctx.current_qualifier
                    .push(NameSeg::Type(offset, name.into()));
            }
            None => {
                ctx.current_qualifier.push(NameSeg::Anonymous);
            }
        }
        node.for_each_child(|child| load_namespace_recur(child, ctx))?;
        ctx.current_qualifier.pop();
    } else {
        match tag {
            DW_TAG_compile_unit => {
                node.for_each_child(|child| load_namespace_recur(child, ctx))?;
            }
            DW_TAG_variable => {
                ctx.register_current_at_offset(offset);
                node.for_each_child(|child| load_namespace_recur(child, ctx))?;
            }
            // types could be defined inside a function
            DW_TAG_subprogram => {
                ctx.register_current_at_offset(offset);
                let linkage_name = super::load_func_linkage_name(&entry)?;
                match linkage_name {
                    Some(n) => ctx.current_qualifier.push(NameSeg::Subprogram(
                        offset,
                        n.as_str().into(),
                        true,
                    )),
                    None => {
                        let name = super::load_func_name(&entry)?;
                        let name = match name {
                            Some(name) => name,
                            None => "anonymous".to_string(),
                        };
                        ctx.current_qualifier.push(NameSeg::Subprogram(
                            offset,
                            name.as_str().into(),
                            false,
                        ));
                    }
                }
                node.for_each_child(|child| load_namespace_recur(child, ctx))?;
                ctx.current_qualifier.pop();
            }
            DW_TAG_namespace => {
                ctx.register_current_at_offset(offset);
                match entry.name_opt()? {
                    Some(name) => {
                        let seg = NameSeg::Name(name.into());
                        ctx.current_qualifier.push(seg.clone());
                        ctx.current_namespace.push(seg);
                    }
                    None => {
                        ctx.current_qualifier.push(NameSeg::Anonymous);
                        ctx.current_namespace.push(NameSeg::Anonymous);
                    }
                };
                node.for_each_child(|child| load_namespace_recur(child, ctx))?;
                ctx.current_qualifier.pop();
                ctx.current_namespace.pop();
            }
            _ => {
                // ignore
            }
        }
    }

    Ok(())
}
