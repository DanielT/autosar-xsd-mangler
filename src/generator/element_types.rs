use crate::generator::{name_to_identifier, MergedElementDataType};
use crate::{CharacterDataType, Element, ElementAmount, ElementCollection, ElementCollectionItem};
use rustc_hash::FxHashMap;
use std::collections::HashSet;

pub(crate) fn generate(
    element_types: &FxHashMap<String, MergedElementDataType>,
    character_types: &FxHashMap<String, CharacterDataType>,
    subelements_index_info: &FxHashMap<String, usize>,
    subelements_ver_index_info: &FxHashMap<String, usize>,
    attributes_index_info: &FxHashMap<String, (usize, usize)>,
    attr_ver_index_info: &FxHashMap<String, usize>,
) -> String {
    let mut generated = String::new();
    let mut elemtypes = String::new();
    let mut all_refstrings = Vec::<String>::new();

    let mut elemtypenames: Vec<&String> = element_types.keys().collect();
    elemtypenames.sort();
    let mut chartypenames: Vec<&String> = character_types.keys().collect();
    chartypenames.sort();

    let ref_attribute_types = find_ref_attribute_types(element_types, character_types);

    // map each character type name to an index
    let chartype_nameidx: FxHashMap<&str, usize> = chartypenames
        .iter()
        .enumerate()
        .map(|(idx, name)| (&***name, idx))
        .collect();
    // map from element definition string to the variable name emitted for that definition
    let mut element_definitions: FxHashMap<String, String> = FxHashMap::default();
    // map each attribute definition string to the variable name emitted for that definition
    let mut attribute_definitions: FxHashMap<String, String> = FxHashMap::default();

    // empty element list and empty attribute list don't need a named variable, so are treated specially here
    element_definitions.insert(String::new(), "[]".to_string());
    attribute_definitions.insert(String::new(), "[]".to_string());

    // build a mapping from type names to elements which use that type
    let element_names_of_typename = build_elementnames_of_type_list(element_types);

    elemtypes.push_str(&format!(
        "\n#[rustfmt::skip]\npub(crate) const DATATYPES: [ElementSpec; {}] = [\n",
        element_types.len()
    ));
    for (idx, etypename) in elemtypenames.iter().enumerate() {
        let elemtype = element_types.get(*etypename).unwrap();
        let mode = calc_element_mode(elemtype);

        let subelem_limit_low = *subelements_index_info.get(*etypename).unwrap();
        let subelem_limit_high =
            subelem_limit_low + elemtype.collection().map_or(0, |ec| ec.items().len());
        let subelement_ver_info_low = subelements_ver_index_info.get(*etypename).unwrap();
        let (attrs_limit_low, attrs_limit_high) = attributes_index_info.get(*etypename).unwrap();
        let attrs_ver_info_low = attr_ver_index_info.get(*etypename).unwrap();
        let chartype = if let Some(name) = elemtype.basetype() {
            format!("Some({})", *chartype_nameidx.get(name).unwrap())
        } else {
            "None".to_string()
        };
        let infostring = if let Some(elems) = element_names_of_typename.get(*etypename) {
            let mut elemlist: Vec<String> = elems.iter().cloned().collect();
            elemlist.sort();
            elemlist.join(", ")
        } else {
            "(sub-group)".to_owned()
        };

        let (ref_info_low, ref_info_high) = if let Some(xsd_typenames) = elemtype.xsd_typenames() {
            let mut namevec: Vec<String> = xsd_typenames
                .iter()
                .filter_map(|xtn| {
                    ref_attribute_types
                        .get(xtn)
                        .map(|name| format!("EnumItem::{}", name_to_identifier(name)))
                })
                .collect();
            namevec.sort();

            if namevec.is_empty() {
                (0, 0)
            } else if let Some(existing_pos) = all_refstrings
                .iter()
                .enumerate()
                .filter(|(_, item)| **item == namevec[0])
                .map(|(pos, _)| pos)
                .find(|pos| all_refstrings[*pos..].starts_with(&namevec))
            {
                (existing_pos, existing_pos + namevec.len())
            } else {
                let len = namevec.len();
                let pos = all_refstrings.len();
                all_refstrings.append(&mut namevec);
                (pos, pos + len)
            }
        } else {
            (0, 0)
        };

        elemtypes.push_str(&format!(
            "    /* {idx:4} */ ElementSpec {{sub_elements: ({subelem_limit_low}, {subelem_limit_high}), \
                            sub_element_ver: {subelement_ver_info_low}, \
                            attributes: ({attrs_limit_low}, {attrs_limit_high}), attributes_ver: {attrs_ver_info_low}, \
                            character_data: {chartype}, mode: {mode}, ref_info: ({ref_info_low}, {ref_info_high})}}, // {infostring}\n"));
    }
    elemtypes.push_str("];\n");

    elemtypes.push_str("\n#[rustfmt::skip]");
    elemtypes.push_str(&format!(
        "\npub(crate) const REF_ITEMS: [EnumItem; {}] = [\n    {},\n];\n",
        all_refstrings.len(),
        all_refstrings.join(",\n    ")
    ));

    generated.push_str(&elemtypes);

    generated
}

/// collect the enum items of DEST attributes of all elements
fn find_ref_attribute_types(
    element_types: &FxHashMap<String, MergedElementDataType>,
    character_types: &FxHashMap<String, CharacterDataType>,
) -> HashSet<String> {
    let ref_attribute_types: HashSet<String> = element_types
        .iter() // iterate over all element types
        .filter_map(|(_, et)| {
            // filtering to get only those which have a DEST attribute
            et.attributes()
                .iter()
                .find(|attr| attr.name == "DEST")
                .map(|attr| &attr.attr_type) // map to provide only the attribute_type string
                .and_then(|attrtype| {
                    // with the attribute type string, get the CharacterDataType of the attribute from the schema
                    character_types.get(attrtype).and_then(|ctype| {
                        // extract the enum items array from the CharacterDataType
                        if let CharacterDataType::Enum(items) = ctype {
                            Some(&items.enumitems)
                        } else {
                            None
                        }
                    })
                })
        })
        .flatten() // flatten the two-level iterator
        .map(|(name, _)| name.to_owned())
        .collect();
    ref_attribute_types
}

fn build_elementnames_of_type_list(
    element_types: &FxHashMap<String, MergedElementDataType>,
) -> FxHashMap<String, HashSet<String>> {
    let mut map = FxHashMap::default();
    map.reserve(element_types.len());

    map.insert("AR:AUTOSAR".to_string(), HashSet::new());
    map.get_mut("AR:AUTOSAR")
        .unwrap()
        .insert("AUTOSAR".to_string());

    for definition in element_types.values() {
        if let Some(ec) = definition.collection() {
            for item in ec.items() {
                if let ElementCollectionItem::Element(Element { name, typeref, .. }) = item {
                    if let Some(entry) = map.get_mut(typeref) {
                        entry.insert(name.to_string());
                    } else {
                        map.insert(typeref.to_string(), HashSet::new());
                        map.get_mut(typeref).unwrap().insert(name.to_string());
                    }
                }
            }
        }
    }
    map
}

fn calc_element_mode(elemtype: &MergedElementDataType) -> &'static str {
    match elemtype {
        MergedElementDataType::ElementsGroup { element_collection }
        | MergedElementDataType::Elements {
            element_collection, ..
        } => {
            if let ElementCollection::Choice { amount, .. } = element_collection {
                if let ElementAmount::Any = amount {
                    "ContentMode::Bag"
                } else {
                    "ContentMode::Choice"
                }
            } else {
                "ContentMode::Sequence"
            }
        }
        MergedElementDataType::Characters { .. } => "ContentMode::Characters",
        MergedElementDataType::Mixed { .. } => "ContentMode::Mixed",
    }
}
