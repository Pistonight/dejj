use cu::pre::*;
use gimli::constants::*;

use crate::dwarf::{Die, Loff};

pub fn load_func_is_inlined<'a>(entry: &'a Die<'_, '_>) -> cu::Result<bool> {
    let offset = entry.goff();
    let has_inline_attr = cu::check!(
        entry.is_inlined(),
        "failed to check if function entry has DW_AT_inlined at {offset}"
    )?;
    if has_inline_attr {
        return Ok(true);
    }
    let abstract_origin = cu::check!(
        entry.loff_opt(DW_AT_abstract_origin),
        "failed to read abstract origin for function at {offset}"
    )?;
    if let Some(abstract_origin) = abstract_origin {
        let entry = cu::check!(
            entry.unit().entry_at(abstract_origin),
            "failed to read abstract origin entry for function at {offset}"
        )?;
        let has_inline_attr = cu::check!(
            entry.is_inlined(),
            "failed to check if function entry has DW_AT_inlined from abstract origin at {offset}"
        )?;
        if has_inline_attr {
            return Ok(true);
        }
    }
    let specification = cu::check!(
        entry.loff_opt(DW_AT_specification),
        "failed to read specification for function at {offset}"
    )?;
    if let Some(specification) = specification {
        let entry = cu::check!(
            entry.unit().entry_at(specification),
            "failed to read specification entry for function at {offset}"
        )?;
        let has_inline_attr = cu::check!(
            entry.is_inlined(),
            "failed to check if function entry has DW_AT_inlined from specification at {offset}"
        )?;
        if has_inline_attr {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn load_func_linkage_name<'a>(entry: &'a Die<'_, '_>) -> cu::Result<Option<String>> {
    let offset = entry.goff();
    let linkage_name = cu::check!(
        entry.str_opt(DW_AT_linkage_name),
        "failed to read linkage name for function at {offset}"
    )?;
    if let Some(linkage_name) = linkage_name {
        return Ok(Some(linkage_name.to_string()));
    }
    let abstract_origin = cu::check!(
        entry.loff_opt(DW_AT_abstract_origin),
        "failed to read abstract origin for function at {offset}"
    )?;
    if let Some(abstract_origin) = abstract_origin {
        let entry = cu::check!(
            entry.unit().entry_at(abstract_origin),
            "failed to read abstract origin entry for function at {offset}"
        )?;
        let name = cu::check!(
            load_func_linkage_name(&entry),
            "failed to load linkage_name from abstract origin entry, for function at {offset}"
        )?;
        if let Some(name) = name {
            return Ok(Some(name));
        }
    }
    let specification = cu::check!(
        entry.loff_opt(DW_AT_specification),
        "failed to read specification for function at {offset}"
    )?;
    if let Some(specification) = specification {
        let entry = cu::check!(
            entry.unit().entry_at(specification),
            "failed to read specification entry for function at {offset}"
        )?;
        let name = cu::check!(
            load_func_linkage_name(&entry),
            "failed to load linkage_name from specification entry, for function at {offset}"
        )?;
        if let Some(name) = name {
            return Ok(Some(name));
        }
    }
    Ok(None)
}

pub fn load_func_name<'a>(entry: &'a Die<'_, '_>) -> cu::Result<Option<String>> {
    let offset = entry.goff();
    let simple_name = cu::check!(
        entry.str_opt(DW_AT_name),
        "failed to read name for function at {offset}"
    )?;
    if let Some(simple_name) = simple_name {
        return Ok(Some(simple_name.to_string()));
    }
    let abstract_origin = cu::check!(
        entry.loff_opt(DW_AT_abstract_origin),
        "failed to read abstract origin for function at {offset}"
    )?;
    if let Some(abstract_origin) = abstract_origin {
        let entry = cu::check!(
            entry.unit().entry_at(abstract_origin),
            "failed to read abstract origin entry for function at {offset}"
        )?;
        let name = cu::check!(
            load_func_name(&entry),
            "failed to load name from abstract origin entry, for function at {offset}"
        )?;
        if let Some(name) = name {
            return Ok(Some(name));
        }
    }
    let specification = cu::check!(
        entry.loff_opt(DW_AT_specification),
        "failed to read specification for function at {offset}"
    )?;
    if let Some(specification) = specification {
        let entry = cu::check!(
            entry.unit().entry_at(specification),
            "failed to read specification entry for function at {offset}"
        )?;
        let name = cu::check!(
            load_func_name(&entry),
            "failed to load name from specification entry, for function at {offset}"
        )?;
        if let Some(name) = name {
            return Ok(Some(name));
        }
    }
    Ok(None)
}

pub fn load_func_retty(entry: &Die<'_, '_>) -> cu::Result<Option<Loff>> {
    let offset = entry.goff();
    let loff = cu::check!(
        entry.loff_opt(DW_AT_type),
        "failed to read type for function entry at {offset}"
    )?;
    if let Some(loff) = loff {
        return Ok(Some(loff));
    }
    let abstract_origin = cu::check!(
        entry.loff_opt(DW_AT_abstract_origin),
        "failed to read abstract origin for function at {offset}"
    )?;
    if let Some(abstract_origin) = abstract_origin {
        let entry = cu::check!(
            entry.unit().entry_at(abstract_origin),
            "failed to read abstract origin entry for function at {offset}"
        )?;
        let loff = cu::check!(
            load_func_retty(&entry),
            "failed to load retty from abstract origin entry, for function at {offset}"
        )?;
        if let Some(loff) = loff {
            return Ok(Some(loff));
        }
    }
    let specification = cu::check!(
        entry.loff_opt(DW_AT_specification),
        "failed to read specification for function at {offset}"
    )?;
    if let Some(specification) = specification {
        let entry = cu::check!(
            entry.unit().entry_at(specification),
            "failed to read specification entry for function at {offset}"
        )?;
        let loff = cu::check!(
            load_func_retty(&entry),
            "failed to load retty from specification entry, for function at {offset}"
        )?;
        if let Some(loff) = loff {
            return Ok(Some(loff));
        }
    }
    Ok(None)
}

pub fn load_func_param_name(entry: &Die<'_, '_>) -> cu::Result<Option<String>> {
    let offset = entry.goff();
    let name = cu::check!(
        entry.name_opt(),
        "failed to read name for function param entry at {offset}"
    )?;
    if let Some(name) = name {
        return Ok(Some(name.to_string()));
    }
    let abstract_origin = cu::check!(
        entry.loff_opt(DW_AT_abstract_origin),
        "failed to read abstract origin for function at {offset}"
    )?;
    if let Some(abstract_origin) = abstract_origin {
        let entry = cu::check!(
            entry.unit().entry_at(abstract_origin),
            "failed to read abstract origin entry for function at {offset}"
        )?;
        let name = cu::check!(
            load_func_param_name(&entry),
            "failed to load param name from abstract origin entry, for function at {offset}"
        )?;
        if let Some(name) = name {
            return Ok(Some(name));
        }
    }
    let specification = cu::check!(
        entry.loff_opt(DW_AT_specification),
        "failed to read specification for function at {offset}"
    )?;
    if let Some(specification) = specification {
        let entry = cu::check!(
            entry.unit().entry_at(specification),
            "failed to read specification entry for function at {offset}"
        )?;
        let name = cu::check!(
            load_func_param_name(&entry),
            "failed to load param name from specification entry, for function at {offset}"
        )?;
        if let Some(name) = name {
            return Ok(Some(name));
        }
    }
    Ok(None)
}

pub fn load_func_param_type(entry: &Die<'_, '_>) -> cu::Result<Option<Loff>> {
    let offset = entry.goff();
    let loff = cu::check!(
        entry.loff_opt(DW_AT_type),
        "failed to read type for function param entry at {offset}"
    )?;
    if let Some(l) = loff {
        return Ok(Some(l));
    }
    let abstract_origin = cu::check!(
        entry.loff_opt(DW_AT_abstract_origin),
        "failed to read abstract origin for function at {offset}"
    )?;
    if let Some(abstract_origin) = abstract_origin {
        let entry = cu::check!(
            entry.unit().entry_at(abstract_origin),
            "failed to read abstract origin entry for function at {offset}"
        )?;
        let loff = cu::check!(
            load_func_param_type(&entry),
            "failed to load param type from abstract origin entry, for function at {offset}"
        )?;
        if let Some(l) = loff {
            return Ok(Some(l));
        }
    }
    let specification = cu::check!(
        entry.loff_opt(DW_AT_specification),
        "failed to read specification for function at {offset}"
    )?;
    if let Some(specification) = specification {
        let entry = cu::check!(
            entry.unit().entry_at(specification),
            "failed to read specification entry for function at {offset}"
        )?;
        let loff = cu::check!(
            load_func_param_type(&entry),
            "failed to load param type from specification entry, for function at {offset}"
        )?;
        if let Some(l) = loff {
            return Ok(Some(l));
        }
    }
    Ok(None)
}
