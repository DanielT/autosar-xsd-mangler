use crate::generator::{GroupItem, MergedElementDataType, SimpleElement, SubelementsInfo};
use crate::{Element, ElementCollectionItem};
use rustc_hash::FxHashMap;

pub(crate) fn build_info(
    element_types: &FxHashMap<String, MergedElementDataType>,
    element_definitions_array: &[SimpleElement],
) -> SubelementsInfo {
    let mut versions_array = Vec::new();
    let mut versions_index_info: FxHashMap<String, usize> = FxHashMap::default();
    let mut item_ref_array: Vec<GroupItem> = vec![];
    let mut item_ref_info: FxHashMap<String, usize> = FxHashMap::default();

    let elem_idx: FxHashMap<SimpleElement, usize> = element_definitions_array
        .iter()
        .enumerate()
        .map(|(pos, elem)| (elem.clone(), pos))
        .collect();

    // sort the group type names so that the element types with the most sub elements are first
    let mut elemtypenames_bysize: Vec<&String> = element_types.keys().collect();
    elemtypenames_bysize.sort_by(|k1, k2| cmp_grouptypenames_subelems(k1, k2, element_types));
    let mut elemtypenames_alphabetical = elemtypenames_bysize.clone();
    elemtypenames_alphabetical.sort();

    // iterate over the groups according to the sorted type name order
    for elemtypename in elemtypenames_bysize {
        if let Some(element_collection) = element_types
            .get(elemtypename)
            .and_then(|et| et.collection())
        {
            let items = element_collection.items();
            if !items.is_empty() {
                // build a list of versions from the list of items
                let item_versions: Vec<usize> = items
                    .iter()
                    .map(|item| match item {
                        ElementCollectionItem::Element(Element { version_info, .. }) => {
                            *version_info
                        }
                        ElementCollectionItem::GroupRef(_) => 0,
                    })
                    .collect();
                // check if this exact sequence of version information already exists within the versions_array
                if let Some(existing_version_position) = versions_array
                    .iter()
                    .enumerate()
                    .filter(|(_, ver)| **ver == item_versions[0])
                    .map(|(pos, _)| pos)
                    .find(|pos| versions_array[*pos..].starts_with(&item_versions))
                {
                    // exact sequence was found, store the position of the existing data
                    versions_index_info.insert(elemtypename.to_owned(), existing_version_position);
                } else {
                    // the exact sequence was not found, append it to the end of versions_array and store the position
                    versions_index_info.insert(elemtypename.to_owned(), versions_array.len());
                    versions_array.extend(item_versions.iter());
                }

                // try to reuse group item lists
                let grpitems: Vec<GroupItem> = items
                    .iter()
                    .map(|item| match item {
                        ElementCollectionItem::Element(element) => GroupItem::ElementRef(
                            *elem_idx.get(&SimpleElement::from(element)).unwrap(),
                        ),

                        ElementCollectionItem::GroupRef(group_ref) => {
                            let grouptype_idx = elemtypenames_alphabetical
                                .iter()
                                .position(|name| *name == group_ref)
                                .unwrap();
                            GroupItem::GroupRef(grouptype_idx)
                        }
                    })
                    .collect();
                if let Some(existing_position) = item_ref_array
                    .iter()
                    .enumerate()
                    .filter(|(_, item)| **item == grpitems[0])
                    .map(|(pos, _)| pos)
                    .find(|pos| item_ref_array[*pos..].starts_with(&grpitems))
                {
                    item_ref_info.insert(elemtypename.clone(), existing_position);
                } else {
                    item_ref_info.insert(elemtypename.clone(), item_ref_array.len());
                    item_ref_array.extend(grpitems.iter().cloned());
                }
            } else {
                // number of subelements = 0
                versions_index_info.insert(elemtypename.to_owned(), 0);
                item_ref_info.insert(elemtypename.clone(), 0);
            }
        } else {
            versions_index_info.insert(elemtypename.to_owned(), 0);
            item_ref_info.insert(elemtypename.clone(), 0);
        }
    }

    SubelementsInfo {
        versions_array,
        versions_index_info,
        item_ref_array,
        item_ref_info,
    }
}

fn cmp_grouptypenames_subelems(
    k1: &str,
    k2: &str,
    elemtypes: &FxHashMap<String, MergedElementDataType>,
) -> std::cmp::Ordering {
    let len1 = elemtypes
        .get(k1)
        .map(|et| et.collection().map_or(0, |ec| ec.items().len()))
        .unwrap();
    let len2 = elemtypes
        .get(k2)
        .map(|et| et.collection().map_or(0, |ec| ec.items().len()))
        .unwrap();

    match len2.cmp(&len1) {
        std::cmp::Ordering::Less => std::cmp::Ordering::Less,
        std::cmp::Ordering::Equal => k1.cmp(k2),
        std::cmp::Ordering::Greater => std::cmp::Ordering::Greater,
    }
}

const SUBELEMENT_CHUNK_SIZE: usize = 25;
pub(crate) fn generate(items: &[GroupItem]) -> String {
    let mut generated = format!(
        "\n#[rustfmt::skip]\npub(crate) const SUBELEMENTS: [SubElement; {}] = [\n",
        items.len()
    );
    let mut item_strings = vec![];
    for item in items {
        item_strings.push(match item {
            GroupItem::ElementRef(idx) => format!("e!({idx})"),
            GroupItem::GroupRef(idx) => {
                format!("g!({idx})")
            }
        });
    }
    for idx in (0..item_strings.len()).step_by(SUBELEMENT_CHUNK_SIZE) {
        let upper_idx = (idx + SUBELEMENT_CHUNK_SIZE).min(item_strings.len());
        let slice_str = item_strings[idx..upper_idx].join(", ");
        generated.push_str("    ");
        generated.push_str(&slice_str);
        generated.push_str(",\n");
    }

    generated.push_str("];\n");
    generated
}
