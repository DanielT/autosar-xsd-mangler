use super::{
    Attribute, AutosarDataTypes, CharacterDataType, Element, ElementAmount, ElementCollection,
    ElementCollectionItem, ElementDataType, File, FxHashMap, XsdFileInfo, XsdRestrictToStandard,
};
use std::collections::HashSet;
use std::fmt::Write;

mod perfect_hash;

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

    generate_xsd_versions(xsd_config);

    generate_identifier_enums(autosar_schema);

    generate_types(autosar_schema);
}

fn create_output_dir() {
    let _ = std::fs::create_dir("gen");
}

fn generate_xsd_versions(xsd_config: &[XsdFileInfo]) {
    let mut match_lines = String::new();
    let mut filename_lines = String::new();
    let mut desc_lines = String::new();
    let mut generated = String::from(
        r"use num_derive::FromPrimitive;
use num_traits::cast::FromPrimitive;

#[derive(Debug)]
/// Error type returned when `from_str()` / `parse()` for `AutosarVersion` fails
pub struct ParseAutosarVersionError;

#[allow(non_camel_case_types)]
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Hash, FromPrimitive)]
#[repr(u32)]
#[non_exhaustive]
/// Enum of all Autosar versions
pub enum AutosarVersion {
",
    );

    for (idx, xsd_file_info) in xsd_config.iter().enumerate() {
        writeln!(
            generated,
            r#"    /// {} - xsd file name: {}"#,
            xsd_file_info.desc, xsd_file_info.name
        )
        .unwrap();
        writeln!(
            generated,
            r#"    {} = 0x{:x},"#,
            xsd_file_info.ident,
            1 << idx
        )
        .unwrap();
        writeln!(
            match_lines,
            r#"            "{}" => Ok(Self::{}),"#,
            xsd_file_info.name, xsd_file_info.ident
        )
        .unwrap();
        writeln!(
            filename_lines,
            r#"            Self::{} => "{}","#,
            xsd_file_info.ident, xsd_file_info.name
        )
        .unwrap();
        writeln!(
            desc_lines,
            r#"            Self::{} => "{}","#,
            xsd_file_info.ident, xsd_file_info.desc
        )
        .unwrap();
    }
    let lastident = xsd_config[xsd_config.len() - 1].ident;
    writeln!(
        generated,
        r#"}}

impl AutosarVersion {{
    /// get the name of the xds file matching the Autosar version
    #[must_use]
    pub fn filename(&self) -> &'static str {{
        match self {{
{filename_lines}
        }}
    }}

    /// Human readable description of the Autosar version
    ///
    /// This is particularly useful for the later versions, where the xsd files are just sequentially numbered.
    /// For example `Autosar_00050` -> "AUTOSAR R21-11"
    #[must_use]
    pub fn describe(&self) -> &'static str {{
        match self {{
{desc_lines}
        }}
    }}

    /// make an `AutosarVersion` from a u32 value
    ///
    /// All `AutosarVersion`s are associated with a power of two u32 value, for example `Autosar_4_3_0` == 0x100
    /// If the given value is a valid constant of `AutosarVersion`, the enum value will be returnd
    ///
    /// This is useful in order to decode version masks
    #[must_use]
    pub fn from_val(n: u32) -> Option<Self> {{
        Self::from_u32(n)
    }}

    /// `AutosarVersion::LATEST` is an alias of which ever is the latest version
    pub const LATEST: AutosarVersion = AutosarVersion::{lastident};
}}

impl std::str::FromStr for AutosarVersion {{
    type Err = ParseAutosarVersionError;
    fn from_str(input: &str) -> Result<Self, Self::Err> {{
        match input {{
{match_lines}
            _ => Err(ParseAutosarVersionError),
        }}
    }}
}}

impl std::fmt::Display for AutosarVersion {{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{
        f.write_str(self.describe())
    }}
}}
"#,
    )
    .unwrap();

    use std::io::Write;
    let mut file = File::create("gen/autosarversion.rs").unwrap();
    file.write_all(generated.as_bytes()).unwrap();
}

fn generate_identifier_enums(autosar_schema: &AutosarDataTypes) {
    let mut attribute_names = HashSet::new();
    let mut element_names = HashSet::new();
    let mut enum_items = HashSet::new();

    element_names.insert("AUTOSAR".to_string());

    // for each group type in the schema: collect element names
    for group_type in autosar_schema.group_types.values() {
        for ec_item in group_type.items() {
            // for each sub-element of the current element type (skipping groups)
            if let ElementCollectionItem::Element(elem) = ec_item {
                if element_names.get(&elem.name).is_none() {
                    element_names.insert(elem.name.clone());
                }
            }
        }
    }
    // for each element data type in the schema: collect attribute names
    for artype in autosar_schema.element_types.values() {
        for attr in artype.attributes() {
            attribute_names.insert(attr.name.clone());
        }
    }

    // collect all enum values in use by any character data type
    for artype in autosar_schema.character_types.values() {
        if let CharacterDataType::Enum(enumdef) = &artype {
            for (itemname, _) in &enumdef.enumitems {
                enum_items.insert(itemname.to_owned());
            }
        }
    }

    let mut element_names: Vec<String> = element_names
        .iter()
        .map(std::borrow::ToOwned::to_owned)
        .collect();
    element_names.sort();
    let mut attribute_names: Vec<String> = attribute_names
        .iter()
        .map(std::borrow::ToOwned::to_owned)
        .collect();
    attribute_names.sort();
    let mut enum_items: Vec<String> = enum_items
        .iter()
        .map(std::borrow::ToOwned::to_owned)
        .collect();
    enum_items.sort();

    let element_name_refs: Vec<&str> = element_names.iter().map(|name| &**name).collect();
    let disps = perfect_hash::make_perfect_hash(&element_name_refs, 7);
    let enumstr = generate_enum(
        "ElementName",
        "Enum of all element names in Autosar",
        &element_name_refs,
        &disps,
    );
    use std::io::Write;
    let mut file = File::create("gen/elementname.rs").unwrap();
    file.write_all(enumstr.as_bytes()).unwrap();

    let attribute_name_refs: Vec<&str> = attribute_names.iter().map(|name| &**name).collect();
    let disps = perfect_hash::make_perfect_hash(&attribute_name_refs, 5);
    let enumstr = generate_enum(
        "AttributeName",
        "Enum of all attribute names in Autosar",
        &attribute_name_refs,
        &disps,
    );
    let mut file = File::create("gen/attributename.rs").unwrap();
    file.write_all(enumstr.as_bytes()).unwrap();

    let enum_item_refs: Vec<&str> = enum_items.iter().map(|name| &**name).collect();
    let disps = perfect_hash::make_perfect_hash(&enum_item_refs, 7);

    let enumstr = generate_enum(
        "EnumItem",
        "Enum of all possible enum values in Autosar",
        &enum_item_refs,
        &disps,
    );
    let mut file = File::create("gen/enumitem.rs").unwrap();
    file.write_all(enumstr.as_bytes()).unwrap();
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

    let character_types = generate_character_types(autosar_schema);
    generated.push_str(&character_types);

    let element_definitions_array = build_elements_info(autosar_schema);
    let docstring_ids = build_docstrings_info(&element_definitions_array);
    let GroupsInfo {
        mut versions_array,
        versions_index_info,
        groups_array,
        item_ref_array,
        item_ref_info,
    } = build_groups_info(autosar_schema, &element_definitions_array);

    generated.push_str(&generate_element_definitions_array(
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

    generated.push_str(&generate_element_docstrings(&docstring_ids));

    use std::io::Write;
    let mut file = File::create("gen/specification.rs").unwrap();
    file.write_all(generated.as_bytes()).unwrap();
}

pub(crate) fn generate_character_types(autosar_schema: &AutosarDataTypes) -> String {
    let mut generated = String::new();

    let regexes: FxHashMap<String, String> = VALIDATOR_REGEX_MAPPING
        .iter()
        .map(|(regex, name)| ((*regex).to_string(), (*name).to_string()))
        .collect();

    let mut ctnames: Vec<&String> = autosar_schema.character_types.keys().collect();
    ctnames.sort();

    writeln!(
        generated,
        "pub(crate) static CHARACTER_DATA: [CharacterDataSpec; {}] = [",
        ctnames.len()
    )
    .unwrap();
    for ctname in &ctnames {
        let chtype = autosar_schema.character_types.get(*ctname).unwrap();

        let chdef = match chtype {
            CharacterDataType::Pattern {
                pattern,
                max_length,
            } => {
                let fullmatch_pattern = format!("^({pattern})$");
                // no longer using proc-macro-regex due to unacceptably long run-times of the proc macro (> 5 Minutes!)
                // if regexes.get(&fullmatch_pattern).is_none() {
                //     let regex_validator_name = format!("validate_regex_{}", regexes.len() + 1);
                //     writeln!(validators, r#"regex!({regex_validator_name} br"{fullmatch_pattern}");"#).unwrap();
                //     regexes.insert(fullmatch_pattern.clone(), regex_validator_name);
                // }
                let regex_validator_name = regexes
                    .get(&fullmatch_pattern)
                    .unwrap_or_else(|| panic!("missing regex: {fullmatch_pattern}"));
                format!(
                    r#"CharacterDataSpec::Pattern{{check_fn: {regex_validator_name}, regex: r"{pattern}", max_length: {max_length:?}}}"#
                )
            }
            CharacterDataType::Enum(enumdef) => {
                let enumitem_strs: Vec<String> = enumdef
                    .enumitems
                    .iter()
                    .map(|(name, ver)| {
                        format!("(EnumItem::{}, 0x{ver:x})", name_to_identifier(name))
                    })
                    .collect();
                format!(
                    r#"CharacterDataSpec::Enum{{items: &[{}]}}"#,
                    enumitem_strs.join(", ")
                )
            }
            CharacterDataType::String {
                max_length,
                preserve_whitespace,
            } => {
                format!(
                    r#"CharacterDataSpec::String{{preserve_whitespace: {preserve_whitespace}, max_length: {max_length:?}}}"#
                )
            }
            CharacterDataType::UnsignedInteger => "CharacterDataSpec::UnsignedInteger".to_string(),
            CharacterDataType::Double => "CharacterDataSpec::Double".to_string(),
        };
        generated.push_str("    ");
        generated.push_str(&chdef);
        generated.push_str(",\n");
    }
    generated.push_str("];\n");

    let (reference_type_idx, _) = ctnames
        .iter()
        .enumerate()
        .find(|(_, name)| **name == "AR:REF--SIMPLE")
        .expect("reference type \"AR:REF--SIMPLE\" not found ?!");
    generated.push_str(&format!(
        "pub(crate) const REFERENCE_TYPE_IDX: u16 = {reference_type_idx};\n"
    ));

    generated
}

fn build_elements_info(autosar_schema: &AutosarDataTypes) -> Vec<SimpleElement> {
    // make a hashset of all elements to eliminate any duplicates
    let all_elements: HashSet<SimpleElement> = autosar_schema
        .group_types
        .values()
        .flat_map(|group| {
            group.items().iter().filter_map(|item| match item {
                ElementCollectionItem::Element(element) => Some(SimpleElement::from(element)),
                ElementCollectionItem::GroupRef(_) => None,
            })
        })
        .collect();
    let mut element_definitions_array: Vec<SimpleElement> = all_elements.into_iter().collect();
    element_definitions_array.sort_by(|e1, e2| {
        e1.name
            .cmp(&e2.name)
            .then(e1.typeref.cmp(&e2.typeref))
            .then(e1.docstring.cmp(&e2.docstring))
            .then(e1.ordered.cmp(&e2.ordered))
            .then(e1.splittable.cmp(&e2.splittable))
            .then(e1.restrict_std.cmp(&e2.restrict_std))
    });

    // create an element definition for the AUTOSAR element - the xsd files contain this info, but it is lost before we get here
    element_definitions_array.insert(
        0,
        SimpleElement {
            name: String::from("AUTOSAR"),
            typeref: String::from("AR:AUTOSAR"),
            amount: ElementAmount::One,
            splittable: true,
            ordered: false,
            restrict_std: XsdRestrictToStandard::NotSet,
            docstring: None,
        },
    );
    element_definitions_array
}

fn build_docstrings_info(element_definitions_array: &[SimpleElement]) -> FxHashMap<String, usize> {
    // first, put all docstrings into a HashSet to elimitate duplicates
    let docstrings: HashSet<String> = element_definitions_array
        .iter()
        .filter_map(|e| e.docstring.clone())
        .collect();
    // transform the HashSet into a Vec and sort the list
    let mut docstrings: Vec<String> = docstrings.into_iter().collect();
    docstrings.sort();
    // enable lookup of entries by transferring iverything into a HashMap<docstring, position>

    docstrings
        .into_iter()
        .enumerate()
        .map(|(idx, ds)| (ds, idx))
        .collect()
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

fn generate_element_definitions_array(
    autosar_schema: &AutosarDataTypes,
    elements: &[SimpleElement],
    docstring_ids: &FxHashMap<String, usize>,
) -> String {
    let mut elemtypenames: Vec<&String> = autosar_schema.element_types.keys().collect();
    elemtypenames.sort();
    let elemtype_nameidx: FxHashMap<&str, usize> = elemtypenames
        .iter()
        .enumerate()
        .map(|(idx, name)| (&***name, idx))
        .collect();
    let mut generated = format!(
        "\npub(crate) static ELEMENTS: [ElementDefinition; {}] = [\n",
        elements.len()
    );
    for elem in elements {
        generated.push_str(&build_element_string(
            elem,
            &elemtype_nameidx,
            docstring_ids,
        ));
    }
    generated.push_str("];\n");

    generated
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

fn build_element_string(
    elem: &SimpleElement,
    elemtype_nameidx: &FxHashMap<&str, usize>,
    docstring_ids: &FxHashMap<String, usize>,
) -> String {
    // let mut sub_element_strings: Vec<String> = Vec::new();
    let elem_docstring_id = elem
        .docstring
        .as_ref()
        .and_then(|ds| docstring_ids.get(ds))
        .copied();
    let restrict_txt = restrict_std_to_text(elem.restrict_std);
    format!(
        "    element!({}, {}, {:?}, {}, {}, {}, {:?}),\n",
        name_to_identifier(&elem.name),
        elemtype_nameidx.get(&*elem.typeref).unwrap(),
        elem.amount,
        elem.ordered,
        elem.splittable,
        restrict_txt,
        elem_docstring_id,
    )
}

fn generate_element_docstrings(docstring_ids: &FxHashMap<String, usize>) -> String {
    let mut docstrings: Vec<String> = docstring_ids.keys().cloned().collect();
    docstrings.sort_by(|a, b| docstring_ids.get(a).cmp(&docstring_ids.get(b)));

    let mut output = String::from("\n#[cfg(feature = \"docstrings\")]\n");
    output.push_str(&format!(
        "pub(crate) static ELEMENT_DOCSTRINGS: [&'static str; {}] = [\n",
        docstrings.len()
    ));
    for ds in docstrings {
        output.push_str(&format!("    {ds:?},\n"));
    }
    output.push_str("];\n");
    output
}

fn restrict_std_to_text(restrict_std: XsdRestrictToStandard) -> &'static str {
    match restrict_std {
        XsdRestrictToStandard::NotSet | XsdRestrictToStandard::Both => "NotRestricted",
        XsdRestrictToStandard::ClassicPlatform => "ClassicPlatform",
        XsdRestrictToStandard::AdaptivePlatform => "AdaptivePlatform",
    }
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

fn generate_enum(
    enum_name: &str,
    enum_docstring: &str,
    item_names: &[&str],
    disps: &[(u32, u32)],
) -> String {
    let mut generated = String::new();
    let displen = disps.len();

    let width = item_names.iter().map(|name| name.len()).max().unwrap();

    writeln!(
        generated,
        "use crate::hashfunc;

#[derive(Debug)]
/// The error type `Parse{enum_name}Error` is returned when `from_str()` / `parse()` fails for `{enum_name}`
pub struct Parse{enum_name}Error;
"
    )
    .unwrap();
    generated
        .write_str(
            "#[allow(dead_code, non_camel_case_types)]
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
#[repr(u16)]
#[non_exhaustive]
",
        )
        .unwrap();
    writeln!(generated, "/// {enum_docstring}\npub enum {enum_name} {{").unwrap();
    let mut hash_sorted_item_names = item_names.to_owned();
    hash_sorted_item_names.sort_by(|k1, k2| {
        perfect_hash::get_index(k1, disps, item_names.len()).cmp(&perfect_hash::get_index(
            k2,
            disps,
            item_names.len(),
        ))
    });
    for item_name in item_names {
        let idx = perfect_hash::get_index(item_name, disps, item_names.len());
        let ident = name_to_identifier(item_name);
        writeln!(generated, "    /// {item_name}").unwrap();
        writeln!(generated, "    {ident:width$}= {idx},").unwrap();
    }
    writeln!(generated, "}}").unwrap();

    let length = item_names.len();
    writeln!(
        generated,
        r##"
impl {enum_name} {{
    const STRING_TABLE: [&'static str; {length}] = {hash_sorted_item_names:?};

    /// derive an enum entry from an input string using a perfect hash function
    pub fn from_bytes(input: &[u8]) -> Result<Self, Parse{enum_name}Error> {{
        static DISPLACEMENTS: [(u16, u16); {displen}] = {disps:?};
        let (g, f1, f2) = hashfunc(input);
        let (d1, d2) = DISPLACEMENTS[(g % {displen}) as usize];
        let item_idx = u32::from(d2).wrapping_add(f1.wrapping_mul(u32::from(d1))).wrapping_add(f2) as usize % {length};
        if {enum_name}::STRING_TABLE[item_idx].as_bytes() != input {{
            return Err(Parse{enum_name}Error);
        }}
        Ok(unsafe {{
            std::mem::transmute::<u16, Self>(item_idx as u16)
        }})
    }}

    /// get the str corresponding to an item
    ///
    /// The returned &str has static lifetime, becasue it is a reference to an entry in a list of constants
    #[must_use]
    pub fn to_str(&self) -> &'static str {{
        {enum_name}::STRING_TABLE[*self as usize]
    }}
}}

impl std::str::FromStr for {enum_name} {{
    type Err = Parse{enum_name}Error;
    fn from_str(input: &str) -> Result<Self, Self::Err> {{
        Self::from_bytes(input.as_bytes())
    }}
}}

impl std::fmt::Debug for {enum_name} {{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{
        f.write_str({enum_name}::STRING_TABLE[*self as usize])
    }}
}}

impl std::fmt::Display for {enum_name} {{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{
        f.write_str({enum_name}::STRING_TABLE[*self as usize])
    }}
}}
"##
    )
    .unwrap();

    generated
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

// map a regex to a validation finction name
static VALIDATOR_REGEX_MAPPING: [(&str, &str); 28] = [
    (r"^(0x[0-9a-z]*)$", "validate_regex_1"),
    (
        r"^([1-9][0-9]*|0[xX][0-9a-fA-F]*|0[bB][0-1]+|0[0-7]*|UNSPECIFIED|UNKNOWN|BOOLEAN|PTR)$",
        "validate_regex_2",
    ),
    (
        r"^([1-9][0-9]*|0[xX][0-9a-fA-F]+|0[0-7]*|0[bB][0-1]+|ANY|ALL)$",
        "validate_regex_3",
    ),
    (r"^([0-9]+|ANY)$", "validate_regex_4"),
    (r"^([0-9]+|STRING|ARRAY)$", "validate_regex_5"),
    (r"^(0|1|true|false)$", "validate_regex_6"),
    (r"^([a-zA-Z_][a-zA-Z0-9_]*)$", "validate_regex_7"),
    (r"^([a-zA-Z][a-zA-Z0-9_]*)$", "validate_regex_8"),
    (
        r"^(([0-9]{4}-[0-9]{2}-[0-9]{2})(T[0-9]{2}:[0-9]{2}:[0-9]{2}(Z|([+\-][0-9]{2}:[0-9]{2})))?)$",
        "validate_regex_9",
    ),
    (r"^([a-zA-Z][a-zA-Z0-9-]*)$", "validate_regex_10"),
    (r"^([0-9a-zA-Z_\-]+)$", "validate_regex_11"),
    (
        r"^(%[ \-+#]?[0-9]*(\.[0-9]+)?[bBdiouxXfeEgGcs])$",
        "validate_regex_12",
    ),
    (
        r"^(0|[\+\-]?[1-9][0-9]*|0[xX][0-9a-fA-F]+|0[bB][0-1]+|0[0-7]+)$",
        "validate_regex_13",
    ),
    (
        r"^((25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)|ANY)$",
        "validate_regex_14",
    ),
    (
        r"^([0-9A-Fa-f]{1,4}(:[0-9A-Fa-f]{1,4}){7,7}|ANY)$",
        "validate_regex_15",
    ),
    (
        r"^((0[xX][0-9a-fA-F]+)|(0[0-7]+)|(0[bB][0-1]+)|(([+\-]?[1-9][0-9]+(\.[0-9]+)?|[+\-]?[0-9](\.[0-9]+)?)([eE]([+\-]?)[0-9]+)?)|\.0|INF|-INF|NaN)$",
        "validate_regex_16",
    ),
    (
        r"^(([0-9a-fA-F]{2}:){5}[0-9a-fA-F]{2})$",
        "validate_regex_17",
    ),
    (
        r"^([a-zA-Z_][a-zA-Z0-9_]*(\[([a-zA-Z_][a-zA-Z0-9_]*|[0-9]+)\])*(\.[a-zA-Z_][a-zA-Z0-9_]*(\[([a-zA-Z_][a-zA-Z0-9_]*|[0-9]+)\])*)*)$",
        "validate_regex_18",
    ),
    (r"^([A-Z][a-zA-Z0-9_]*)$", "validate_regex_19"),
    (r"^([1-9][0-9]*)$", "validate_regex_20"),
    (
        r"^(0|[\+]?[1-9][0-9]*|0[xX][0-9a-fA-F]+|0[bB][0-1]+|0[0-7]+)$",
        "validate_regex_21",
    ),
    (
        r"^([a-zA-Z]([a-zA-Z0-9]|_[a-zA-Z0-9])*_?)$",
        "validate_regex_22",
    ),
    (
        r"^(-?([0-9]+|MAX-TEXT-SIZE|ARRAY-SIZE))$",
        "validate_regex_23",
    ),
    (
        r"^(/?[a-zA-Z][a-zA-Z0-9_]{0,127}(/[a-zA-Z][a-zA-Z0-9_]{0,127})*)$",
        "validate_regex_24",
    ),
    (
        r"^([0-9]+\.[0-9]+\.[0-9]+([\._;].*)?)$",
        "validate_regex_25",
    ),
    (
        r"^((0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(-((0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(\.(0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(\+([0-9a-zA-Z-]+(\.[0-9a-zA-Z-]+)*))?)$",
        "validate_regex_26",
    ),
    (r"^([0-1])$", "validate_regex_27"),
    (
        r"^((-?[a-zA-Z_]+)(( )+-?[a-zA-Z_]+)*)$",
        "validate_regex_28",
    ),
];
