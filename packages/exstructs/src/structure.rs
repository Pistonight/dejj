use std::sync::Arc;

use tyyaml::{Prim, Tree};

use crate::{Goff, Namespace, NamespacedName, NamespacedTemplatedName, TemplateArg};

/// High-level (H) Type data
///
/// - Declarations are merged with the definitions
///   - Undefined declarations become empty struct with the name
/// - Typedef names are merged with the definitions
/// - All compile units are linked together
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HType {
    // TODO
    // /// Pritimive type
    // Prim(Prim),
    // /// Enum + typedef names. The name does not include template args. could be anonymous
    // Enum(Option<NamespacedName>, Type1Enum, Vec<NamespacedTemplatedName>),
    // /// Union + typedef names. The name does not include template args. could be anonymous
    // Union(Option<NamespacedName>, Type0Union, Vec<NamespacedTemplatedName>),
    // /// Struct + typedef names. The name does not include template args. could be anonymous
    // Struct(Option<NamespacedName>, Type0Struct, Vec<NamespacedTemplatedName>),
}

// pub struct HTypeData<T> {
// }

/// Mid-level (M) Type data
///
/// This is the abstraction used for merging and optimizing types
///
/// - Aliases are merged & eliminated
/// - Trees are flattened: A Tree::Base definitely points to a
///   primitive, enum, union or struct.
/// - Typedefs to composite types are eliminated
/// - Other typedefs have their names merged into the target
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MTypeData<T> {
    /// The name; does not include template args; None means anonymous
    pub name: Option<NamespacedName>,
    /// Declared aliases (e.g from typedefs)
    pub decl_names: Vec<NamespacedTemplatedName>,
    /// The data
    pub data: T,
}

/// Declaration data of an MType
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MTypeDecl {
    /// Primary name, includes template args
    pub name: NamespacedTemplatedName,
    /// Other names from typedef
    pub typedef_names: Vec<NamespacedTemplatedName>,
}

/// Low-level (L) Type data
///
/// This mostly maps to raw data parsed from DWARF
///
/// - Trees are not flattened: for example, A Tree::Base could be pointing to a Goff
///   that is a pointer type.
/// - Templates are not parsed: Declarations and typedefs could have templates embedded in the name
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LTypeData<T> {
    /// The name; does not include template args; None means anonymous
    pub name: Option<NamespacedName>,
    /// The data
    pub data: T,
}

/// Declaration data of a LType
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LTypeDecl {
    /// The enclosing namespace that is required to resolve the names
    /// in the template args
    pub enclosing: Namespace,
    /// The name; could include template args
    pub name_with_tpl: NamespacedName,
}

/// Data of an `enum`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Enum {
    /// Base type, used to determine the size
    pub byte_size: u32,
    /// Enumerators of the enum, in the order they appear in DWARF
    pub enumerators: Vec<Enumerator>,
}

/// An enum with size not-yet fully determined (could be linked to another type)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnumUndeterminedSize {
    /// Base type, used to determine the size
    pub byte_size_or_base: Result<u32, Goff>,
    /// Enumerators of the enum, in the order they appear in DWARF
    pub enumerators: Vec<Enumerator>,
}

/// Data of a `union`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Union {
    /// Template arguments, if any
    pub template_args: Vec<TemplateArg<Goff>>,
    /// Byte size of the union (should be size of the largest member)
    pub byte_size: u32,
    /// Union members. The members must have offset of 0 and special of None
    pub members: Vec<Member>,
}

/// Data of a `struct` or `class`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Struct {
    /// Template specialization of the struct, if any
    pub template_args: Vec<TemplateArg<Goff>>,
    /// Byte size of the struct
    pub byte_size: u32,
    /// Vtable of the struct. (index, entry).
    /// Dtors will have an index of 0
    pub vtable: Vec<(usize, VtableEntry)>,
    /// Members of the struct
    pub members: Vec<Member>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Enumerator {
    /// Name of the enumerator
    pub name: Arc<str>,
    /// Value of the enumerator. If the enumerator is unsigned
    /// and the value is greater than `i64::MAX`, then it's stored
    /// as if it's a `u64`. Enum type of byte size greater than 8
    /// is not allowed right now
    pub value: i64,
}

/// A struct or union member
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Member {
    /// Offset of the member within the struct. 0 For union.
    pub offset: u32,
    /// Name of the member. Could be None for anonymous typed member
    pub name: Option<Arc<str>>,
    /// Type of the member. Might be unflattened, depending on the stage
    pub ty: Tree<Goff>,
    /// Special-case member, None for union
    pub special: Option<SpecialMember>,
}
impl Member {
    pub fn is_base(&self) -> bool {
        matches!(self.special, Some(SpecialMember::Base))
    }
}

/// Special member type
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SpecialMember {
    Base,
    Vfptr,
    Bitfield(u32 /* byte_size */),
}

/// An entry in the virtual function table
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VtableEntry {
    /// Name of the virtual function
    pub name: Arc<str>,
    /// Types to make up the subroutine type
    pub function_types: Vec<Tree<Goff>>,
}

impl VtableEntry {
    pub fn is_dtor(&self) -> bool {
        self.name.starts_with('~')
    }
}
