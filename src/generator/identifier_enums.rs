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
                element_names.insert(elem.name.clone());
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
    let disps = perfect_hash::make_perfect_hash(&element_name_refs, 7)
        .map_err(|err| format!("element names: {err}"))
        .unwrap();
    // element names use alphabetically sorted discriminants, so that the specification crate
    // can binary search the (alphabetically sorted) sub elements of Bag-type elements by comparing discriminants
    let enumstr = generate_enum(
        "ElementName",
        "Enum of all element names in Autosar",
        &element_name_refs,
        &disps,
        true,
    );

    let mut file = File::create("gen/elementname.rs").unwrap();
    file.write_all(enumstr.as_bytes()).unwrap();

    let attribute_name_refs: Vec<&str> = attribute_names.iter().map(|name| &**name).collect();
    let disps = perfect_hash::make_perfect_hash(&attribute_name_refs, 5)
        .map_err(|err| format!("attribute names: {err}"))
        .unwrap();
    let enumstr = generate_enum(
        "AttributeName",
        "Enum of all attribute names in Autosar",
        &attribute_name_refs,
        &disps,
        false,
    );
    let mut file = File::create("gen/attributename.rs").unwrap();
    file.write_all(enumstr.as_bytes()).unwrap();

    let enum_item_refs: Vec<&str> = enum_items.iter().map(|name| &**name).collect();
    let disps = perfect_hash::make_perfect_hash(&enum_item_refs, 5)
        .map_err(|err| format!("enum item names: {err}"))
        .unwrap();

    let enumstr = generate_enum(
        "EnumItem",
        "Enum of all possible enum values in Autosar",
        &enum_item_refs,
        &disps,
        false,
    );
    let mut file = File::create("gen/enumitem.rs").unwrap();
    file.write_all(enumstr.as_bytes()).unwrap();
}

fn generate_enum(
    enum_name: &str,
    enum_docstring: &str,
    item_names: &[&str],
    disps: &[(u32, u32)],
    sorted_discriminants: bool,
) -> String {
    // with sorted_discriminants, the discriminant of each item is its position in the sorted list of names,
    // and an additional table maps the index produced by the perfect hash to the discriminant.
    // Otherwise the discriminant is the index produced by the perfect hash.
    assert!(item_names.is_sorted());
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
    generated.push_str("#[rustfmt::skip]\n");
    generated.push_str(&format!("/// {enum_docstring}\npub enum {enum_name} {{\n"));
    let mut hash_sorted_item_names = item_names.to_owned();
    hash_sorted_item_names.sort_by(|k1, k2| {
        perfect_hash::get_index(k1, disps, item_names.len()).cmp(&perfect_hash::get_index(
            k2,
            disps,
            item_names.len(),
        ))
    });
    for (pos, item_name) in item_names.iter().enumerate() {
        let idx = if sorted_discriminants {
            pos
        } else {
            perfect_hash::get_index(item_name, disps, item_names.len())
        };
        let ident = name_to_identifier(item_name);
        writeln!(generated, "    /// {item_name}").unwrap();
        writeln!(generated, "    {ident:width$}= {idx},").unwrap();
    }
    writeln!(generated, "}}").unwrap();

    let length = item_names.len();
    let discriminant_order = if sorted_discriminants {
        item_names
    } else {
        &hash_sorted_item_names
    };
    // all names are concatenated into one string; STRING_OFFSETS[i]..STRING_OFFSETS[i + 1] is the name of item i.
    // Compared to an array of &str this avoids the overhead of a fat pointer (and a relocation) per name.
    let mut offsets = vec![0];
    let mut total_len = 0;
    let mut string_data = String::new();
    for name in discriminant_order {
        assert!(name.is_ascii());
        total_len += name.len();
        offsets.push(total_len);
        writeln!(string_data, "    {name:?},").unwrap();
    }
    let offset_type = if u16::try_from(total_len).is_ok() {
        "u16"
    } else {
        "u32"
    };
    let offsets_len = offsets.len();
    let mut offset_data = String::new();
    for chunk in offsets.chunks(16) {
        let chunk_strs: Vec<String> = chunk.iter().map(ToString::to_string).collect();
        writeln!(offset_data, "    {},", chunk_strs.join(", ")).unwrap();
    }

    let slot_mapping = if sorted_discriminants {
        let mut slot_to_item = vec![0; length];
        for (pos, name) in item_names.iter().enumerate() {
            slot_to_item[perfect_hash::get_index(name, disps, length)] = pos;
        }
        let mut slot_data = String::new();
        for chunk in slot_to_item.chunks(16) {
            let chunk_strs: Vec<String> = chunk.iter().map(ToString::to_string).collect();
            writeln!(slot_data, "            {},", chunk_strs.join(", ")).unwrap();
        }
        format!(
            "        // map the hash slot to the discriminant
        #[rustfmt::skip]
        static SLOT_TO_ITEM: [u16; {length}] = [
{slot_data}        ];
        let item_idx = SLOT_TO_ITEM[item_idx] as usize;
"
        )
    } else {
        String::new()
    };

    generated.push_str(&format!(
        r##"
#[rustfmt::skip]
static STRING_DATA: &str = concat!(
{string_data});

#[rustfmt::skip]
static STRING_OFFSETS: [{offset_type}; {offsets_len}] = [
{offset_data}];

impl {enum_name} {{
    /// derive an enum entry from an input string using a perfect hash function
    ///
    /// # Errors
    ///
    /// [`Parse{enum_name}Error`]: The input string did not match the name of any enum item
    pub fn from_bytes(input: &[u8]) -> Result<Self, Parse{enum_name}Error> {{
        #[rustfmt::skip]
        static DISPLACEMENTS: [(u16, u16); {displen}] = {disps:?};

        let (g, f1, f2) = hashfunc(input);
        let (d1, d2) = DISPLACEMENTS[(g % {displen}) as usize];
        let item_idx = u32::from(d2)
            .wrapping_add(f1.wrapping_mul(u32::from(d1)))
            .wrapping_add(f2) as usize
            % {length};
{slot_mapping}        if Self::name_bytes(item_idx) != input {{
            return Err(Parse{enum_name}Error);
        }}
        Ok(unsafe {{ core::mem::transmute::<u16, Self>(item_idx as u16) }})
    }}

    fn name_bytes(item_idx: usize) -> &'static [u8] {{
        let start = STRING_OFFSETS[item_idx] as usize;
        let end = STRING_OFFSETS[item_idx + 1] as usize;
        &STRING_DATA.as_bytes()[start..end]
    }}

    /// get the str corresponding to an item
    ///
    /// The returned &str has static lifetime, becasue it is a reference to an entry in a list of constants
    #[must_use]
    pub fn to_str(&self) -> &'static str {{
        // all names are ASCII, so every offset is a char boundary
        let start = STRING_OFFSETS[*self as usize] as usize;
        let end = STRING_OFFSETS[*self as usize + 1] as usize;
        &STRING_DATA[start..end]
    }}
}}

impl core::str::FromStr for {enum_name} {{
    type Err = Parse{enum_name}Error;
    fn from_str(input: &str) -> Result<Self, Self::Err> {{
        Self::from_bytes(input.as_bytes())
    }}
}}

impl core::fmt::Debug for {enum_name} {{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {{
        f.write_str(self.to_str())
    }}
}}

impl core::fmt::Display for {enum_name} {{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {{
        f.write_str(self.to_str())
    }}
}}
"##
    ));

    generated
}
