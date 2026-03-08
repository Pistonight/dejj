use std::collections::{BTreeMap, BTreeSet};

use cu::pre::*;

use crate::{Goff, GoffMap, GoffSet, HType, SymbolInfo};

pub struct ConnectedComponent {
    pub types: Vec<Goff>,
    pub symbols: Vec<String>,
}

pub fn calc_connected_components(
    types: &GoffMap<HType>,
    symbols: &BTreeMap<String, SymbolInfo>,
) -> cu::Result<Vec<ConnectedComponent>> {
    let mut remaining_keys: GoffSet = types.keys().copied().collect();
    let mut marked = GoffSet::new();
    let mut marked_symbols = BTreeSet::new();
    let mut newly_marked = GoffSet::new();

    let mut components = vec![];
    while let Some(k) = next_non_prim_goff(&remaining_keys) {
        marked.clear();
        marked_symbols.clear();

        // mark connected components (forward direction)
        newly_marked.clear();
        newly_marked.insert(k);
        loop {
            let len_before = marked.len();
            for k in newly_marked.iter().copied().collect::<Vec<_>>() {
                let t = cu::check!(
                    types.get(&k),
                    "unexpected unconnected type goff {k} (while marking forward)"
                )?;
                t.mark(k, &mut newly_marked);
            }
            marked.extend(newly_marked.iter().copied());
            if marked.len() == len_before {
                break;
            }
            newly_marked.clear();
        }

        // mark backward directions
        // a SymbolInfo can be treated as a vertex that only has out-edges,
        // so we can also mark them here
        loop {
            let len_before = marked.len();
            let symbols_len_before = marked_symbols.len();

            for k in &remaining_keys {
                if marked.contains(k) {
                    continue;
                }
                newly_marked.clear();
                let t = cu::check!(
                    types.get(k),
                    "unexpected unconnected type goff {k} (while marking backward)"
                )?;
                let k = *k;
                t.mark(k, &mut newly_marked);
                if newly_marked.intersection(&marked).next().is_some() {
                    marked.extend(newly_marked.iter().copied());
                }
            }

            for (sym, info) in symbols {
                if marked_symbols.contains(sym) {
                    continue;
                }
                newly_marked.clear();
                info.mark(&mut newly_marked);
                if newly_marked.intersection(&marked).next().is_some() {
                    marked.extend(newly_marked.iter().copied());
                    marked_symbols.insert(sym);
                }
            }

            if marked.len() == len_before && marked_symbols.len() == symbols_len_before {
                break;
            }
        }

        // extract this component, but ignore primitives
        let new_keys = marked.iter().filter(|k| !k.is_prim()).copied().collect();
        for k in &new_keys {
            remaining_keys.remove(k);
        }
        let new_symbols = marked_symbols.iter().map(|x| x.to_string()).collect();
        components.push(ConnectedComponent {
            types: new_keys,
            symbols: new_symbols,
        });
    }

    Ok(components)
}

pub fn next_non_prim_goff(remaining: &GoffSet) -> Option<Goff> {
    remaining.iter().filter(|k| !k.is_prim()).next().copied()
}
