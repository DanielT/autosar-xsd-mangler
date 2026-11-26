use super::{
    Attribute, AutosarDataTypes, CharacterDataType, Element, ElementAmount, ElementCollection,
    ElementCollectionItem, ElementDataType, XsdFileInfo, XsdRestrictToStandard,
};
use rustc_hash::FxHashMap;
use std::collections::HashSet;
use std::fs::File;
use std::io::Write;

mod attributes;
mod character_types;
mod element_definitions;
mod element_types;
mod identifier_enums;
mod perfect_hash;
mod subelements;
mod xsd_versions;

struct SubelementsInfo {
    versions_array: Vec<usize>,
    versions_index_info: FxHashMap<String, usize>,
    item_ref_array: Vec<GroupItem>,
    item_ref_info: FxHashMap<String, usize>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum GroupItem {
    ElementRef(usize),
    GroupRef(usize),
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub(crate) struct SimpleElement {
    pub(crate) name: String,
    pub(crate) typeref: String,
    pub(crate) amount: ElementAmount,
    pub(crate) splittable_ver: usize,
    pub(crate) ordered: bool,
    pub(crate) restrict_std: XsdRestrictToStandard,
    pub(crate) docstring: Option<String>,
}

struct AttributeInfo {
    attributes_array: Vec<Attribute>,
    attributes_index_info: FxHashMap<String, (usize, usize)>,
    attr_ver_index_info: FxHashMap<String, usize>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum MergedElementDataType {
    Elements {
        element_collection: ElementCollection,
        attributes: Vec<Attribute>,
        xsd_typenames: HashSet<String>,
        // mm_class: Option<String>,
    },
    Characters {
        attributes: Vec<Attribute>,
        basetype: String,
    },
    Mixed {
        element_collection: ElementCollection,
        attributes: Vec<Attribute>,
        basetype: String,
        // mm_class: Option<String>,
    },
    ElementsGroup {
        element_collection: ElementCollection,
    },
}

pub(crate) fn generate(xsd_config: &[XsdFileInfo], autosar_schema: &AutosarDataTypes) {
    create_output_dir();

    xsd_versions::generate(xsd_config);

    identifier_enums::generate(autosar_schema);

    generate_types(autosar_schema);
}

fn create_output_dir() {
    let _ = std::fs::create_dir("gen");
}

/// generate the information about element data types in specification.rs
pub(crate) fn generate_types(autosar_schema: &AutosarDataTypes) {
    let mut generated = String::from(
        r#"// This file is @generated

use crate::regex::*;
use crate::*;

macro_rules! element {
    ($etype:literal, $mult:ident, $ordered:literal, $splittable:literal, $stdrestrict:ident) => {
        ElementDefinition {
            elemtype: $etype,
            multiplicity: ElementMultiplicity::$mult,
            ordered: $ordered,
            splittable: $splittable,
            restrict_std: StdRestrict::$stdrestrict,
        }
    };
}

macro_rules! e {
    ($namepart:ident, $idx:literal) => {
        SubElement::element(ElementName::$namepart, $idx)
    };
}

macro_rules! g {
    ($idx:literal) => {
        SubElement::group($idx)
    };
}

"#,
    );

    let mut element_types =
        merge_element_groups(&autosar_schema.element_types, &autosar_schema.group_types);
    sort_bag_elements(&mut element_types);

    let element_definitions_array = element_definitions::build_info(&element_types);
    let version_masks = VersionMasks::collect(
        &element_types,
        &autosar_schema.character_types,
        &element_definitions_array,
    );

    let character_types = character_types::generate(autosar_schema, &version_masks);
    generated.push_str(&character_types);

    let docstring_ids = element_definitions::build_docstrings_info(&element_definitions_array);

    let SubelementsInfo {
        mut versions_array,
        versions_index_info,
        item_ref_array,
        item_ref_info,
    } = subelements::build_info(&element_types, &element_definitions_array);

    generated.push_str(&element_definitions::generate(
        &element_types,
        &element_definitions_array,
        &docstring_ids,
        &version_masks,
    ));

    generated.push_str(&subelements::generate(
        &item_ref_array,
        &element_definitions_array,
    ));
    generated.push_str(&format!(
        "\npub(crate) const MAX_SUBELEMENT_DEPTH: usize = {};\n",
        max_subelement_depth(&element_types)
    ));

    let AttributeInfo {
        attributes_array,
        attributes_index_info,
        attr_ver_index_info,
    } = attributes::build_info(&element_types, &mut versions_array);

    generated.push_str(&attributes::generate(autosar_schema, &attributes_array));

    generated.push_str(&version_masks.generate());
    generated.push_str(&generate_versions_array(&versions_array, &version_masks));

    generated.push_str(&element_types::generate(
        &element_types,
        &autosar_schema.character_types,
        &item_ref_info,
        &versions_index_info,
        &attributes_index_info,
        &attr_ver_index_info,
    ));

    generated.push_str(&element_definitions::generate_docstrings(&docstring_ids));

    let mut file = File::create("gen/specification.rs").unwrap();
    file.write_all(generated.as_bytes()).unwrap();
}

/// merge the group types into the element types
/// This removes one layer of indirection in the generated output and simplifies
/// the common case - with few exceptions an element type contains a list of
/// elements and no additional groups are involved
fn merge_element_groups(
    element_types: &FxHashMap<String, ElementDataType>,
    group_types: &FxHashMap<String, ElementCollection>,
) -> FxHashMap<String, MergedElementDataType> {
    let mut merged_element_types = FxHashMap::default();
    let mut needed_groups = HashSet::new();
    for (ename, etype) in element_types {
        merged_element_types.insert(
            ename.clone(),
            match etype {
                ElementDataType::Elements {
                    group_ref,
                    attributes,
                    xsd_typenames,
                    // mm_class,
                } => {
                    let mut element_collection = group_types.get(group_ref).unwrap().clone();
                    update_group_deps(&mut element_collection, &mut needed_groups);

                    MergedElementDataType::Elements {
                        element_collection,
                        attributes: attributes.clone(),
                        xsd_typenames: xsd_typenames.clone(),
                        // mm_class: mm_class.clone(),
                    }
                }
                ElementDataType::Characters {
                    attributes,
                    basetype,
                } => MergedElementDataType::Characters {
                    attributes: attributes.clone(),
                    basetype: basetype.clone(),
                },
                ElementDataType::Mixed {
                    group_ref,
                    attributes,
                    basetype,
                    // mm_class,
                } => {
                    let mut element_collection = group_types.get(group_ref).unwrap().clone();
                    update_group_deps(&mut element_collection, &mut needed_groups);

                    MergedElementDataType::Mixed {
                        element_collection,
                        attributes: attributes.clone(),
                        basetype: basetype.clone(),
                        // mm_class: mm_class.clone(),
                    }
                }
            },
        );
    }

    while !needed_groups.is_empty() {
        // get any groupname from the set
        let groupname = needed_groups.iter().next().cloned().unwrap();
        needed_groups.remove(&groupname);
        // create an elementname based on the group name; to make sure there are no collisions
        // with existing names, the names of group elements end with ":GROUP"
        let element_type_name = format!("{groupname}:GROUP");
        // get the referenced group
        let group = group_types.get(&groupname).unwrap();
        // copy the element collection; the copy is updated with the modified group names
        let mut element_collection = group.clone();
        update_group_deps(&mut element_collection, &mut needed_groups);
        let old = merged_element_types.insert(
            element_type_name,
            MergedElementDataType::ElementsGroup { element_collection },
        );
        assert!(old.is_none());
    }

    merged_element_types
}

fn update_group_deps(
    element_collection: &mut ElementCollection,
    needed_groups: &mut HashSet<String>,
) {
    match element_collection {
        ElementCollection::Choice { sub_elements, .. }
        | ElementCollection::Sequence { sub_elements, .. } => {
            for item in sub_elements {
                if let ElementCollectionItem::GroupRef(groupname) = item {
                    needed_groups.insert(groupname.clone());
                    *groupname = format!("{groupname}:GROUP");
                }
            }
        }
    }
}

/// A collection of the distinct version masks used anywhere in the specification
///
/// There are only a few dozen distinct version masks, so all tables store a u8 index into
/// the `VERSION_MASKS` table instead of the u32 mask itself.
pub(crate) struct VersionMasks {
    masks: Vec<usize>,
    index: FxHashMap<usize, usize>,
}

impl VersionMasks {
    fn collect(
        element_types: &FxHashMap<String, MergedElementDataType>,
        character_types: &FxHashMap<String, CharacterDataType>,
        element_definitions: &[SimpleElement],
    ) -> Self {
        // version 0 is used for group references in the sub element lists
        let mut all_masks: HashSet<usize> = HashSet::from([0]);
        for etype in element_types.values() {
            if let Some(ec) = etype.collection() {
                for item in ec.items() {
                    if let ElementCollectionItem::Element(elem) = item {
                        all_masks.insert(elem.version_info);
                    }
                }
            }
            all_masks.extend(etype.attributes().iter().map(|attr| attr.version_info));
        }
        for chartype in character_types.values() {
            if let CharacterDataType::Enum(enumdef) = chartype {
                all_masks.extend(enumdef.enumitems.iter().map(|(_, ver)| *ver));
            }
        }
        all_masks.extend(element_definitions.iter().map(|elem| elem.splittable_ver));

        let mut masks: Vec<usize> = all_masks.into_iter().collect();
        masks.sort_unstable();
        assert!(
            masks.len() <= 256,
            "too many distinct version masks ({}) for a u8 index",
            masks.len()
        );
        let index = masks
            .iter()
            .enumerate()
            .map(|(idx, mask)| (*mask, idx))
            .collect();
        Self { masks, index }
    }

    /// get the index of a version mask in the `VERSION_MASKS` table
    pub(crate) fn idx(&self, mask: usize) -> usize {
        *self
            .index
            .get(&mask)
            .unwrap_or_else(|| panic!("version mask 0x{mask:x} was not collected"))
    }

    fn generate(&self) -> String {
        let mut generated = format!(
            "\n#[rustfmt::skip]\npub(crate) static VERSION_MASKS: [u32; {}] = [\n",
            self.masks.len()
        );
        for chunk in self.masks.chunks(VERSION_INFO_CHUNK_SIZE) {
            let chunk_str = chunk
                .iter()
                .map(|val| format!("0x{val:06x}"))
                .collect::<Vec<String>>()
                .join(", ");
            generated.push_str(&format!("    {chunk_str},\n"));
        }
        generated.push_str("];\n");
        generated
    }
}

/// Sort the sub elements of all Bag types (`Choice` with `amount` Any) by name
///
/// The order of the elements in a bag has no meaning, and sorting them allows the
/// specification crate to use a binary search when looking up a sub element by name.
/// The sort is stable, so elements with the same name keep their relative order.
fn sort_bag_elements(element_types: &mut FxHashMap<String, MergedElementDataType>) {
    for (typename, etype) in element_types.iter_mut() {
        let collection = match etype {
            MergedElementDataType::ElementsGroup { element_collection }
            | MergedElementDataType::Elements {
                element_collection, ..
            } => element_collection,
            // Mixed content is not a bag, and Characters content has no sub elements
            MergedElementDataType::Mixed { .. } | MergedElementDataType::Characters { .. } => {
                continue;
            }
        };
        if let ElementCollection::Choice {
            sub_elements,
            amount: ElementAmount::Any,
            ..
        } = collection
        {
            // the binary search in the specification crate can't handle groups inside of a bag
            assert!(
                sub_elements
                    .iter()
                    .all(|item| matches!(item, ElementCollectionItem::Element(_))),
                "bag type {typename} contains a group"
            );
            // is_named() in the specification crate expects SHORT-NAME at position 0
            assert!(
                sub_elements.iter().all(|item| item.name() != "SHORT-NAME"),
                "bag type {typename} contains SHORT-NAME"
            );
            sub_elements.sort_by(|a, b| a.name().cmp(b.name()));
        }
    }
}

/// find the maximum length of the index list returned by `find_sub_element()`
fn max_subelement_depth(element_types: &FxHashMap<String, MergedElementDataType>) -> usize {
    fn depth(
        typename: &str,
        element_types: &FxHashMap<String, MergedElementDataType>,
        cache: &mut FxHashMap<String, usize>,
    ) -> usize {
        if let Some(d) = cache.get(typename) {
            return *d;
        }
        let mut result = 0;
        if let Some(ec) = element_types.get(typename).and_then(|et| et.collection()) {
            for item in ec.items() {
                let item_depth = match item {
                    ElementCollectionItem::Element(_) => 1,
                    ElementCollectionItem::GroupRef(group) => {
                        1 + depth(group, element_types, cache)
                    }
                };
                result = result.max(item_depth);
            }
        }
        cache.insert(typename.to_string(), result);
        result
    }

    let mut cache = FxHashMap::default();
    element_types
        .keys()
        .map(|typename| depth(typename, element_types, &mut cache))
        .max()
        .unwrap_or(0)
}

const VERSION_INFO_CHUNK_SIZE: usize = 10;
/// generate the array of version information for the elements
///
/// Each entry is an index into `VERSION_MASKS`
fn generate_versions_array(versions_array: &[usize], version_masks: &VersionMasks) -> String {
    let mut generated = format!(
        "\n#[rustfmt::skip]\npub(crate) static VERSION_INFO: [u8; {}] = [\n",
        versions_array.len()
    );
    for idx in (0..versions_array.len()).step_by(VERSION_INFO_CHUNK_SIZE) {
        let chunk = &versions_array[idx..(idx + VERSION_INFO_CHUNK_SIZE).min(versions_array.len())];
        let chunk_str = chunk
            .iter()
            .map(|val| format!("{:3}", version_masks.idx(*val)))
            .collect::<Vec<String>>()
            .join(", ");
        generated.push_str(&format!("    {},\n", chunk_str));
    }

    generated.push_str("];\n");
    generated
}

/// generate a CamelCase identifier for an enum variant in Rust from an ALL-CAPS name in the xsd
fn name_to_identifier(name: &str) -> String {
    let mut keep_capital = true;
    let mut force_capital = false;
    let mut result = String::new();
    let mut prev_is_digit = false;

    if let Some(firstchar) = name.chars().next() {
        if !firstchar.is_ascii_alphabetic() {
            result.push('_');
        }
    }

    for c in name.chars() {
        if c == ':' {
            force_capital = true;
        }
        if c == '-' {
            keep_capital = true;
        } else if c.is_ascii_alphanumeric() {
            if force_capital {
                if prev_is_digit && c.is_ascii_digit() {
                    // two digits separated by '-' should still be separated after the transformation
                    result.push('_');
                }
                result.push(c.to_ascii_uppercase());
                force_capital = false;
            } else if keep_capital {
                if prev_is_digit && c.is_ascii_digit() {
                    // two digits separated by '-' should still be separated after the transformation
                    result.push('_');
                }
                result.push(c);
                keep_capital = false;
            } else {
                result.push(c.to_ascii_lowercase());
            }
            prev_is_digit = c.is_ascii_digit();
        }
    }

    result
}

impl From<&Element> for SimpleElement {
    fn from(element: &Element) -> Self {
        Self {
            name: element.name.clone(),
            typeref: element.typeref.clone(),
            amount: element.amount,
            splittable_ver: element.splittable_ver,
            ordered: element.ordered,
            restrict_std: element.restrict_std,
            docstring: element.docstring.clone(),
        }
    }
}

impl MergedElementDataType {
    fn collection(&self) -> Option<&ElementCollection> {
        match self {
            MergedElementDataType::ElementsGroup { element_collection }
            | MergedElementDataType::Elements {
                element_collection, ..
            }
            | MergedElementDataType::Mixed {
                element_collection, ..
            } => Some(element_collection),
            MergedElementDataType::Characters { .. } => None,
        }
    }

    fn attributes(&self) -> &[Attribute] {
        match self {
            MergedElementDataType::Elements { attributes, .. }
            | MergedElementDataType::Characters { attributes, .. }
            | MergedElementDataType::Mixed { attributes, .. } => attributes,
            MergedElementDataType::ElementsGroup { .. } => &[],
        }
    }

    fn xsd_typenames(&self) -> Option<&HashSet<String>> {
        if let MergedElementDataType::Elements { xsd_typenames, .. } = self {
            Some(xsd_typenames)
        } else {
            None
        }
    }

    fn basetype(&self) -> Option<&str> {
        match self {
            MergedElementDataType::Characters { basetype, .. }
            | MergedElementDataType::Mixed { basetype, .. } => Some(basetype),
            MergedElementDataType::ElementsGroup { .. }
            | MergedElementDataType::Elements { .. } => None,
        }
    }
}
