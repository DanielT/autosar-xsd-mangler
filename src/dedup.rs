use super::{AutosarDataTypes, Element, ElementCollection, ElementCollectionItem, ElementDataType};
use rustc_hash::FxHashMap;

pub(crate) fn dedup_types(autosar_types: &mut AutosarDataTypes) {
    // deduplicate char types
    let char_replacements = find_chartype_replacements(autosar_types);
    replace_element_chartypes(autosar_types, &char_replacements);

    // remove the replaced character data types
    for name in char_replacements.keys() {
        autosar_types.character_types.remove(name);
    }

    // replace repeatedly - element and group types may become identical when types they depend on are deduplicated
    loop {
        let group_replacements = find_group_replacements(autosar_types);
        let elem_replacements = find_elemtype_replacements(autosar_types);

        // perform replacements in each group
        replace_groupitem_types(autosar_types, &elem_replacements, &group_replacements);

        // replace group type references in each element
        replace_element_grouptypes(autosar_types, &group_replacements);

        // remove obsolete group types
        for name in group_replacements.keys() {
            autosar_types.group_types.remove(name);
        }
        // remove obsolete element types
        for name in elem_replacements.keys() {
            autosar_types.element_types.remove(name);
        }

        // done if no replacements werre foud in this iteration
        if group_replacements.is_empty() && elem_replacements.is_empty() {
            break;
        }
    }
}

fn find_chartype_replacements(autosar_types: &mut AutosarDataTypes) -> FxHashMap<String, String> {
    // build a table of character types to replace by another identical type
    let mut char_replacements = FxHashMap::default();
    let mut char_typenames = autosar_types
        .character_types
        .keys()
        .cloned()
        .collect::<Vec<String>>();
    char_typenames.sort_by(dedup_keycmp);
    for idx1 in 0..(char_typenames.len() - 1) {
        let typename1 = &char_typenames[idx1];
        if char_replacements.get(typename1).is_none() {
            for typename2 in char_typenames.iter().skip(idx1 + 1) {
                if char_replacements.get(typename2).is_none()
                    && autosar_types.character_types.get(typename1)
                        == autosar_types.character_types.get(typename2)
                {
                    char_replacements.insert(typename2.to_owned(), typename1.to_owned());
                }
            }
        }
    }
    char_replacements
}

fn find_group_replacements(autosar_types: &mut AutosarDataTypes) -> FxHashMap<String, String> {
    // build a table of group types to replace by another identical type
    let mut group_replacements = FxHashMap::default();
    let mut group_typenames = autosar_types
        .group_types
        .keys()
        .cloned()
        .collect::<Vec<String>>();
    group_typenames.sort_by(dedup_keycmp);

    for idx1 in 0..(group_typenames.len() - 1) {
        let typename1 = &group_typenames[idx1];
        if group_replacements.get(typename1).is_none() {
            for typename2 in group_typenames.iter().skip(idx1 + 1) {
                if group_replacements.get(typename2).is_none()
                    && autosar_types.group_types.get(typename1)
                        == autosar_types.group_types.get(typename2)
                {
                    group_replacements.insert(typename2.to_owned(), typename1.to_owned());
                }
            }
        }
    }
    group_replacements
}

fn find_elemtype_replacements(autosar_types: &mut AutosarDataTypes) -> FxHashMap<String, String> {
    // build a table of element types to replace by another identical type
    let mut elem_replacements = FxHashMap::default();
    let mut elem_typenames = autosar_types
        .element_types
        .keys()
        .cloned()
        .collect::<Vec<String>>();
    elem_typenames.sort_by(dedup_keycmp);
    for idx1 in 0..(elem_typenames.len() - 1) {
        let typename1 = &elem_typenames[idx1];
        if elem_replacements.get(typename1).is_none() {
            for typename2 in elem_typenames.iter().skip(idx1 + 1) {
                if elem_replacements.get(typename2).is_none()
                    && autosar_types.element_types.get(typename1)
                        == autosar_types.element_types.get(typename2)
                {
                    elem_replacements.insert(typename2.to_owned(), typename1.to_owned());
                }
            }
        }
    }
    elem_replacements
}

fn replace_element_chartypes(
    autosar_types: &mut AutosarDataTypes,
    char_replacements: &FxHashMap<String, String>,
) {
    for artype in autosar_types.element_types.values_mut() {
        // replace character types for attributes
        match artype {
            ElementDataType::Elements { attributes, .. }
            | ElementDataType::Characters { attributes, .. }
            | ElementDataType::Mixed { attributes, .. } => {
                for attr in attributes {
                    if let Some(rep) = char_replacements.get(&attr.attr_type) {
                        attr.attr_type = rep.to_owned();
                    }
                }
            }
        }
        // replace character data type for character content
        match artype {
            ElementDataType::Characters { basetype, .. }
            | ElementDataType::Mixed { basetype, .. } => {
                if let Some(rep) = char_replacements.get(basetype) {
                    *basetype = rep.to_owned();
                }
            }
            ElementDataType::Elements { .. } => {}
        }
    }
}

fn replace_groupitem_types(
    autosar_types: &mut AutosarDataTypes,
    elem_replacements: &FxHashMap<String, String>,
    group_replacements: &FxHashMap<String, String>,
) {
    for group_type in autosar_types.group_types.values_mut() {
        match group_type {
            ElementCollection::Choice { sub_elements, .. }
            | ElementCollection::Sequence { sub_elements, .. } => {
                for ec_item in sub_elements {
                    match ec_item {
                        ElementCollectionItem::Element(Element {
                            typeref: element_typeref,
                            ..
                        }) => {
                            if let Some(rep) = elem_replacements.get(element_typeref) {
                                *element_typeref = rep.to_owned();
                            }
                        }
                        ElementCollectionItem::GroupRef(group_ref) => {
                            if let Some(rep) = group_replacements.get(group_ref) {
                                *group_ref = rep.to_owned();
                            }
                        }
                    }
                }
            }
        }
    }
}

fn replace_element_grouptypes(
    autosar_types: &mut AutosarDataTypes,
    group_replacements: &FxHashMap<String, String>,
) {
    for artype in autosar_types.element_types.values_mut() {
        // replace group_refs inside an element type
        match artype {
            ElementDataType::Elements { group_ref, .. }
            | ElementDataType::Mixed { group_ref, .. } => {
                if let Some(rep) = group_replacements.get(group_ref) {
                    *group_ref = rep.to_owned();
                }
            }
            ElementDataType::Characters { .. } => {}
        }
    }
}

pub(crate) fn dedup_keycmp(key1: &String, key2: &String) -> std::cmp::Ordering {
    match key1.len().cmp(&key2.len()) {
        std::cmp::Ordering::Equal => key1.cmp(key2),
        nonequal => nonequal,
    }
}
