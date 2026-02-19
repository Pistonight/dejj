use std::collections::BTreeMap;
use std::sync::Arc;

use cu::pre::*;

use dejj_utils::Config;
use exstructs::{GoffMap, HType, LType, MType, NameGraph, NamespaceMaps, SizeMap, SymbolInfo};

#[derive(Default)]
pub struct StageInfo {
    stage_num: usize,
    enum_count: usize,
    enum_decl_count: usize,
    union_count: usize,
    union_decl_count: usize,
    struct_count: usize,
    struct_decl_count: usize,
    other_count: usize,
    data_count: usize,
    func_count: usize,
}

impl StageInfo {
    pub fn new(stage_num: usize) -> Self {
        Self {
            stage_num,
            ..Default::default()
        }
    }
    pub fn hstage3(stage: &HStage) -> Self {
        let mut s = Self::new(3);
        s.add_hstage(stage);
        s
    }
    pub fn mstage2(stage: &MStage) -> Self {
        let mut s = Self::new(2);
        s.add_mstage(stage);
        s
    }
    pub fn add_hstage(&mut self, stage: &HStage) {
        for t in stage.types.values() {
            match t {
                HType::Prim(_) => {}
                HType::Enum(_) => self.enum_count += 1,
                HType::Union(_) => self.union_count += 1,
                HType::Struct(_) => self.struct_count += 1,
            }
        }
        self.add_symbols(&stage.symbols)
    }
    pub fn add_mstage(&mut self, stage: &MStage) {
        for t in stage.types.values() {
            match t {
                MType::Prim(_) => {}
                MType::Enum(_) => self.enum_count += 1,
                MType::EnumDecl(_) => self.enum_decl_count += 1,
                MType::Union(_) => self.union_count += 1,
                MType::UnionDecl(_) => self.union_decl_count += 1,
                MType::Struct(_) => self.struct_count += 1,
                MType::StructDecl(_) => self.struct_decl_count += 1,
            }
        }
        self.add_symbols(&stage.symbols)
    }
    pub fn add_lstage(&mut self, stage: &LStage) {
        for t in stage.types.values() {
            match t {
                LType::Prim(_) => {}
                LType::Enum(_) => self.enum_count += 1,
                LType::EnumDecl(_) => self.enum_decl_count += 1,
                LType::Union(_) => self.union_count += 1,
                LType::UnionDecl(_) => self.union_decl_count += 1,
                LType::Struct(_) => self.struct_count += 1,
                LType::StructDecl(_) => self.struct_decl_count += 1,
                LType::Typedef { .. } | LType::Tree(_) | LType::Alias(_) => self.other_count += 1,
            }
        }
        self.add_symbols(&stage.symbols)
    }

    fn add_symbols(&mut self, symbols: &BTreeMap<String, SymbolInfo>) {
        for si in symbols.values() {
            if si.is_data() {
                self.data_count += 1;
            } else {
                self.func_count += 1;
            }
        }
    }

    pub fn print(&self) {
        use std::fmt::Write as _;
        let mut output = String::new();
        let _ = writeln!(output, "=== Stage {} ===", self.stage_num);

        let total = self.enum_count
            + self.enum_decl_count
            + self.union_count
            + self.union_decl_count
            + self.struct_count
            + self.struct_decl_count
            + self.other_count;

        let digits1 = ([
            self.enum_count,
            self.union_count,
            self.struct_count,
            self.func_count,
            self.other_count,
            total,
        ]
        .into_iter()
        .max()
        .unwrap() as f64
            + 0.1)
            .log10() as usize
            + 1;

        let digits2 = ([
            self.enum_decl_count,
            self.union_decl_count,
            self.struct_decl_count,
            self.data_count,
        ]
        .into_iter()
        .max()
        .unwrap() as f64
            + 0.1)
            .log10() as usize
            + 1;

        if self.struct_decl_count == 0 {
            let _ = writeln!(output, " Structs: {:>digits1$} defns", self.struct_count);
        } else {
            let _ = writeln!(
                output,
                " Structs: {:>digits1$} defns and {:>digits2$} decls",
                self.struct_count, self.struct_decl_count
            );
        }
        if self.enum_decl_count == 0 {
            let _ = writeln!(output, "   Enums: {:>digits1$} defns", self.enum_count);
        } else {
            let _ = writeln!(
                output,
                "   Enums: {:>digits1$} defns and {:>digits2$} decls",
                self.enum_count, self.enum_decl_count
            );
        }
        if self.union_decl_count == 0 {
            let _ = writeln!(output, "  Unions: {:>digits1$} defns", self.union_count);
        } else {
            let _ = writeln!(
                output,
                "  Unions: {:>digits1$} defns and {:>digits2$} decls",
                self.union_count, self.union_decl_count
            );
        }
        if self.other_count > 0 {
            let _ = writeln!(
                output,
                "   Other: {:>digits1$} other relations",
                self.other_count
            );
        }
        let _ = writeln!(output, "   Total: {total:>digits1$} types");
        let _ = writeln!(
            output,
            " Symbols: {:>digits1$} funcs and {:>digits2$} data",
            self.func_count, self.data_count
        );

        cu::print!("{output}");
    }
}

/// High-level (H) type stage
pub struct HStage {
    pub types: GoffMap<HType>,
    pub config: Arc<Config>,
    pub symbols: BTreeMap<String, SymbolInfo>,
    /// Size of each type, cached for convenience
    pub sizes: SizeMap,
    /// Relationship of the names
    pub name_graph: NameGraph,
}

/// Mid-level (M) type stage
pub struct MStage {
    pub offset: usize,
    pub name: String,
    pub types: GoffMap<MType>,
    pub config: Arc<Config>,
    pub symbols: BTreeMap<String, SymbolInfo>,
}

impl MStage {
    /// Link 2 stages together to become 1 stage
    pub fn link(mut self, other: Self) -> cu::Result<Self> {
        self.types.extend(other.types);
        for s in other.symbols.into_values() {
            if let Some(symbol) = self.symbols.get_mut(&s.link_name) {
                cu::check!(
                    symbol.link(&s),
                    "failed to link symbol across CU: {}",
                    other.name
                )?;
            } else {
                self.symbols.insert(s.link_name.to_string(), s);
            }
        }
        Ok(Self {
            offset: 0,
            name: String::new(),
            types: self.types,
            config: self.config,
            symbols: self.symbols,
        })
    }
}

/// Low-level (L) type stage
pub struct LStage {
    pub offset: usize,
    pub name: String,
    pub types: GoffMap<LType>,
    pub config: Arc<Config>,
    pub ns: NamespaceMaps,
    pub symbols: BTreeMap<String, SymbolInfo>,
}
