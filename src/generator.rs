use super::{
    Attribute, AutosarDataTypes, Element, ElementAmount, ElementCollection, ElementCollectionItem,
    ElementDataType, XsdFileInfo, XsdRestrictToStandard,
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

use crate::*;
use crate::regex::*;

#[cfg(feature = "docstrings")]
macro_rules! element {
    ($namepart:ident, $etype:literal, $mult:ident, $ordered:literal, $splittable:literal, $stdrestrict:ident, $docid:expr) => {
        ElementDefinition{name: ElementName::$namepart, elemtype: $etype, multiplicity: ElementMultiplicity::$mult, ordered: $ordered, splittable: $splittable, restrict_std: StdRestrict::$stdrestrict, docstring: $docid}
    };
}
#[cfg(not(feature = "docstrings"))]
macro_rules! element {
    ($namepart:ident, $etype:literal, $mult:ident, $ordered:literal, $splittable:literal, $stdrestrict:ident, $docid:expr) => {
        ElementDefinition{name: ElementName::$namepart, elemtype: $etype, multiplicity: ElementMultiplicity::$mult, ordered: $ordered, splittable: $splittable, restrict_std: StdRestrict::$stdrestrict}
    };
}

macro_rules! e {
    ($idx:literal) => {
        SubElement::Element($idx)
    };
}

macro_rules! g {
    ($idx:literal) => {
        SubElement::Group($idx)
    };
}



"#,
    );

    let element_types =
        merge_element_groups(&autosar_schema.element_types, &autosar_schema.group_types);

    let character_types = character_types::generate(autosar_schema);
    generated.push_str(&character_types);

    let element_definitions_array = element_definitions::build_info(&element_types);
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
    ));

    generated.push_str(&subelements::generate(&item_ref_array));

    let AttributeInfo {
        attributes_array,
        attributes_index_info,
        attr_ver_index_info,
    } = attributes::build_info(&element_types, &mut versions_array);

    generated.push_str(&attributes::generate(autosar_schema, &attributes_array));

    generated.push_str(&generate_versions_array(&versions_array));

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

fn generate_versions_array(versions_array: &[usize]) -> String {
    let mut generated = format!(
        "\npub(crate) static VERSION_INFO: [u32; {}] = [",
        versions_array.len()
    );
    let ver_str = versions_array
        .iter()
        .map(|val| format!("0x{val:x}"))
        .collect::<Vec<String>>()
        .join(", ");
    generated.push_str(&ver_str);
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
