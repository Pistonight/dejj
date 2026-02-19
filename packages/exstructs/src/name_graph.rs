use std::collections::{BTreeMap, BTreeSet};

use crate::FullQualName;

#[derive(Default)]
pub struct NameGraph {
    names: Vec<FullQualName>,
    indices: BTreeMap<FullQualName, usize>,
    is_derived: Edges,
}

impl NameGraph {
    /// add a relationship where derived is a strictly derived name of base
    pub fn add_derived(&mut self, derived: &FullQualName, base: &FullQualName) -> cu::Result<bool> {
        if derived == base {
            return Ok(false);
        }
        let derived_index = self.to_index(derived);
        let base_index = self.to_index(base);
        if self.is_derived.contains((base_index, derived_index)) {
            cu::bail!("edge in the opposite direction exists");
        }
        let changed = self.is_derived.insert((derived_index, base_index));
        Ok(changed)
    }
    fn to_index(&mut self, name: &FullQualName) -> usize {
        match self.indices.get(name) {
            Some(i) => *i,
            None => {
                let i = self.names.len();
                self.names.push(name.clone());
                self.indices.insert(name.clone(), i);
                i
            }
        }
    }
}

#[derive(Default)]
pub struct Edges(BTreeMap<usize, BTreeSet<usize>>);
impl Edges {
    /// Check if an edge is in the graph
    pub fn contains(&self, edge: (usize, usize)) -> bool {
        let Some(targets) = self.0.get(&edge.0) else {
            return false;
        };
        targets.contains(&edge.1)
    }

    /// Add an edge to the graph
    pub fn insert(&mut self, edge: (usize, usize)) -> bool {
        self.0.entry(edge.0).or_default().insert(edge.1)
    }
}
