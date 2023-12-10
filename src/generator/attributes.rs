use crate::generator::{name_to_identifier, AttributeInfo, FxHashMap, MergedElementDataType};
use crate::{Attribute, AutosarDataTypes};

pub(crate) fn build_info(
    element_types: &FxHashMap<String, MergedElementDataType>,
    versions_array: &mut Vec<usize>,
) -> AttributeInfo {
    let mut elemtypenames: Vec<&String> = element_types.keys().collect();
    let mut attributes_array = Vec::new();
    let mut attributes_index_info = FxHashMap::default();
    let mut attr_ver_index_info = FxHashMap::default();

    // sort the element type names so that the element types with the most sub elements are first
    elemtypenames.sort_by(|k1, k2| cmp_elemtypenames_attrs(k1, k2, element_types));
    for etypename in elemtypenames {
        if let Some(attrs) = element_types
            .get(etypename)
            .map(MergedElementDataType::attributes)
        {
            if !attrs.is_empty() {
                // build a list of versions from the list of items
                let attr_versions: Vec<usize> =
                    attrs.iter().map(|attr| attr.version_info).collect();
                // check if this exact sequence of version information already exists within the versions_array
                if let Some(existing_version_position) = versions_array
                    .iter()
                    .enumerate()
                    .filter(|(_, ver)| **ver == attr_versions[0])
                    .map(|(pos, _)| pos)
                    .find(|pos| versions_array[*pos..].starts_with(&attr_versions))
                {
                    // exact sequence was found, store the position of the existing data
                    attr_ver_index_info.insert(etypename.to_owned(), existing_version_position);
                } else {
                    // the exact sequence was not found, append it to the end of versions_array and store the position
                    attr_ver_index_info.insert(etypename.to_owned(), versions_array.len());
                    versions_array.extend(attr_versions.iter());
                }

                // create a copy of the items and strip the version_info from the copied items
                // the version info is handled separately and this makes it more likely that identical sequences can be found
                let mut attrs_copy = attrs.to_owned();
                attrs_copy.iter_mut().for_each(|attr| attr.version_info = 0);
                // as for the versions above, try to find the exact sequene of items in the overall list of subelements
                if let Some(existing_position) = attributes_array
                    .iter()
                    .enumerate()
                    .filter(|(_, ec)| *ec == &attrs_copy[0])
                    .map(|(pos, _)| pos)
                    .find(|pos| attributes_array[*pos..].starts_with(&attrs_copy))
                {
                    attributes_index_info.insert(
                        etypename.to_owned(),
                        (existing_position, existing_position + attrs_copy.len()),
                    );
                } else {
                    attributes_index_info.insert(
                        etypename.to_owned(),
                        (
                            attributes_array.len(),
                            attributes_array.len() + attrs_copy.len(),
                        ),
                    );
                    attributes_array.append(&mut attrs_copy);
                }
            } else {
                attributes_index_info.insert(etypename.to_owned(), (0, 0));
                attr_ver_index_info.insert(etypename.to_owned(), 0);
            }
        } else {
            attributes_index_info.insert(etypename.to_owned(), (0, 0));
            attr_ver_index_info.insert(etypename.to_owned(), 0);
        }
    }

    AttributeInfo {
        attributes_array,
        attributes_index_info,
        attr_ver_index_info,
    }
}

fn cmp_elemtypenames_attrs(
    k1: &str,
    k2: &str,
    elemtypes: &FxHashMap<String, MergedElementDataType>,
) -> std::cmp::Ordering {
    let len1 = elemtypes
        .get(k1)
        .map(MergedElementDataType::attributes)
        .map_or(0, <[Attribute]>::len);
    let len2 = elemtypes
        .get(k2)
        .map(MergedElementDataType::attributes)
        .map_or(0, <[Attribute]>::len);

    match len2.cmp(&len1) {
        std::cmp::Ordering::Less => std::cmp::Ordering::Less,
        std::cmp::Ordering::Equal => k1.cmp(k2),
        std::cmp::Ordering::Greater => std::cmp::Ordering::Greater,
    }
}

pub(crate) fn generate(
    autosar_schema: &AutosarDataTypes,
    attributes_array: &[Attribute],
) -> String {
    let mut chartypenames: Vec<&String> = autosar_schema.character_types.keys().collect();
    chartypenames.sort();
    // map each character type name to an index
    let chartype_nameidx: FxHashMap<&str, usize> = chartypenames
        .iter()
        .enumerate()
        .map(|(idx, name)| (&***name, idx))
        .collect();
    let mut generated = format!(
        "\npub(crate) const ATTRIBUTES: [(AttributeName, u16, bool); {}] = [\n",
        attributes_array.len()
    );
    generated.push_str(&build_attributes_string(
        attributes_array,
        &chartype_nameidx,
    ));
    generated.push_str("\n];\n");

    generated
}

fn build_attributes_string(
    attrs: &[Attribute],
    chartype_nameidx: &FxHashMap<&str, usize>,
) -> String {
    let mut attr_strings = Vec::new();
    for attr in attrs {
        let chartype = format!("{}", *chartype_nameidx.get(&*attr.attr_type).unwrap());

        attr_strings.push(format!(
            "    (AttributeName::{}, {chartype}, {})",
            name_to_identifier(&attr.name),
            attr.required,
        ));
    }
    attr_strings.join(",\n")
}
