use super::{
    Attribute, AutosarDataTypes, CharacterDataType, Element, ElementAmount, ElementCollection,
    ElementCollectionItem, ElementDataType, FxHashMap, XsdFileInfo, XsdRestrictToStandard,
};
use std::collections::HashSet;
use std::fmt::Write;
use std::fs::File;

mod character_types;
mod element_definitions;
mod identifier_enums;
mod perfect_hash;
mod xsd_versions;

struct GroupsInfo {
    groups_array: Vec<Group>,
    versions_array: Vec<usize>,
    versions_index_info: FxHashMap<String, usize>,
    item_ref_array: Vec<GroupItem>,
    item_ref_info: FxHashMap<String, usize>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum GroupItem {
    ElementRef(usize),
    GroupRef(String),
}

enum Group {
    Choice {
        name: String,
        items: Vec<GroupItem>,
        amount: ElementAmount,
    },
    Sequence {
        name: String,
        items: Vec<GroupItem>,
    },
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub(crate) struct SimpleElement {
    pub(crate) name: String,
    pub(crate) typeref: String,
    pub(crate) amount: ElementAmount,
    pub(crate) splittable: bool,
    pub(crate) ordered: bool,
    pub(crate) restrict_std: XsdRestrictToStandard,
    pub(crate) docstring: Option<String>,
}

struct AttributeInfo {
    attributes_array: Vec<Attribute>,
    attributes_index_info: FxHashMap<String, (usize, usize)>,
    attr_ver_index_info: FxHashMap<String, usize>,
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

pub(crate) fn generate_types(autosar_schema: &AutosarDataTypes) {
    let mut generated = String::from(
        r#"use crate::*;
use crate::regex::*;

#[cfg(feature = "docstrings")]
macro_rules! element {
    ($namepart:ident, $etype:literal, $mult:ident, $ordered:literal, $splittable:ident, $stdrestrict:ident, $docid:expr) => {
        ElementDefinition{name: ElementName::$namepart, elemtype: $etype, multiplicity: ElementMultiplicity::$mult, ordered: $ordered, splittable: $splittable, restrict_std: StdRestrict::$stdrestrict, docstring: $docid}
    };
}
#[cfg(not(feature = "docstrings"))]
macro_rules! element {
    ($namepart:ident, $etype:literal, $mult:ident, $ordered:literal, $splittable:ident, $stdrestrict:ident, $docid:expr) => {
        ElementDefinition{name: ElementName::$namepart, elemtype: $etype, multiplicity: ElementMultiplicity::$mult, ordered: $ordered, splittable: $splittable, restrict_std: StdRestrict::$stdrestrict}
    };
}

macro_rules! e {
    ($idx:literal) => {
        GroupItem::Element($idx)
    };
}

macro_rules! g {
    ($idx:literal) => {
        GroupItem::Group($idx)
    };
}



"#,
    );

    let character_types = character_types::generate(autosar_schema);
    generated.push_str(&character_types);

    let element_definitions_array = element_definitions::build_elements_info(autosar_schema);
    let docstring_ids = element_definitions::build_docstrings_info(&element_definitions_array);
    let GroupsInfo {
        mut versions_array,
        versions_index_info,
        groups_array,
        item_ref_array,
        item_ref_info,
    } = build_groups_info(autosar_schema, &element_definitions_array);

    generated.push_str(&element_definitions::generate(
        autosar_schema,
        &element_definitions_array,
        &docstring_ids,
    ));

    generated.push_str(&generate_group_items(&item_ref_array));

    generated.push_str(&generate_groups_array(
        &groups_array,
        &item_ref_info,
        &versions_index_info,
    ));

    let AttributeInfo {
        attributes_array,
        attributes_index_info,
        attr_ver_index_info,
    } = build_attributes_info(autosar_schema, &mut versions_array);
    generated.push_str(&generate_attributes_array(
        autosar_schema,
        &attributes_array,
    ));

    generated.push_str(&generate_versions_array(&versions_array));

    generated.push_str(&generate_element_types(
        autosar_schema,
        &attributes_index_info,
        &attr_ver_index_info,
    ));

    generated.push_str(&element_definitions::generate_docstrings(&docstring_ids));

    use std::io::Write;
    let mut file = File::create("gen/specification.rs").unwrap();
    file.write_all(generated.as_bytes()).unwrap();
}

fn build_groups_info(
    autosar_schema: &AutosarDataTypes,
    element_definitions_array: &[SimpleElement],
) -> GroupsInfo {
    let mut groups_array: Vec<Group> = Vec::new();
    //let mut groups_index_info = FxHashMap::default();
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
    let mut grptypenames: Vec<&String> = autosar_schema.group_types.keys().collect();
    grptypenames.sort_by(|k1, k2| cmp_grouptypenames_subelems(k1, k2, &autosar_schema.group_types));

    // iterate over the groups according to the sorted type name order
    for grptypename in grptypenames {
        if let Some(group) = autosar_schema.group_types.get(grptypename) {
            let items = group.items();
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
                // check if this exact sequence of version information already exists within the verions_array
                if let Some(existing_version_position) = versions_array
                    .iter()
                    .enumerate()
                    .filter(|(_, ver)| **ver == item_versions[0])
                    .map(|(pos, _)| pos)
                    .find(|pos| versions_array[*pos..].starts_with(&item_versions))
                {
                    // exact sequence was found, store the position of the existing data
                    versions_index_info.insert(grptypename.to_owned(), existing_version_position);
                } else {
                    // the exact sequence was not found, append it to the end of versions_array and store the position
                    versions_index_info.insert(grptypename.to_owned(), versions_array.len());
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
                            GroupItem::GroupRef(group_ref.clone())
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
                    item_ref_info.insert(grptypename.clone(), existing_position);
                } else {
                    item_ref_info.insert(grptypename.clone(), item_ref_array.len());
                    item_ref_array.extend(grpitems.iter().cloned());
                }

                let newgroup = match group {
                    ElementCollection::Choice { amount, .. } => Group::Choice {
                        name: grptypename.clone(),
                        items: grpitems,
                        amount: *amount,
                    },
                    ElementCollection::Sequence { .. } => Group::Sequence {
                        name: grptypename.clone(),
                        items: grpitems,
                    },
                };
                groups_array.push(newgroup);
            } else {
                // number of subelements = 0
                versions_index_info.insert(grptypename.to_owned(), 0);
                item_ref_info.insert(grptypename.clone(), 0);
            }
        }
    }

    groups_array.sort_by(|grp1, grp2| grp1.name().cmp(grp2.name()));

    GroupsInfo {
        groups_array,
        versions_array,
        versions_index_info,
        item_ref_array,
        item_ref_info,
    }
}

fn build_attributes_info(
    autosar_schema: &AutosarDataTypes,
    versions_array: &mut Vec<usize>,
) -> AttributeInfo {
    let mut elemtypenames: Vec<&String> = autosar_schema.element_types.keys().collect();
    let mut attributes_array = Vec::new();
    let mut attributes_index_info = FxHashMap::default();
    let mut attr_ver_index_info = FxHashMap::default();

    // sort the element type names so that the element types with the most sub elements are first
    elemtypenames.sort_by(|k1, k2| cmp_elemtypenames_attrs(k1, k2, &autosar_schema.element_types));
    for etypename in elemtypenames {
        //let elemtype = autosar_schema.element_types.get(etypename).unwrap();
        if let Some(attrs) = autosar_schema
            .element_types
            .get(etypename)
            .map(ElementDataType::attributes)
        {
            if !attrs.is_empty() {
                // build a list of versions from the list of items
                let attr_versions: Vec<usize> =
                    attrs.iter().map(|attr| attr.version_info).collect();
                // check if this exact sequence of version information already exists within the verions_array
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
                let mut attrs_copy = attrs.clone();
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

fn generate_group_items(items: &[GroupItem]) -> String {
    let mut generated = format!(
        "\npub(crate) static GROUP_ITEMS: [GroupItem; {}] = [\n",
        items.len()
    );
    for item in items {
        generated.push_str(&match item {
            GroupItem::ElementRef(idx) => format!("e!({idx}), "),
            GroupItem::GroupRef(idx) => format!("g!({idx}), "),
        });
    }
    generated.push_str("];\n");
    generated
}

fn generate_groups_array(
    groups: &[Group],
    item_info: &FxHashMap<String, usize>,
    versions_index_info: &FxHashMap<String, usize>,
) -> String {
    let mut generated = format!(
        "\npub(crate) static GROUPS: [GroupDefinition; {}] = [\n",
        groups.len()
    );
    for group in groups {
        generated.push_str(&match group {
            Group::Choice {
                name,
                items,
                amount,
            } => {
                let item_idx = item_info.get(name).unwrap();
                let ver_idx = versions_index_info.get(name).unwrap();
                if *amount == ElementAmount::Any {
                    format!("    GroupDefinition::Bag{{items: {item_idx}, ver_info: {ver_idx}, item_count: {}}},\n", items.len())
                } else {
                    format!("    GroupDefinition::Choice{{items: {item_idx}, ver_info: {ver_idx}, item_count: {}}},\n", items.len())
                }
            }
            Group::Sequence { name, items } => {
                let item_idx = item_info.get(name).unwrap();
                let ver_idx = versions_index_info.get(name).unwrap();
                format!("    GroupDefinition::Sequence{{items: {item_idx}, ver_info: {ver_idx}, item_count: {}}},\n", items.len())
            }
        });
    }
    generated.push_str("];\n");

    generated
}

fn generate_attributes_array(
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
        "\npub(crate) static ATTRIBUTES: [(AttributeName, u16, bool); {}] = [\n",
        attributes_array.len()
    );
    generated.push_str(&build_attributes_string(
        attributes_array,
        &chartype_nameidx,
    ));
    generated.push_str("\n];\n");

    generated
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

fn generate_element_types(
    autosar_schema: &AutosarDataTypes,
    attributes_index_info: &FxHashMap<String, (usize, usize)>,
    attr_ver_index_info: &FxHashMap<String, usize>,
) -> String {
    let mut generated = String::new();
    let mut elemtypes = String::new();

    let mut grouptypenames: Vec<&String> = autosar_schema.group_types.keys().collect();
    grouptypenames.sort();
    let mut elemtypenames: Vec<&String> = autosar_schema.element_types.keys().collect();
    elemtypenames.sort();
    let mut chartypenames: Vec<&String> = autosar_schema.character_types.keys().collect();
    chartypenames.sort();

    // collect the enum items of DEST attributes
    let ref_attribute_types: HashSet<String> = autosar_schema
        .element_types
        .iter() // iterate over all element types
        .filter_map(|(_, et)| {
            // filtering to get only those which have a DEST attribute
            et.attributes()
                .iter()
                .find(|attr| attr.name == "DEST")
                .map(|attr| &attr.attr_type) // map to provide only the attribute_type string
                .and_then(|attrtype| {
                    // with the attribute type string, get the CharacterDataType of the attribute from the schema
                    autosar_schema
                        .character_types
                        .get(attrtype)
                        .and_then(|ctype| {
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

    // map each group type name to an index
    let grouptype_nameidx: FxHashMap<&str, usize> = grouptypenames
        .iter()
        .enumerate()
        .map(|(idx, name)| (&***name, idx))
        .collect();
    // map each element type name to an index
    let elemtype_nameidx: FxHashMap<&str, usize> = elemtypenames
        .iter()
        .enumerate()
        .map(|(idx, name)| (&***name, idx))
        .collect();
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
    let element_names_of_typename = build_elementnames_of_type_list(autosar_schema);

    writeln!(
        elemtypes,
        "\npub(crate) static DATATYPES: [ElementSpec; {}] = [",
        autosar_schema.element_types.len()
    )
    .unwrap();
    for (idx, etypename) in elemtypenames.iter().enumerate() {
        let elemtype = autosar_schema.element_types.get(*etypename).unwrap();
        let mode = calc_element_mode(autosar_schema, elemtype);
        //let (ordered, splittable) = get_element_attributes(elemtype);

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
        let refstring = if let Some(xsd_typenames) = elemtype.xsd_typenames() {
            let mut namevec: Vec<String> = xsd_typenames
                .iter()
                .filter_map(|xtn| {
                    ref_attribute_types
                        .get(xtn)
                        .map(|name| format!("EnumItem::{}", name_to_identifier(name)))
                })
                .collect();
            namevec.sort();
            namevec.join(", ")
        } else {
            String::new()
        };

        let groupidx: Option<usize> = elemtype
            .group_ref()
            .map(|groupref| *grouptype_nameidx.get(&*groupref).unwrap());

        writeln!(
            elemtypes,
            "    /* {idx:4} */ ElementSpec {{group: {groupidx:?}, \
                            attributes: ({attrs_limit_low}, {attrs_limit_high}), attributes_ver: {attrs_ver_info_low}, \
                            character_data: {chartype}, mode: {mode}, ref_by: &[{refstring}]}}, // {infostring}"
        )
        .unwrap();
    }
    writeln!(elemtypes, "];\n").unwrap();

    writeln!(
        elemtypes,
        "pub(crate) const ROOT_DATATYPE: usize = {};",
        elemtype_nameidx.get("AR:AUTOSAR").unwrap()
    )
    .unwrap();

    generated.write_str(&elemtypes).unwrap();

    generated
}

fn build_elementnames_of_type_list(
    autosar_schema: &AutosarDataTypes,
) -> FxHashMap<String, HashSet<String>> {
    let mut map = FxHashMap::default();
    map.reserve(autosar_schema.group_types.len());

    map.insert("AR:AUTOSAR".to_string(), HashSet::new());
    map.get_mut("AR:AUTOSAR")
        .unwrap()
        .insert("AUTOSAR".to_string());

    for group in autosar_schema.group_types.values() {
        for item in group.items() {
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
    map
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

fn calc_element_mode(
    autosar_schema: &AutosarDataTypes,
    elemtype: &ElementDataType,
) -> &'static str {
    match elemtype {
        ElementDataType::Elements { group_ref, .. } => {
            let element_collection = autosar_schema.group_types.get(group_ref).unwrap();
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
        ElementDataType::Characters { .. } => "ContentMode::Characters",
        ElementDataType::Mixed { .. } => "ContentMode::Mixed",
    }
}

fn cmp_grouptypenames_subelems(
    k1: &str,
    k2: &str,
    elemtypes: &FxHashMap<String, ElementCollection>,
) -> std::cmp::Ordering {
    let len1 = elemtypes.get(k1).map(|ec| ec.items().len()).unwrap();
    let len2 = elemtypes.get(k2).map(|ec| ec.items().len()).unwrap();

    match len2.cmp(&len1) {
        std::cmp::Ordering::Less => std::cmp::Ordering::Less,
        std::cmp::Ordering::Equal => k1.cmp(k2),
        std::cmp::Ordering::Greater => std::cmp::Ordering::Greater,
    }
}

fn cmp_elemtypenames_attrs(
    k1: &str,
    k2: &str,
    elemtypes: &FxHashMap<String, ElementDataType>,
) -> std::cmp::Ordering {
    let len1 = elemtypes
        .get(k1)
        .map(ElementDataType::attributes)
        .map_or(0, std::vec::Vec::len);
    let len2 = elemtypes
        .get(k2)
        .map(ElementDataType::attributes)
        .map_or(0, std::vec::Vec::len);

    match len2.cmp(&len1) {
        std::cmp::Ordering::Less => std::cmp::Ordering::Less,
        std::cmp::Ordering::Equal => k1.cmp(k2),
        std::cmp::Ordering::Greater => std::cmp::Ordering::Greater,
    }
}

fn name_to_identifier(name: &str) -> String {
    let mut keep_capital = true;
    let mut force_capital = false;
    let mut result = String::new();

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
                result.push(c.to_ascii_uppercase());
                force_capital = false;
            } else if keep_capital {
                result.push(c);
                keep_capital = false;
            } else {
                result.push(c.to_ascii_lowercase());
            }
        }
    }

    result
}

impl Group {
    fn name(&self) -> &str {
        match self {
            Group::Choice { name, .. } | Group::Sequence { name, .. } => name,
        }
    }
}

impl From<&Element> for SimpleElement {
    fn from(element: &Element) -> Self {
        Self {
            name: element.name.clone(),
            typeref: element.typeref.clone(),
            amount: element.amount,
            splittable: element.splittable,
            ordered: element.ordered,
            restrict_std: element.restrict_std,
            docstring: element.docstring.clone(),
        }
    }
}
