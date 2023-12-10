use crate::generator::{name_to_identifier, perfect_hash};
use crate::{AutosarDataTypes, CharacterDataType, ElementCollectionItem, HashSet};
use std::fmt::Write;
use std::fs::File;
use std::io::Write as IoWrite;

pub(crate) fn generate(autosar_schema: &AutosarDataTypes) {
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
