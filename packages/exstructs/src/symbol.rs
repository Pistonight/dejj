use tyyaml::Tree;

use crate::{Goff, TemplateArg};

/// Information of a global symbol
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SymbolInfo {
    /// Address of the symbol (offset in the original binary)
    pub address: u32,
    /// Name for linking (linkage name)
    pub link_name: String,
    /// Type of the symbol. For functions, this is a Tree::Sub.
    /// Could be unflattened depending on the stage.
    pub ty: Tree<Goff>,
    /// Function parameter names, if the symbol is a function.
    /// Empty string could exists for unnamed parameters,
    /// depending on the stage.
    pub param_names: Vec<String>,
    /// Function template instantiation
    pub template_args: Vec<TemplateArg<Goff>>,
}
impl SymbolInfo {
    pub fn new_data(linkage_name: String, ty: Goff) -> Self {
        Self {
            address: 0,
            link_name: linkage_name,
            ty: Tree::Base(ty),
            // is_func: false,
            param_names: vec![],
            template_args: Default::default(),
        }
    }
    pub fn new_func(
        linkage_name: String,
        types: Vec<Tree<Goff>>,
        mut param_names: Vec<String>,
        template_args: Vec<TemplateArg<Goff>>,
    ) -> Self {
        // fill in empty param names
        let mut changes = vec![];
        for (i, name) in param_names.iter().enumerate() {
            if !name.is_empty() {
                continue;
            }
            let mut j = i;
            let mut new_name = format!("a{j}");
            while param_names.iter().any(|x| x == &new_name) {
                j += 1;
                new_name = format!("a{j}");
            }
            changes.push((i, new_name));
        }
        for (i, name) in changes {
            param_names[i] = name;
        }
        Self {
            address: 0,
            link_name: linkage_name,
            ty: Tree::Sub(types),
            param_names,
            template_args,
        }
    }

    /// Link symbol info across different CUs
    ///
    /// This does not compare type offsets, since they are different in different CUs
    pub fn link(&mut self, other: &Self) -> cu::Result<()> {
        cu::ensure!(
            self.link_name == other.link_name,
            "cannot merge symbol info with different linkage names: {} != {}",
            self.link_name,
            other.link_name
        )?;
        cu::ensure!(
            self.address == other.address,
            "cannot merge symbol info with different addresses"
        )?;
        cu::ensure!(
            self.param_names == other.param_names,
            "cannot merge symbol info with different param_names"
        )?;
        Ok(())
    }

    /// Merge a symbol in the same CU
    pub fn merge(&mut self, other: &Self) -> cu::Result<()> {
        cu::ensure!(
            self.link_name == other.link_name,
            "cannot merge symbol info with different linkage names: {} != {}",
            self.link_name,
            other.link_name
        )?;
        cu::ensure!(
            self.ty == other.ty,
            "cannot merge symbol info with different types"
        )?;
        cu::ensure!(
            self.param_names == other.param_names,
            "cannot merge symbol info with different param_names"
        )?;
        // some info does not have template args, in which case we fill it in
        match (
            self.template_args.is_empty(),
            other.template_args.is_empty(),
        ) {
            (_, true) => {}
            (true, false) => {
                self.template_args = other.template_args.clone();
            }
            (false, false) => {
                cu::ensure!(
                    self.template_args == other.template_args,
                    "cannot merge symbol info with different template_args"
                )?;
            }
        }
        Ok(())
    }
    //
    //     /// Replace occurrences of a goff anywhere referrenced in this type
    //     /// with another type tree
    //     pub fn replace(&mut self, goff: Goff, replacement: &Tree<Goff>) {
    //     }
}
