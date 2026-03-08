use cu::pre::*;
use tyyaml::{Prim, Tree};

use crate::{ArcStr, FullQualName, Goff, Namespace, NamespacedName, NamespacedTemplatedName, TemplateArg};

/// High-level (H) Type data
///
/// - Declarations are merged with the definitions
///   - Undefined declarations become empty struct with the name
///     i.e. as `struct Foo;`
/// - Typedef names are merged with the definitions
/// - All compile units are linked together
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HType {
    /// Pritimive type
    Prim(Prim),
    /// Enum
    Enum(HTypeData<Enum>),
    /// Union
    Union(HTypeData<Union>),
    /// Struct/Class
    Struct(HTypeData<Struct>),
}

impl HType {
    pub fn as_enum_mut(&mut self) -> cu::Result<&mut HTypeData<Enum>> {
        match self {
            HType::Enum(data) => Ok(data),
            _ => cu::bail!("expected HTYPE to be enum"),
        }
    }
    pub fn as_union_mut(&mut self) -> cu::Result<&mut HTypeData<Union>> {
        match self {
            HType::Union(data) => Ok(data),
            _ => cu::bail!("expected HTYPE to be union"),
        }
    }
    pub fn as_struct_mut(&mut self) -> cu::Result<&mut HTypeData<Struct>> {
        match self {
            HType::Struct(data) => Ok(data),
            _ => cu::bail!("expected HTYPE to be struct"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HTypeData<T> {
    /// All fually qualified names of this type. Empty means this type is currently
    /// anonymous
    pub fqnames: Vec<FullQualName>,
    /// The data of the type
    pub data: T,
}

mod imp_mtype {
    use super::*;
    /// Mid-level (M) Type data
    ///
    /// This is the abstraction used for merging and optimizing types
    ///
    /// - Aliases are merged & eliminated
    /// - Trees are flattened: A Tree::Base definitely points to a
    ///   primitive, enum, union or struct.
    /// - Typedefs to composite types are eliminated
    /// - Other typedefs have their names merged into the target
    #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
    pub enum MType {
        /// Pritimive type
        Prim(Prim),
        /// Enum. The name does not include template args. could be anonymous
        Enum(MTypeData<Enum>),
        /// Declaration of an enum.
        /// Name includes template args
        EnumDecl(MTypeDecl),
        /// Union. The name does not include template args. could be anonymous
        Union(MTypeData<Union>),
        /// Declaration of union.
        UnionDecl(MTypeDecl),
        /// Struct or Class. The name does not include template args. could be anonymous
        Struct(MTypeData<Struct>),
        /// Declaration of struct or class.
        StructDecl(MTypeDecl),
    }

    /// Data of an MType
    #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
    pub struct MTypeData<T> {
        /// The name; does not include template args; None means anonymous
        pub name: Option<NamespacedName>,
        /// Declared aliases (e.g from typedefs)
        pub decl_names: Vec<NamespacedTemplatedName>,
        /// The data
        pub data: T,
    }

    /// Declaration data of an MType
    #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
    pub struct MTypeDecl {
        /// Primary name, includes template args
        pub name: NamespacedTemplatedName,
        /// Other names from typedef
        pub typedef_names: Vec<NamespacedTemplatedName>,
    }
}
pub use imp_mtype::{MType, MTypeData, MTypeDecl};

mod imp_ltype {
    use super::*;
    /// Low-level (L) Type data
    ///
    /// This mostly maps to raw data parsed from DWARF
    ///
    /// - Trees are not flattened: for example, A Tree::Base could be pointing to a Goff
    ///   that is a pointer type.
    /// - Templates are not parsed: Declarations and typedefs could have templates embedded in the name
    #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
    #[rkyv(compare(PartialEq))]
    pub enum LType {
        /// Pritimive type
        Prim(Prim),
        /// Typedef <target> <name>;
        Typedef {
            /// Name of the typedef, could have template args
            name: NamespacedName,
            /// Typedef target
            target: Goff,
        },
        /// Enum
        Enum(LTypeData<EnumUndeterminedSize>),
        /// Declaration of an enum.
        EnumDecl(LTypeDecl),
        /// Union.
        Union(LTypeData<Union>),
        /// Declaration of union.
        UnionDecl(LTypeDecl),
        /// Struct or Class.
        Struct(LTypeData<Struct>),
        /// Declaration of struct or class.
        StructDecl(LTypeDecl),

        /// Composite types
        Tree(Tree<Goff>),
        /// Alias to another type for type layout purpose (basically typedef without a name)
        Alias(Goff),
    }

    /// Data of a LType
    #[rustfmt::skip]
    #[derive(
        Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize,
        rkyv::Archive, rkyv::Serialize, rkyv::Deserialize
    )]
    #[rkyv(derive(PartialEq))]
    #[rkyv(compare(PartialEq))]
    #[rkyv(archive_bounds(
        <T as rkyv::Archive>::Archived: PartialEq
    ))]
    pub struct LTypeData<T> {
        /// The name; does not include template args; None means anonymous
        pub name: Option<NamespacedName>,
        /// The data
        pub data: T,
    }

    /// Declaration data of a LType
    #[rustfmt::skip]
    #[derive(
        Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize,
        rkyv::Archive, rkyv::Serialize, rkyv::Deserialize
    )]
    #[rkyv(derive(PartialEq))]
    #[rkyv(compare(PartialEq))]
    pub struct LTypeDecl {
        /// The enclosing namespace that is required to resolve the names
        /// in the template args
        pub enclosing: Namespace,
        /// The name; could include template args
        pub name_with_tpl: NamespacedName,
    }
}
pub use imp_ltype::{LType, LTypeData, LTypeDecl};

mod imp_enum {
    use super::*;

    /// Data of an `enum`
    #[rustfmt::skip]
    #[derive(
        Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize,
        rkyv::Archive, rkyv::Serialize, rkyv::Deserialize
    )]
    #[rkyv(derive(PartialEq))]
    #[rkyv(compare(PartialEq))]
    pub struct Enum {
        /// Base type, used to determine the size
        pub byte_size: u32,
        /// Enumerators of the enum, in the order they appear in DWARF
        pub enumerators: Vec<Enumerator>,
    }
    /// An enum with size not-yet fully determined (could be linked to another type)
    #[rustfmt::skip]
    #[derive(
        Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize,
        rkyv::Archive, rkyv::Serialize, rkyv::Deserialize
    )]
    #[rkyv(derive(PartialEq))]
    #[rkyv(compare(PartialEq))]
    pub struct EnumUndeterminedSize {
        /// Base type, used to determine the size
        pub byte_size_or_base: Result<u32, Goff>,
        /// Enumerators of the enum, in the order they appear in DWARF
        pub enumerators: Vec<Enumerator>,
    }
}
pub use imp_enum::{Enum, EnumUndeterminedSize};

mod imp_union {
    use super::*;
    /// Data of a `union`
    #[rustfmt::skip]
    #[derive(
        Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize,
        rkyv::Archive, rkyv::Serialize, rkyv::Deserialize
    )]
    #[rkyv(derive(PartialEq))]
    #[rkyv(compare(PartialEq))]
    pub struct Union {
        /// Byte size of the union (should be size of the largest member)
        pub byte_size: u32,
        /// Template arguments, if any
        pub template_args: Vec<TemplateArg<Goff>>,
        /// Union members. The members must have offset of 0 and special of None
        pub members: Vec<Member>,
    }
}
pub use imp_union::Union;

mod imp_struct {
    use super::*;
    /// Data of a `struct` or `class`
    #[rustfmt::skip]
    #[derive(
        Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize,
        rkyv::Archive, rkyv::Serialize, rkyv::Deserialize
    )]
    #[rkyv(derive(PartialEq))]
    #[rkyv(compare(PartialEq))]
    pub struct Struct {
        /// Byte size of the struct
        pub byte_size: u32,
        /// Template specialization of the struct, if any
        pub template_args: Vec<TemplateArg<Goff>>,
        /// Members of the struct
        pub members: Vec<Member>,
        /// Vtable of the struct. (index, entry).
        /// Dtors will have an index of 0
        pub vtable: Vec<(u32, VtableEntry)>,
    }
}
pub use imp_struct::Struct;

impl Struct {
    /// Create a zero-sized type (which has sizeof(T) == 1)
    pub fn zst() -> Self {
        Self::zst_with_templates(vec![])
    }
    pub fn zst_with_templates(template_args: Vec<TemplateArg<Goff>>) -> Self {
        Self {
            byte_size: 1,
            template_args,
            members: vec![],
            vtable: vec![],
        }
    }
}

mod imp_enumerator {
    use super::*;
    #[rustfmt::skip]
    #[derive(
        Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize,
        rkyv::Archive, rkyv::Serialize, rkyv::Deserialize
    )]
    #[rkyv(derive(PartialEq))]
    #[rkyv(compare(PartialEq))]
    pub struct Enumerator {
        /// Name of the enumerator
        pub name: ArcStr,
        /// Value of the enumerator. If the enumerator is unsigned
        /// and the value is greater than `i64::MAX`, then it's stored
        /// as if it's a `u64`. Enum type of byte size greater than 8
        /// is not allowed right now
        pub value: i64,
    }
}
pub use imp_enumerator::Enumerator;

mod imp_member {
    use super::*;
    /// A struct or union member
    #[rustfmt::skip]
    #[derive(
        Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize,
        rkyv::Archive, rkyv::Serialize, rkyv::Deserialize
    )]
    #[rkyv(derive(PartialEq))]
    #[rkyv(compare(PartialEq))]
    pub struct Member {
        /// Offset of the member within the struct. 0 For union.
        pub offset: u32,
        /// Name of the member. Could be None for anonymous typed member
        pub name: Option<ArcStr>,
        /// Type of the member. Might be unflattened, depending on the stage
        pub ty: Tree<Goff>,
        /// Special-case member, None for union
        pub special: Option<SpecialMember>,
    }
}
pub use imp_member::Member;

impl Member {
    pub fn is_base(&self) -> bool {
        matches!(self.special, Some(SpecialMember::Base))
    }
}

mod imp_special_member {
    use super::*;
    /// Special member type
    #[rustfmt::skip]
    #[derive(
        Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize,
        rkyv::Archive, rkyv::Serialize, rkyv::Deserialize
    )]
    #[rkyv(derive(PartialEq))]
    #[rkyv(compare(PartialEq))]
    pub enum SpecialMember {
        Base,
        Vfptr,
        Bitfield(u32 /* byte_size */),
    }
}
pub use imp_special_member::SpecialMember;

mod imp_vtable_entry {
    use super::*;
    /// An entry in the virtual function table
    #[rustfmt::skip]
    #[derive(
        Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize,
        rkyv::Archive, rkyv::Serialize, rkyv::Deserialize
    )]
    #[rkyv(derive(PartialEq))]
    #[rkyv(compare(PartialEq))]
    pub struct VtableEntry {
        /// Name of the virtual function
        pub name: ArcStr,
        /// Types to make up the subroutine type
        pub function_types: Vec<Tree<Goff>>,
    }
}
pub use imp_vtable_entry::VtableEntry;

impl VtableEntry {
    pub fn is_dtor(&self) -> bool {
        self.name.starts_with('~')
    }
}
