use cu::pre::*;
use exstructs::Goff;
use gimli::constants::*;

pub type In<'i> = gimli::EndianSlice<'i, gimli::LittleEndian>;
pub type Tag = gimli::DwTag;

pub fn is_type_tag(tag: Tag) -> bool {
    match tag {
        DW_TAG_structure_type
        | DW_TAG_class_type
        | DW_TAG_union_type
        | DW_TAG_enumeration_type
        // typedefs
        | DW_TAG_unspecified_type
        | DW_TAG_typedef
        // pointer
        | DW_TAG_pointer_type
        | DW_TAG_reference_type
        | DW_TAG_array_type
        // qualifier
        | DW_TAG_const_type
        | DW_TAG_volatile_type
        | DW_TAG_restrict_type
        // function
        | DW_TAG_subroutine_type
        | DW_TAG_ptr_to_member_type
        // base
        | DW_TAG_base_type => true,

        // this is to prevent constants above from being interpreted as variable ident
        _tag => false
    }
}

/// Local offset into a Compilation Unit in DWARF
#[rustfmt::skip]
#[derive(
    DebugCustom, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord,
    Into, Display
)]
#[display("local(0x{:08x})", self.0)]
#[debug("local(0x{:08x})", self.0)]
pub struct Loff(usize);

impl From<gimli::UnitOffset<usize>> for Loff {
    fn from(value: gimli::UnitOffset<usize>) -> Self {
        Self(value.0)
    }
}

impl From<Loff> for gimli::UnitOffset<usize> {
    fn from(value: Loff) -> Self {
        Self(value.0)
    }
}

impl Loff {
    /// Convert unit-local offset to global offset by adding the offset of the unit
    #[inline(always)]
    pub fn to_global(self, unit_offset: impl Into<usize>) -> Goff {
        Goff(self.0 + unit_offset.into())
    }
}
