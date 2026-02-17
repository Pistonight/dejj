use std::borrow::Cow;
use std::sync::Arc;

use cu::pre::*;
use exstructs::Goff;
use gimli::{Abbreviations, AttributeValue, DwAt, Operation, UnitSectionOffset};

use crate::dwarf::{Die, Dwarf, EntriesTree, In, Loff};

pub struct UnitIter {
    pub(crate) debug_info_iter: gimli::DebugInfoUnitHeadersIter<In<'static>>,
    pub(crate) dwarf: Arc<Dwarf>,
}

impl UnitIter {
    pub fn next_unit(&mut self) -> cu::Result<Option<Unit>> {
        let header = cu::check!(
            self.debug_info_iter.next(),
            "failed to read next unit header"
        )?;
        let Some(header) = header else {
            return Ok(None);
        };
        let offset = match header.offset() {
            UnitSectionOffset::DebugInfoOffset(o) => o.0,
            UnitSectionOffset::DebugTypesOffset(o) => {
                cu::bail!(
                    "failed to get DWARF offset for compilation unit: expecting DebugInfoOffset, got {o:?}"
                );
            }
        };
        let unit = cu::check!(
            gimli::Unit::new(&self.dwarf.dwarf, header),
            "failed to create debug info unit"
        )?;
        let abbrevs = cu::check!(
            header.abbreviations(&self.dwarf.dwarf.debug_abbrev),
            "failed to create debug info unit abbrevs"
        )?;
        let mut unit = Unit {
            unit,
            header,
            abbrevs,
            dwarf: Arc::clone(&self.dwarf),
            name: String::new(),
            offset: offset.into(),
        };

        let mut tree = cu::check!(
            unit.tree(),
            "failed to parse root node when creating debug info unit"
        )?;
        let root = cu::check!(
            tree.root(),
            "failed to parse root node when creating debug info unit"
        )?;
        let entry = root.entry();
        let name = cu::check!(entry.name(), "failed to get name of compilation unit")?;
        unit.name = name.to_string();
        Ok(Some(unit))
    }
}

/// Holder of a Unit in .debug_info
#[derive(Display)]
#[display("compilation unit at {} ({})", self.offset, self.name)]
pub struct Unit {
    unit: gimli::Unit<In<'static>>,
    header: gimli::UnitHeader<In<'static>>,
    abbrevs: Abbreviations,
    dwarf: Arc<Dwarf>,
    /// name of the unit (typically file name)
    pub name: String,
    /// offset of the unit
    pub offset: Goff,
}

impl Unit {
    pub fn tree(&self) -> cu::Result<EntriesTree<'_>> {
        self.entries_tree(None)
    }
    pub fn tree_at(&self, loff: Loff) -> cu::Result<EntriesTree<'_>> {
        self.entries_tree(Some(loff))
    }
    fn entries_tree(&self, loff: Option<Loff>) -> cu::Result<EntriesTree<'_>> {
        let tree = match loff {
            None => cu::check!(
                self.unit.entries_tree(None),
                "failed to parse root for {self}"
            )?,
            Some(loff) => cu::check!(
                self.unit.entries_tree(Some(loff.into())),
                "failed to parse tree at {} for {self}",
                self.goff(loff)
            )?,
        };
        Ok(EntriesTree { unit: self, tree })
    }

    /// Get a single entry at offset
    pub fn entry_at<'x>(&'x self, loff: Loff) -> cu::Result<Die<'x, 'x>> {
        let entry = self.unit.entry(loff.into());
        let entry = cu::check!(
            entry,
            "failed to read entry at {} for {self}",
            self.goff(loff)
        )?;
        Ok(Die {
            unit: self,
            entry: Cow::Owned(entry),
        })
    }

    /// Convert local offset in this compilation unit to global offset
    pub fn goff(&self, loff: Loff) -> Goff {
        loff.to_global(self.offset)
    }

    /// Get an attribute value as string
    pub(crate) fn attr_string<'x>(
        &'x self,
        value: AttributeValue<In<'static>>,
    ) -> cu::Result<&'x str> {
        let value = cu::check!(
            self.dwarf.dwarf.attr_string(&self.unit, value),
            "failed to get attribute value as string in {self}"
        )?;
        cu::check!(
            value.to_string(),
            "failed to decode attribute value as string in {self}"
        )
    }
    /// Get an attribute value as signed integer
    pub(crate) fn attr_signed(
        &self,
        offset: Goff,
        at: DwAt,
        attr: AttributeValue<In<'_>>,
    ) -> cu::Result<i64> {
        match attr {
            AttributeValue::Data1(x) => Ok(x as i64),
            AttributeValue::Data2(x) => Ok(x as i64),
            AttributeValue::Data4(x) => Ok(x as i64),
            AttributeValue::Data8(x) => Ok(x as i64),
            AttributeValue::Udata(x) => Ok(x as i64),
            AttributeValue::Sdata(x) => Ok(x as i64),
            _ => cu::bail!("expecting signed data for entry {offset}, attr {at}"),
        }
    }
    /// Get an attribute value as unsigned integer
    pub(crate) fn attr_unsigned(
        &self,
        offset: Goff,
        at: DwAt,
        attr: AttributeValue<In<'_>>,
    ) -> cu::Result<u64> {
        match attr {
            AttributeValue::Data1(x) => Ok(x as u64),
            AttributeValue::Data2(x) => Ok(x as u64),
            AttributeValue::Data4(x) => Ok(x as u64),
            AttributeValue::Data8(x) => Ok(x),
            AttributeValue::Udata(x) => Ok(x),
            AttributeValue::Addr(x) => Ok(x),
            // this is used for vtable elem location
            AttributeValue::Exprloc(expr) => {
                let mut ops = expr.operations(self.unit.encoding());
                let op = cu::check!(
                    ops.next(),
                    "failed to read Exprloc ops for entry {offset}, attr {at}"
                )?;
                let op = cu::check!(op, "expecting an Exprloc ops for entry {offset}, attr {at}")?;
                let Operation::UnsignedConstant { value } = op else {
                    cu::bail!(
                        "expecting UnsignedConstant for Exprloc ops for entry {offset}, attr {at}"
                    );
                };
                Ok(value)
            }
            other => {
                cu::bail!("expecting unsigned data for entry {offset}, attr {at}, got: {other:?}")
            }
        }
    }
}
