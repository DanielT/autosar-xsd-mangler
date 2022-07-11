use super::*;
use std::collections::HashSet;
use std::fmt::Write;

mod perfect_hash;

struct SubelementInfo {
    subelements_array: Vec<ElementCollectionItem>,
    subelements_index_info: HashMap<String, (usize, usize)>,
    versions_array: Vec<usize>,
    versions_index_info: HashMap<String, usize>,
}

struct AttributeInfo {
    attributes_array: Vec<Attribute>,
    attributes_index_info: HashMap<String, (usize, usize)>,
    attr_ver_index_info: HashMap<String, usize>,
}

pub(crate) fn generate(
    xsd_config: &[XsdFileInfo],
    autosar_schema: &AutosarDataTypes,
) -> Result<(), String> {
    generate_xsd_versions(xsd_config)?;

    generate_identifier_enums(autosar_schema)?;

    generate_types(autosar_schema)?;

    Ok(())
}

fn generate_xsd_versions(xsd_config: &[XsdFileInfo]) -> Result<(), String> {
    let mut match_lines = String::new();
    let mut filename_lines = String::new();
    let mut desc_lines = String::new();
    let mut generated = String::from(
        r##"
/// Error type returned when from_str / parse for AutosarVersion fails
pub struct ParseAutosarVersionError;

#[allow(non_camel_case_types)]
#[derive(Debug, Eq, PartialEq, Clone, Copy, Hash)]
#[repr(u32)]
/// Enum of all Autosar versions
pub enum AutosarVersion {
"##,
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
    pub fn filename(&self) -> &'static str {{
        match self {{
{filename_lines}
        }}
    }}

    /// Human readable description of the Autosar version
    ///
    /// This is particularly useful for the later versions, where the xsd files are just sequentially numbered.
    /// For example Autosar_00050 -> "AUTOSAR 4.7.0"
    pub fn describe(&self) -> &'static str {{
        match self {{
{desc_lines}
        }}
    }}

    /// AutosarVersion::LATEST is an alias of whichever is the latest version, currently Autosar_00050
    pub const LATEST: AutosarVersion = AutosarVersion::{lastident};
}}

impl std::str::FromStr for AutosarVersion {{
    type Err = ParseAutosarVersionError;
    fn from_str(input: &str) -> Result<Self, Self::Err> {{
        match input {{
{match_lines}
            _ => Err(ParseAutosarVersionError)
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

    Ok(())
}

fn generate_identifier_enums(autosar_schema: &AutosarDataTypes) -> Result<(), String> {
    let mut attribute_names = HashSet::new();
    let mut element_names = HashMap::<String, HashSet<String>>::new();
    let mut enum_items = HashSet::new();

    element_names.insert("AUTOSAR".to_string(), HashSet::new());
    element_names
        .get_mut("AUTOSAR")
        .unwrap()
        .insert("AR:AUTOSAR".to_string());

    for artype in autosar_schema.element_types.values() {
        if let Some(element_collection) = artype.collection() {
            for ec_item in element_collection.items() {
                if let ElementCollectionItem::Element(elem) = ec_item {
                    if element_names.get(&elem.name).is_none() {
                        element_names.insert(elem.name.to_owned(), HashSet::new());
                    }
                    element_names
                        .get_mut(&elem.name)
                        .unwrap()
                        .insert(elem.typeref.clone());
                }
            }
        }
        if let Some(attributes) = artype.attributes() {
            for attr in attributes {
                attribute_names.insert(attr.name.to_owned());
            }
        }
    }

    for artype in autosar_schema.character_types.values() {
        if let CharacterDataType::Enum(enumdef) = &artype {
            for (itemname, _) in &enumdef.enumitems {
                enum_items.insert(itemname.to_owned());
            }
        }
    }

    let mut element_names: Vec<String> = element_names.keys().map(|name| name.to_owned()).collect();
    element_names.sort();
    let mut attribute_names: Vec<String> =
        attribute_names.iter().map(|name| name.to_owned()).collect();
    attribute_names.sort();
    let mut enum_items: Vec<String> = enum_items.iter().map(|item| item.to_owned()).collect();
    enum_items.sort();

    let element_name_refs: Vec<&str> = element_names.iter().map(|name| &**name).collect();
    // let (hashsize, param1, param2) = perfect_hash::find_hash_parameters(&element_name_refs).unwrap();
    // let (hashsize, param1, param2) = (4753, 20751, 3074);
    // let (hashsize, param1, param2) = (4701, 48655, 55675);
    let (hashsize, param1, param2) = (4645, 19458, 3491);
    let enumstr = generate_enum(
        "ElementName",
        "Enum of all element names in Autosar",
        &element_name_refs,
        hashsize,
        param1,
        param2,
    );
    use std::io::Write;
    let mut file = File::create("gen/elementname.rs").unwrap();
    file.write_all(enumstr.as_bytes()).unwrap();

    let attribute_name_refs: Vec<&str> = attribute_names.iter().map(|name| &**name).collect();
    // let (hashsize, param1, param2) = perfect_hash::find_hash_parameters(&attribute_name_refs).unwrap();
    let (hashsize, param1, param2) = (54, 13929, 17554);
    let enumstr = generate_enum(
        "AttributeName",
        "Enum of all attribute names in Autosar",
        &attribute_name_refs,
        hashsize,
        param1,
        param2,
    );
    let mut file = File::create("gen/attributename.rs").unwrap();
    file.write_all(enumstr.as_bytes()).unwrap();

    let enum_item_refs: Vec<&str> = enum_items.iter().map(|name| &**name).collect();
    // let (hashsize, param1, param2) = perfect_hash::find_hash_parameters(&enum_item_refs).unwrap();
    // let (hashsize, param1, param2) = (1893, 47826, 2613);
    let (hashsize, param1, param2) = (1869, 27359, 20733);
    let enumstr = generate_enum(
        "EnumItem",
        "Enum of all possible enum values in Autosar",
        &enum_item_refs,
        hashsize,
        param1,
        param2,
    );
    let mut file = File::create("gen/enumitem.rs").unwrap();
    file.write_all(enumstr.as_bytes()).unwrap();

    Ok(())
}

pub(crate) fn generate_types(autosar_schema: &AutosarDataTypes) -> Result<(), String> {
    let mut generated = String::from(
        r##"use crate::*;
use crate::regex::*;

"##,
    );

    let character_types = generate_character_types(autosar_schema)?;
    generated.write_str(&character_types).unwrap();

    let SubelementInfo {
        subelements_array,
        subelements_index_info,
        mut versions_array,
        versions_index_info,
    } = build_subelements_info(autosar_schema)?;
    generated
        .write_str(&generate_subelements_array(
            autosar_schema,
            &subelements_array,
        ))
        .unwrap();

    let AttributeInfo {
        attributes_array,
        attributes_index_info,
        attr_ver_index_info,
    } = build_attributes_info(autosar_schema, &mut versions_array)?;
    generated
        .write_str(&generate_attributes_array(
            autosar_schema,
            &attributes_array,
        ))
        .unwrap();

    generated
        .write_str(&generate_versions_array(&versions_array))
        .unwrap();

    generated
        .write_str(&generate_element_types(
            autosar_schema,
            subelements_index_info,
            versions_index_info,
            attributes_index_info,
            attr_ver_index_info,
        )?)
        .unwrap();

    use std::io::Write;
    let mut file = File::create("gen/specification.rs").unwrap();
    file.write_all(generated.as_bytes()).unwrap();

    Ok(())
}

pub(crate) fn generate_character_types(
    autosar_schema: &AutosarDataTypes,
) -> Result<String, String> {
    let mut generated = String::new();

    let regexes: HashMap<String, String> = VALIDATOR_REGEX_MAPPING
        .iter()
        .map(|(regex, name)| (regex.to_string(), name.to_string()))
        .collect();

    let mut ctnames: Vec<&String> = autosar_schema.character_types.keys().collect();
    ctnames.sort();

    writeln!(
        generated,
        "pub(crate) static CHARACTER_DATA: [CharacterDataSpec; {}] = [",
        ctnames.len()
    )
    .unwrap();
    for ctname in ctnames.iter() {
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
                let regex_validator_name = regexes.get(&fullmatch_pattern).unwrap();
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

    Ok(generated)
}

fn build_subelements_info(autosar_schema: &AutosarDataTypes) -> Result<SubelementInfo, String> {
    let mut elemtypenames: Vec<&String> = autosar_schema.element_types.keys().collect();
    let mut subelements_array: Vec<ElementCollectionItem> = Vec::new();
    let mut subelements_index_info = HashMap::new();
    let mut versions_array = Vec::new();
    let mut versions_index_info: HashMap<String, usize> = HashMap::new();

    // sort the element type names so that the element types with the most sub elements are first
    elemtypenames
        .sort_by(|k1, k2| cmp_elemtypenames_subelems(k1, k2, &autosar_schema.element_types));

    for etypename in elemtypenames {
        //let elemtype = autosar_schema.element_types.get(etypename).unwrap();
        if let Some(items) = autosar_schema
            .element_types
            .get(etypename)
            .and_then(|e| e.collection())
            .map(|ec| ec.items())
        {
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
                    versions_index_info.insert(etypename.to_owned(), existing_version_position);
                } else {
                    // the exact sequence was not found, append it to the end of versions_array and store the position
                    versions_index_info.insert(etypename.to_owned(), versions_array.len());
                    versions_array.extend(item_versions.iter());
                }

                // create a copy of the items and strip the version_info from the copied items
                // the version info is handled separately and this makes it more likely that identical sequences can be found
                let mut items_copy = items.clone();
                items_copy.iter_mut().for_each(|it| {
                    if let ElementCollectionItem::Element(Element { version_info, .. }) = it {
                        *version_info = 0;
                    }
                });
                // as for the versions above, try to find the exact sequene of items in the overall list of subelements
                if let Some(existing_position) = subelements_array
                    .iter()
                    .enumerate()
                    .filter(|(_, ec)| *ec == &items_copy[0])
                    .map(|(pos, _)| pos)
                    .find(|pos| subelements_array[*pos..].starts_with(&items_copy))
                {
                    subelements_index_info.insert(
                        etypename.to_owned(),
                        (existing_position, existing_position + items.len()),
                    );
                } else {
                    subelements_index_info.insert(
                        etypename.to_owned(),
                        (
                            subelements_array.len(),
                            subelements_array.len() + items.len(),
                        ),
                    );
                    subelements_array.append(&mut items_copy);
                }
            } else {
                // number of subelements = 0
                subelements_index_info.insert(etypename.to_owned(), (0, 0));
                versions_index_info.insert(etypename.to_owned(), 0);
            }
        } else {
            // no subelement info present
            subelements_index_info.insert(etypename.to_owned(), (0, 0));
            versions_index_info.insert(etypename.to_owned(), 0);
        }
    }

    Ok(SubelementInfo {
        subelements_array,
        subelements_index_info,
        versions_array,
        versions_index_info,
    })
}

fn build_attributes_info(
    autosar_schema: &AutosarDataTypes,
    versions_array: &mut Vec<usize>,
) -> Result<AttributeInfo, String> {
    let mut elemtypenames: Vec<&String> = autosar_schema.element_types.keys().collect();
    let mut attributes_array = Vec::new();
    let mut attributes_index_info = HashMap::new();
    let mut attr_ver_index_info = HashMap::new();

    // sort the element type names so that the element types with the most sub elements are first
    elemtypenames.sort_by(|k1, k2| cmp_elemtypenames_attrs(k1, k2, &autosar_schema.element_types));
    for etypename in elemtypenames {
        //let elemtype = autosar_schema.element_types.get(etypename).unwrap();
        if let Some(attrs) = autosar_schema
            .element_types
            .get(etypename)
            .and_then(|e| e.attributes())
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

    Ok(AttributeInfo {
        attributes_array,
        attributes_index_info,
        attr_ver_index_info,
    })
}

fn generate_subelements_array(
    autosar_schema: &AutosarDataTypes,
    sub_elements: &[ElementCollectionItem],
) -> String {
    let mut elemtypenames: Vec<&String> = autosar_schema.element_types.keys().collect();
    elemtypenames.sort();
    let elemtype_nameidx: HashMap<&str, usize> = elemtypenames
        .iter()
        .enumerate()
        .map(|(idx, name)| (&***name, idx))
        .collect();
    let mut generated = format!(
        "\npub(crate) static SUBELEMENTS: [SubElement; {}] = [\n",
        sub_elements.len()
    );
    generated.push_str(&build_sub_elements_string(sub_elements, &elemtype_nameidx));
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
    let chartype_nameidx: HashMap<&str, usize> = chartypenames
        .iter()
        .enumerate()
        .map(|(idx, name)| (&***name, idx))
        .collect();
    let mut generated = format!(
        "\npub(crate) static ATTRIBUTES: [(AttributeName, usize, bool); {}] = [\n",
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
    subelements_index_info: HashMap<String, (usize, usize)>,
    subelements_ver_index_info: HashMap<String, usize>,
    attributes_index_info: HashMap<String, (usize, usize)>,
    attr_ver_index_info: HashMap<String, usize>,
) -> Result<String, String> {
    let mut generated = String::new();
    let mut elemtypes = String::new();

    let mut elemtypenames: Vec<&String> = autosar_schema.element_types.keys().collect();
    elemtypenames.sort();
    let mut chartypenames: Vec<&String> = autosar_schema.character_types.keys().collect();
    chartypenames.sort();

    // map each element type name to an index
    let elemtype_nameidx: HashMap<&str, usize> = elemtypenames
        .iter()
        .enumerate()
        .map(|(idx, name)| (&***name, idx))
        .collect();
    // map each character type name to an index
    let chartype_nameidx: HashMap<&str, usize> = chartypenames
        .iter()
        .enumerate()
        .map(|(idx, name)| (&***name, idx))
        .collect();
    // map from element definition string to the variable name emitted for that definition
    let mut element_definitions: HashMap<String, String> = HashMap::new();
    // map each attribute definition string to the variable name emitted for that definition
    let mut attribute_definitions: HashMap<String, String> = HashMap::new();

    // empty element list and empty attribute list don't need a named variable, so are treated specially here
    element_definitions.insert("".to_string(), "[]".to_string());
    attribute_definitions.insert("".to_string(), "[]".to_string());

    let elem_is_ref = build_ref_element_list(autosar_schema, &elemtypenames);
    // build a list that tells which elements have a SHORT-NAME and in which versions this is the case
    let elem_is_named: Vec<usize> = build_named_element_list(autosar_schema, &elemtypenames);
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
        let mode = calc_element_mode(elemtype);

        let (subelem_limit_low, subelem_limit_high) =
            subelements_index_info.get(*etypename).unwrap();
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
        writeln!(
            elemtypes,
            "    /* {idx:4} */ ElementSpec {{sub_elements: ({subelem_limit_low}, {subelem_limit_high}), \
                            sub_element_ver: {subelement_ver_info_low}, \
                            attributes: ({attrs_limit_low}, {attrs_limit_high}), attributes_ver: {attrs_ver_info_low}, \
                            character_data: {chartype}, mode: {mode}, is_named: 0x{:x}, is_ref: {}}}, // {infostring}",
            elem_is_named[idx], elem_is_ref[idx]
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

    Ok(generated)
}

fn build_named_element_list(
    autosar_schema: &AutosarDataTypes,
    elemtypenames: &[&String],
) -> Vec<usize> {
    elemtypenames
        .iter()
        .map(|name| {
            if let Some(ec) = autosar_schema
                .element_types
                .get(*name)
                .unwrap()
                .collection()
            {
                if let Some(ElementCollectionItem::Element(sn)) = ec
                    .items()
                    .iter()
                    .find(|sub_elem| sub_elem.name() == "SHORT-NAME")
                {
                    sn.version_info
                } else {
                    0
                }
            } else {
                0
            }
        })
        .collect()
}

fn build_ref_element_list(
    autosar_schema: &AutosarDataTypes,
    elemtypenames: &[&String],
) -> Vec<bool> {
    elemtypenames
        .iter()
        .map(|name| {
            if let ElementDataType::Characters { basetype, .. } =
                autosar_schema.element_types.get(*name).unwrap()
            {
                basetype == "AR:REF--SIMPLE"
            } else {
                false
            }
        })
        .collect()
}

fn build_elementnames_of_type_list(
    autosar_schema: &AutosarDataTypes,
) -> HashMap<String, HashSet<String>> {
    let mut map = HashMap::with_capacity(autosar_schema.element_types.len());

    map.insert("AR:AUTOSAR".to_string(), HashSet::new());
    map.get_mut("AR:AUTOSAR")
        .unwrap()
        .insert("AUTOSAR".to_string());

    for definition in autosar_schema.element_types.values() {
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

fn build_sub_elements_string(
    sub_elements: &[ElementCollectionItem],
    elemtype_nameidx: &HashMap<&str, usize>,
) -> String {
    let mut sub_element_strings: Vec<String> = Vec::new();
    for ec_item in sub_elements {
        match ec_item {
            ElementCollectionItem::Element(elem) => {
                sub_element_strings.push(
                    format!("    SubElement::Element{{name: ElementName::{}, elemtype: {}, multiplicity: ElementMultiplicity::{:?}}}",
                        name_to_identifier(&elem.name),
                        elemtype_nameidx.get(&*elem.typeref).unwrap(),
                        elem.amount
                    )
                );
            }
            ElementCollectionItem::GroupRef(group) => {
                sub_element_strings.push(format!(
                    "    SubElement::Group{{groupid: {}}}",
                    elemtype_nameidx.get(&**group).unwrap()
                ));
            }
        }
    }
    sub_element_strings.join(",\n")
}

fn build_attributes_string(attrs: &[Attribute], chartype_nameidx: &HashMap<&str, usize>) -> String {
    let mut attr_strings = Vec::new();
    for attr in attrs {
        let chartype = format!("{}", *chartype_nameidx.get(&*attr.attribute_type).unwrap());

        attr_strings.push(format!(
            "    (AttributeName::{}, {chartype}, {})",
            name_to_identifier(&attr.name),
            attr.required,
        ));
    }
    attr_strings.join(",\n")
}

fn calc_element_mode(elemtype: &ElementDataType) -> &'static str {
    match elemtype {
        ElementDataType::ElementsGroup { element_collection }
        | ElementDataType::Elements {
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
        ElementDataType::Characters { .. } => "ContentMode::Characters",
        ElementDataType::Mixed { .. } => "ContentMode::Mixed",
    }
}

fn cmp_elemtypenames_subelems(
    k1: &str,
    k2: &str,
    elemtypes: &HashMap<String, ElementDataType>,
) -> std::cmp::Ordering {
    let len1 = elemtypes
        .get(k1)
        .and_then(|e| e.collection())
        .map_or(0, |ec| ec.items().len());
    let len2 = elemtypes
        .get(k2)
        .and_then(|e| e.collection())
        .map_or(0, |ec| ec.items().len());

    match len2.cmp(&len1) {
        std::cmp::Ordering::Less => std::cmp::Ordering::Less,
        std::cmp::Ordering::Equal => k1.cmp(k2),
        std::cmp::Ordering::Greater => std::cmp::Ordering::Greater,
    }
}

fn cmp_elemtypenames_attrs(
    k1: &str,
    k2: &str,
    elemtypes: &HashMap<String, ElementDataType>,
) -> std::cmp::Ordering {
    let len1 = elemtypes
        .get(k1)
        .and_then(|e| e.attributes())
        .map_or(0, |attrs| attrs.len());
    let len2 = elemtypes
        .get(k2)
        .and_then(|e| e.attributes())
        .map_or(0, |attrs| attrs.len());

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
    hashsize: usize,
    param1: usize,
    param2: usize,
) -> String {
    let mut generated = String::new();
    let (table1, table2) =
        perfect_hash::make_perfect_hash(item_names, param1, param2, hashsize).unwrap();

    let width = item_names.iter().map(|name| name.len()).max().unwrap();

    writeln!(
        generated,
        "use crate::hashfunc;

#[derive(Debug)]
/// The error type Parse{enum_name}Error is returned when from_str() / parse() fails for {enum_name}
pub struct Parse{enum_name}Error;
"
    )
    .unwrap();
    generated
        .write_str(
            "#[allow(dead_code, non_camel_case_types)]
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
#[repr(u16)]
",
        )
        .unwrap();
    writeln!(generated, "/// {enum_docstring}\npub enum {enum_name} {{").unwrap();
    for (idx, item_name) in item_names.iter().enumerate() {
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
    const STRING_TABLE: [&'static str; {length}] = {item_names:?};

    /// derive an enum entry from an input string using a perfect hash function
    pub fn from_bytes(input: &[u8]) -> Result<Self, Parse{enum_name}Error> {{
        // here, hashfunc(input, <param>) is an ordinary hash function which may produce collisions
        // it is possible to create two tables so that
        //     table1[hashfunc(input, param1)] + table2[hashfunc(input, param2)] == desired enumeration value
        // these tables are pre-built and included here as constants, since the values to be hashed don't change
        static HASH_TABLE_1: [u16; {hashsize}] = {table1:?};
        static HASH_TABLE_2: [u16; {hashsize}] = {table2:?};
        let hashval1: usize = hashfunc(input, {param1});
        let hashval2: usize = hashfunc(input, {param2});
        let val1 = HASH_TABLE_1[hashval1 % {hashsize}];
        let val2 = HASH_TABLE_2[hashval2 % {hashsize}];
        if val1 == u16::MAX || val2 == u16::MAX {{
            return Err(Parse{enum_name}Error);
        }}
        let item_idx = (val1 + val2) as usize % {length};
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
        r"^(%[ \-+#]?[0-9]*(\.[0-9]+)?[diouxXfeEgGcs])$",
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
