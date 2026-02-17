use cu::pre::*;
use exstructs::algorithm;
use exstructs::{Enum, GoffBuckets, GoffMap, GoffSet, LType, MType, MTypeData, MTypeDecl};
use llvmutils::{CompileCommand, NameParser};

use crate::stages::{LStage, MStage};

mod clean_typedefs;
mod flatten_trees;
mod resolve_enum_sizes;

pub async fn to_mstage(mut stage: LStage, command: CompileCommand) -> cu::Result<MStage> {
    cu::check!(
        resolve_enum_sizes::run(&mut stage),
        "stage1: resolve_enum_sizes failed"
    )?;
    cu::check!(
        clean_typedefs::run(&mut stage),
        "stage1: clean_typedefs failed"
    )?;
    cu::check!(
        flatten_trees::run(&mut stage),
        "stage1: flatten_trees failed"
    )?;

    let name_parser = NameParser {
        output_dir: stage.config.paths.extract_output.join("clang-type-parse"),
        system_header_paths: stage.config.paths.system_header_paths.clone(),
        char_repr: stage.config.extract.char_repr,
        wchar_repr: stage.config.extract.wchar_repr,
    };
    let mut names = cu::check!(
        name_parser.parse(command, &stage.ns, &stage.types).await,
        "stage1: name parse failed"
    )?;

    // GC types to ensure trees are all GC-ed
    // note GC must be after parsing names, since some types could be referenced
    // only in namespaces, and we only get it after parsing the string type name
    let mut marked = GoffSet::default();
    for symbol in stage.symbols.values() {
        symbol.mark(&mut marked);
    }
    // also mark the parsed name
    for name in names.values() {
        name.mark(&mut marked);
    }
    algorithm::mark_and_sweep(marked, &mut stage.types, |t, k, marked| {
        t.mark(k, marked);
    });
    for (k, t) in &stage.types {
        if let LType::Tree(t) = t {
            cu::bail!("unexpected tree type not gc'ed: k={k}, type={t:#?}");
        }
    }

    // convert to MStage
    let mut types = GoffMap::default();
    let mut typedef_names = GoffMap::<Vec<_>>::default();
    let mut dupes = vec![];
    for (k, t) in &stage.types {
        match t {
            LType::Prim(prim) => {
                types.insert(*k, MType::Prim(*prim));
            }
            LType::Typedef { target: goff, .. } => {
                let mut target_goff = *goff;
                loop {
                    match stage.types.get(&target_goff).unwrap() {
                        LType::Typedef { target: x, .. } => {
                            target_goff = *x;
                        }
                        _ => break,
                    }
                }
                dupes.push((*k, target_goff));
                match names.remove(&k) {
                    Some(name) => {
                        typedef_names.entry(target_goff).or_default().push(name);
                    }
                    None => {
                        // cannot resolve the name: this means this typedef
                        // might be a private one (`using` inside a class or function).
                        // So in this case, we ignore the name
                    }
                }
            }
            LType::EnumDecl(_) => {
                let name = cu::check!(
                    names.remove(k),
                    "was not able to resolve enum decl name for {k}"
                )?;
                types.insert(
                    *k,
                    MType::EnumDecl(MTypeDecl {
                        name,
                        typedef_names: vec![],
                    }),
                );
            }
            LType::Enum(data) => {
                let Ok(byte_size) = data.data.byte_size_or_base else {
                    cu::bail!("unexpected did not resolve enum byte size: {k}");
                };
                let enumerators = data.data.enumerators.clone();
                types.insert(
                    *k,
                    MType::Enum(MTypeData {
                        name: data.name.clone(),
                        data: Enum {
                            byte_size,
                            enumerators,
                        },
                        decl_names: vec![],
                    }),
                );
            }
            LType::UnionDecl(_) => {
                let name = cu::check!(
                    names.remove(&k),
                    "was not able to resolve union decl name for {k}"
                )?;
                types.insert(
                    *k,
                    MType::UnionDecl(MTypeDecl {
                        name,
                        typedef_names: vec![],
                    }),
                );
            }
            LType::Union(data) => {
                types.insert(
                    *k,
                    MType::Union(MTypeData {
                        name: data.name.clone(),
                        data: data.data.clone(),
                        decl_names: vec![],
                    }),
                );
            }
            LType::StructDecl(_) => {
                let name = cu::check!(
                    names.remove(k),
                    "was not able to resolve struct decl name for {k}"
                )?;
                types.insert(
                    *k,
                    MType::StructDecl(MTypeDecl {
                        name,
                        typedef_names: vec![],
                    }),
                );
            }
            LType::Struct(data) => {
                types.insert(
                    *k,
                    MType::Struct(MTypeData {
                        name: data.name.clone(),
                        data: data.data.clone(),
                        decl_names: vec![],
                    }),
                );
            }
            LType::Tree(tree) => {
                cu::bail!("unexpected leftover tree after stage1: {k}: {tree:#?}")
            }
            LType::Alias(goff) => {
                cu::bail!("unexpected leftover alias after stage1: {k} -> {goff}")
            }
        }
    }

    // fill in typedef/decl names
    for (k, names) in typedef_names {
        match types.get_mut(&k).unwrap() {
            MType::Prim(_) => {
                cu::bail!("unexpected leftover typedef to prim: {k}");
            }
            MType::Enum(MTypeData { decl_names, .. })
            | MType::Union(MTypeData { decl_names, .. })
            | MType::Struct(MTypeData { decl_names, .. }) => *decl_names = names,
            MType::EnumDecl(MTypeDecl { typedef_names, .. })
            | MType::UnionDecl(MTypeDecl { typedef_names, .. })
            | MType::StructDecl(MTypeDecl { typedef_names, .. }) => *typedef_names = names,
        }
    }
    for (k, g) in dupes {
        types.insert(k, types.get(&g).unwrap().clone());
    }
    let deduped = algorithm::dedupe(
        types,
        GoffBuckets::default(),
        &mut stage.symbols,
        None,
        |data, buckets| data.map_goff(|k| Ok(buckets.primary_fallback(k))),
    );
    let deduped = cu::check!(deduped, "stage1: final deduped failed")?;

    Ok(MStage {
        offset: stage.offset,
        name: stage.name,
        types: deduped,
        config: stage.config,
        symbols: stage.symbols,
    })
}
