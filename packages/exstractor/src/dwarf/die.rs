use std::borrow::Cow;

use cu::pre::*;
use exstructs::{Goff, NamespaceMaps, NamespacedName};
use gimli::AttributeValue;
use gimli::constants::*;
use tyyaml::Prim;

use crate::dwarf::{In, Loff, Tag, Unit};

pub struct EntriesTree<'x> {
    pub(crate) unit: &'x Unit,
    pub(crate) tree: gimli::EntriesTree<'x, 'x, In<'static>>,
}

impl<'x> EntriesTree<'x> {
    pub fn root(&mut self) -> cu::Result<DieNode<'x, '_>> {
        let node = cu::check!(
            self.tree.root(),
            "failed to parse tree node in {}",
            self.unit
        )?;
        Ok(DieNode {
            unit: self.unit,
            node,
        })
    }
}

pub struct DieNode<'x, 't> {
    pub(crate) node: gimli::EntriesTreeNode<'x, 'x, 't, In<'static>>,
    pub(crate) unit: &'x Unit,
}

impl<'x> DieNode<'x, '_> {
    #[allow(unused)]
    pub fn unit(&self) -> &'x Unit {
        self.unit
    }
    pub fn entry(&self) -> Die<'x, '_> {
        let entry = self.node.entry();
        Die {
            unit: self.unit,
            entry: Cow::Borrowed(entry),
        }
    }
    pub fn goff(&self) -> Goff {
        self.unit.goff(self.node.entry().offset().into())
    }
    /// Execute f on each direct child node (does not include this node)
    pub fn for_each_child<F>(self, mut f: F) -> cu::Result<()>
    where
        F: for<'t> FnMut(DieNode<'x, 't>) -> cu::Result<()>,
    {
        let offset = self.goff();
        let mut children = self.node.children();
        while let Some(child) = cu::check!(
            children.next(),
            "failed to read a child for entry at {offset} in {}",
            self.unit
        )? {
            let node = DieNode {
                node: child,
                unit: self.unit,
            };
            let child_offset = node.goff();
            cu::check!(
                f(node),
                "error while processing child entry at {child_offset}"
            )?;
        }
        Ok(())
    }
}

pub struct Die<'x, 'n> {
    pub(crate) unit: &'x Unit,
    pub(crate) entry: Cow<'n, gimli::DebuggingInformationEntry<'x, 'x, In<'static>, usize>>,
}

impl<'x> Die<'x, '_> {
    /// Get the global offset of this entry
    pub fn goff(&self) -> Goff {
        (self.entry.offset().0 + usize::from(self.unit.offset)).into()
    }
    pub fn to_global(&self, loff: Loff) -> Goff {
        self.unit.goff(loff)
    }
    /// Get the unit
    pub fn unit(&self) -> &'x Unit {
        self.unit
    }
    pub fn tag(&self) -> Tag {
        self.entry.tag()
    }
    /// Get the name of the entry
    pub fn name(&self) -> cu::Result<&str> {
        let value = self.name_opt()?;
        let offset = self.goff();
        let value = cu::check!(
            value,
            "DW_AT_name is missing for entry at offset {offset} in {}",
            self.unit
        )?;
        Ok(value)
    }

    // /// Get the name of the entry before the first `<`. This can only be used
    // /// for types, and not function names, because of `operator<=`
    // pub fn untemplated_name(&self) -> cu::Result<&str> {
    //     let value = self.untemplated_name_opt()?;
    //     let offset = self.goff();
    //     let value = cu::check!(
    //         value,
    //         "DW_AT_name is missing for entry at offset {offset} in {}",
    //         self.unit
    //     )?;
    //     Ok(value)
    // }

    /// Get the name of the entry before the first `<`. This can only be used
    /// for types, and not function names, because of `operator<=`
    pub fn untemplated_name_opt(&self) -> cu::Result<Option<&str>> {
        let value = self.name_opt()?;
        Ok(value.map(|x| match x.find('<') {
            Some(i) => &x[..i],
            None => x,
        }))
    }

    /// Get the DW_AT_name of a DIE, if it exists
    pub fn name_opt(&self) -> cu::Result<Option<&str>> {
        self.str_opt(DW_AT_name)
    }

    /// Get a string attribute value
    pub fn str_opt(&self, attr: DwAt) -> cu::Result<Option<&str>> {
        let offset = self.goff();
        let value = cu::check!(
            self.entry.attr_value(attr),
            "failed to read {attr} at {offset} in {}",
            self.unit
        )?;
        let Some(value) = value else {
            return Ok(None);
        };
        let value = cu::check!(
            self.unit.attr_string(value),
            "failed to read value for {attr} at {offset} in {}",
            self.unit
        )?;
        Ok(Some(value))
    }
    /// Get a signed integer attribute value
    pub fn int(&self, attr: DwAt) -> cu::Result<i64> {
        let value = self.int_opt(attr)?;
        let offset = self.goff();
        let value = cu::check!(value, "entry is missing {attr} at offset {offset}")?;
        Ok(value)
    }
    /// Get a signed integer attribute value, allowing it to be missing
    pub fn int_opt(&self, attr: DwAt) -> cu::Result<Option<i64>> {
        let offset = self.goff();
        let value = cu::check!(
            self.entry.attr_value(attr),
            "failed to read {attr} at offset {offset}"
        )?;
        let Some(value) = value else {
            return Ok(None);
        };
        let value = self.unit.attr_signed(offset, attr, value)?;
        Ok(Some(value))
    }
    /// Get an unsigned integer attribute value
    pub fn uint(&self, attr: DwAt) -> cu::Result<u64> {
        let value = self.uint_opt(attr)?;
        let offset = self.goff();
        let value = cu::check!(value, "entry is missing {attr} at offset {offset}")?;
        Ok(value)
    }
    /// Get an unsigned integer attribute value, allowing it to be missing
    pub fn uint_opt(&self, attr: DwAt) -> cu::Result<Option<u64>> {
        let offset = self.goff();
        let value = cu::check!(
            self.entry.attr_value(attr),
            "failed to read {attr} at offset {offset}"
        )?;
        let Some(value) = value else {
            return Ok(None);
        };
        let value = self.unit.attr_unsigned(offset, attr, value)?;
        Ok(Some(value))
    }
    /// Get an attr of an entry as flag
    pub fn flag(&self, attr: DwAt) -> cu::Result<bool> {
        let offset = self.goff();
        let value = cu::check!(
            self.entry.attr_value(attr),
            "failed to read {attr} at {offset}"
        )?;
        match value {
            None => Ok(false),
            Some(AttributeValue::Flag(x)) => Ok(x),
            _ => {
                cu::bail!("expecting {attr} to be a Flag, at entry {offset}");
            }
        }
    }
    /// Get the DW_TAG_vtable_elem_location of a DIE (index of the entry in the vtable), return None if not virtual
    pub fn vtable_index(&self) -> cu::Result<Option<usize>> {
        let offset = self.goff();
        let virtuality = cu::check!(
            self.entry.attr_value(DW_AT_virtuality),
            "failed to read DW_AT_virtuality for entry at {offset}"
        )?;
        let velem = cu::check!(
            self.entry.attr_value(DW_AT_vtable_elem_location),
            "failed to read DW_AT_vtable_elem_location for entry at {offset}"
        )?;
        match virtuality {
            None | Some(AttributeValue::Virtuality(DW_VIRTUALITY_none)) => {
                // vtable_elem_localtion should not be there
                if velem.is_some() {
                    cu::bail!(
                        "DW_AT_vtable_elem_location should not exist for non virtual entry at {offset}"
                    );
                }
                Ok(None)
            }
            Some(AttributeValue::Virtuality(DW_VIRTUALITY_virtual))
            | Some(AttributeValue::Virtuality(DW_VIRTUALITY_pure_virtual)) => {
                let velem = cu::check!(
                    velem,
                    "missing DW_AT_vtable_elem_location for virtual entry at {offset}"
                )?;
                let vel = self
                    .unit
                    .attr_unsigned(offset, DW_AT_vtable_elem_location, velem)?;
                Ok(Some(vel as usize))
            }
            _ => cu::bail!("expecting DW_AT_virtuality to be Virtuality, at entry {offset}"),
        }
    }

    /// Read an attribute of a DIE, expecting a unit reference (local offset)
    pub fn loff(&self, attr: DwAt) -> cu::Result<Loff> {
        let t = self.loff_opt(attr)?;
        cu::check!(t, "missing {attr} for entry at offset {}", self.goff())
    }

    /// Read an attribute of a DIE, expecting a local offset, allowing it to be missing
    pub fn loff_opt(&self, attr: DwAt) -> cu::Result<Option<Loff>> {
        let offset = self.goff();
        let type_value = cu::check!(
            self.entry.attr_value(attr),
            "failed to read {attr} at offset {offset}"
        )?;
        let Some(type_value) = type_value else {
            return Ok(None);
        };
        let type_offset = match type_value {
            AttributeValue::UnitRef(offset) => offset,
            _ => cu::bail!("expecting {attr} to be a unit ref at offset {offset}"),
        };
        Ok(Some(type_offset.into()))
    }

    /// Read this entry as a primitive type node
    pub fn prim_type(&self) -> cu::Result<Prim> {
        let offset = self.goff();
        let encoding = cu::check!(
            self.entry.attr_value(DW_AT_encoding),
            "failed to read DW_AT_encoding for primitive type at offset {offset}"
        )?;
        let encoding = cu::check!(
            encoding,
            "missing DW_AT_encoding for primitive type at offset {offset}"
        )?;
        let AttributeValue::Encoding(encoding) = encoding else {
            cu::bail!(
                "expecting an Encoding attribute for DW_AT_encoding for primitive type at offset {offset}"
            );
        };
        let byte_size = cu::check!(
            self.uint(DW_AT_byte_size),
            "failed to get byte size for primitive type at offset {offset}"
        )?;
        let prim = match (encoding, byte_size) {
            (DW_ATE_boolean, 0x1) => Prim::Bool,
            (DW_ATE_unsigned, 0x1) => Prim::U8,
            (DW_ATE_unsigned_char, 0x1) => Prim::U8,
            (DW_ATE_signed, 0x1) => Prim::I8,
            (DW_ATE_signed_char, 0x1) => Prim::I8,

            (DW_ATE_unsigned, 0x2) => Prim::U16,
            (DW_ATE_signed, 0x2) => Prim::I16,
            (DW_ATE_UTF, 0x2) => Prim::U16,

            (DW_ATE_unsigned, 0x4) => Prim::U32,
            (DW_ATE_signed, 0x4) => Prim::I32,
            (DW_ATE_float, 0x4) => Prim::F32,

            (DW_ATE_unsigned, 0x8) => Prim::U64,
            (DW_ATE_signed, 0x8) => Prim::I64,
            (DW_ATE_float, 0x8) => Prim::F64,

            (DW_ATE_unsigned, 0x10) => Prim::U128,
            (DW_ATE_signed, 0x10) => Prim::I128,
            (DW_ATE_float, 0x10) => Prim::F128,

            _ => cu::bail!("unknown primitive type. encoding: {encoding}, byte size: {byte_size}"),
        };

        Ok(prim)
    }

    pub fn is_inlined(&self) -> cu::Result<bool> {
        let offset = self.goff();
        let inline = cu::check!(
            self.entry.attr_value(DW_AT_inline),
            "failed to read DW_AT_inline for entry at offset {offset}"
        )?;
        match inline {
            None => Ok(false),
            Some(AttributeValue::Inline(x)) => {
                Ok(matches!(x, DW_INL_inlined | DW_INL_declared_inlined))
            }
            _ => {
                cu::bail!("expecting DW_AT_inline to be an inline attribute at offset {offset}")
            }
        }
    }

    /// Execute f on each direct child node (does not include the input node)
    pub fn for_each_child<F>(&self, f: F) -> cu::Result<()>
    where
        F: for<'t> FnMut(DieNode<'x, 't>) -> cu::Result<()>,
    {
        let mut tree = self.unit.tree_at(self.entry.offset().into())?;
        let node = tree.root()?;
        node.for_each_child(f)
    }

    // namespaces

    // /// Get the name of the entry with namespace prefix, without templated args
    // pub fn untemplated_qual_name(&self, namespaces: &NamespaceMaps) -> cu::Result<NamespacedName> {
    //     let name = self.untemplated_name()?;
    //     Self::make_qual_name(namespaces, self.goff(), name)
    // }
    /// Get the name of the entry with namespace prefix, without templated args
    pub fn untemplated_qual_name_opt(
        &self,
        nsmaps: &NamespaceMaps,
    ) -> cu::Result<Option<NamespacedName>> {
        let Some(name) = self.untemplated_name_opt()? else {
            return Ok(None);
        };
        Self::make_qual_name(nsmaps, self.goff(), name).map(Some)
    }
    /// Get the name of the entry with namespace prefix
    pub fn qual_name(&self, nsmaps: &NamespaceMaps) -> cu::Result<NamespacedName> {
        let name = self.name()?;
        Self::make_qual_name(nsmaps, self.goff(), name)
    }

    // /// Get the name of the entry with namespace prefix, optional
    // pub fn qual_name_opt(&self, nsmaps: &NamespaceMaps) -> cu::Result<Option<NamespacedName>> {
    //     let Some(name) = self.name_opt()? else {
    //         return Ok(None);
    //     };
    //     Self::make_qual_name(nsmaps, self.goff(), name).map(Some)
    // }

    fn make_qual_name(
        nsmaps: &NamespaceMaps,
        offset: Goff,
        name: &str,
    ) -> cu::Result<NamespacedName> {
        let namespace = cu::check!(
            nsmaps.qualifiers.get(&offset),
            "cannot find namespace for entry {offset}, with name '{name}'"
        )?;
        Ok(NamespacedName::namespaced(namespace, name))
    }
}
