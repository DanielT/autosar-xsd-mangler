use rustc_hash::FxHashMap;
use std::env;
use std::fs::File;
use std::path::Path;

use xsd::Xsd;

mod flatten;
mod generator;
mod merge;
mod xsd;

#[derive(Debug, Clone, Eq, PartialEq)]
struct EnumDefinition {
    name: String,
    enumitems: Vec<(String, usize)>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct Attribute {
    pub(crate) name: String,
    pub(crate) attribute_type: String,
    pub(crate) required: bool,
    pub(crate) version_info: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct Element {
    pub(crate) name: String,
    pub(crate) typeref: String,
    pub(crate) amount: ElementAmount,
    pub(crate) version_info: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ElementAmount {
    ZeroOrOne,
    One,
    Any,
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum ElementCollectionItem {
    Element(Element),
    GroupRef(String),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum ElementCollection {
    Choice {
        name: String,
        sub_elements: Vec<ElementCollectionItem>,
        amount: ElementAmount,
    },
    Sequence {
        name: String,
        sub_elements: Vec<ElementCollectionItem>,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum ElementDataType {
    Elements {
        element_collection: ElementCollection,
        attributes: Vec<Attribute>,
    },
    Characters {
        attributes: Vec<Attribute>,
        basetype: String,
    },
    Mixed {
        element_collection: ElementCollection,
        attributes: Vec<Attribute>,
        basetype: String,
    },
    ElementsGroup {
        element_collection: ElementCollection,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum CharacterDataType {
    Pattern {
        pattern: String,
        max_length: Option<usize>,
    },
    Enum(EnumDefinition),
    String {
        max_length: Option<usize>,
        preserve_whitespace: bool,
    },
    UnsignedInteger,
    Double,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct AutosarDataTypes {
    element_types: FxHashMap<String, ElementDataType>,
    character_types: FxHashMap<String, CharacterDataType>,
}

pub(crate) struct XsdFileInfo {
    name: &'static str,
    ident: &'static str,
    desc: &'static str,
}

const XSD_CONFIG: [XsdFileInfo; 18] = [
    XsdFileInfo {
        name: "AUTOSAR_4-0-1.xsd",
        ident: "Autosar_4_0_1",
        desc: "AUTOSAR 4.0.1",
    },
    XsdFileInfo {
        name: "AUTOSAR_4-0-2.xsd",
        ident: "Autosar_4_0_2",
        desc: "AUTOSAR 4.0.2",
    },
    XsdFileInfo {
        name: "AUTOSAR_4-0-3.xsd",
        ident: "Autosar_4_0_3",
        desc: "AUTOSAR 4.0.3",
    },
    XsdFileInfo {
        name: "AUTOSAR_4-1-1.xsd",
        ident: "Autosar_4_1_1",
        desc: "AUTOSAR 4.1.1",
    },
    XsdFileInfo {
        name: "AUTOSAR_4-1-2.xsd",
        ident: "Autosar_4_1_2",
        desc: "AUTOSAR 4.1.2",
    },
    XsdFileInfo {
        name: "AUTOSAR_4-1-3.xsd",
        ident: "Autosar_4_1_3",
        desc: "AUTOSAR 4.1.3",
    },
    XsdFileInfo {
        name: "AUTOSAR_4-2-1.xsd",
        ident: "Autosar_4_2_1",
        desc: "AUTOSAR 4.2.1",
    },
    XsdFileInfo {
        name: "AUTOSAR_4-2-2.xsd",
        ident: "Autosar_4_2_2",
        desc: "AUTOSAR 4.2.2",
    },
    XsdFileInfo {
        name: "AUTOSAR_4-3-0.xsd",
        ident: "Autosar_4_3_0",
        desc: "AUTOSAR 4.3.0",
    },
    XsdFileInfo {
        name: "AUTOSAR_00042.xsd",
        ident: "Autosar_00042",
        desc: "AUTOSAR Adaptive 17-03",
    },
    XsdFileInfo {
        name: "AUTOSAR_00043.xsd",
        ident: "Autosar_00043",
        desc: "AUTOSAR Adaptive 17-10",
    },
    XsdFileInfo {
        name: "AUTOSAR_00044.xsd",
        ident: "Autosar_00044",
        desc: "AUTOSAR Classic 4.3.1",
    },
    XsdFileInfo {
        name: "AUTOSAR_00045.xsd",
        ident: "Autosar_00045",
        desc: "AUTOSAR Adaptive 18-03",
    },
    XsdFileInfo {
        name: "AUTOSAR_00046.xsd",
        ident: "Autosar_00046",
        desc: "AUTOSAR Classic 4.4.0 / Adaptive 18-10",
    },
    XsdFileInfo {
        name: "AUTOSAR_00047.xsd",
        ident: "Autosar_00047",
        desc: "AUTOSAR Adaptive 19-03",
    },
    XsdFileInfo {
        name: "AUTOSAR_00048.xsd",
        ident: "Autosar_00048",
        desc: "AUTOSAR 4.5.0",
    },
    XsdFileInfo {
        name: "AUTOSAR_00049.xsd",
        ident: "Autosar_00049",
        desc: "AUTOSAR 4.6.0",
    },
    XsdFileInfo {
        name: "AUTOSAR_00050.xsd",
        ident: "Autosar_00050",
        desc: "AUTOSAR 4.7.0",
    },
];

fn core() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        println!("usage: {} <input xsd path>", &args[0]);
        std::process::exit(1);
    }

    let path = Path::new(&args[1]);
    if !path.exists() {
        println!("Error: path \"{}\" does not exist.", &args[1]);
        println!("usage: {} <xsd path>", &args[0]);
        std::process::exit(2);
    }

    let mut autosar_schema_version = Vec::new();
    for (index, xsd_file_info) in XSD_CONFIG.iter().enumerate() {
        let filepath = path.join(Path::new(xsd_file_info.name));
        if filepath.exists() {
            let file = File::open(filepath).unwrap();
            println!("loading {}", xsd_file_info.name);
            let xsd = Xsd::load(file, 1 << index)?;
            // println!("\n\n######################\nXSD {friendly_name}:\n{xsd:#?}\n##################\n\n");
            autosar_schema_version.push((xsd_file_info.desc, flatten::flatten_schema(&xsd)?));
        } else {
            println!(
                "Error: XSD file \"{}\" for the standard {} was not found",
                filepath.to_string_lossy(),
                xsd_file_info.desc
            );
        }
    }

    let (_base_name, mut autosar_schema) = autosar_schema_version.pop().unwrap();
    //let mut merged = HashMap::new();
    sanity_check(&autosar_schema);

    println!("merge base: {}", _base_name);
    for (_input_name, xsd) in autosar_schema_version.iter().rev() {
        println!("merging: {}", _input_name);
        merge::merge(&mut autosar_schema, xsd)?;
        sanity_check(&autosar_schema);
    }

    dedup_types(&mut autosar_schema);
    sanity_check(&autosar_schema);

    generator::generate(&XSD_CONFIG, &autosar_schema)?;

    Ok(())
}

fn dedup_types(autosar_types: &mut AutosarDataTypes) {
    // println!("before dedup: {} element types, {} character types", autosar_types.element_types.len(), autosar_types.character_types.len());
    loop {
        let mut elem_typenames = autosar_types
            .element_types
            .keys()
            .map(|k| k.to_owned())
            .collect::<Vec<String>>();
        elem_typenames.sort_by(dedup_keycmp);
        let mut char_typenames = autosar_types
            .character_types
            .keys()
            .map(|k| k.to_owned())
            .collect::<Vec<String>>();
        char_typenames.sort_by(dedup_keycmp);

        let mut elem_replacements = FxHashMap::default();
        let mut char_replacements = FxHashMap::default();

        for idx1 in 0..(elem_typenames.len() - 1) {
            let typename1 = &elem_typenames[idx1];
            if elem_replacements.get(typename1).is_none() {
                for typename2 in elem_typenames.iter().skip(idx1 + 1) {
                    if elem_replacements.get(typename2).is_none()
                        && autosar_types.element_types.get(typename1)
                            == autosar_types.element_types.get(typename2)
                    {
                        elem_replacements.insert(typename2.to_owned(), typename1.to_owned());
                    }
                }
            }
        }
        for idx1 in 0..(char_typenames.len() - 1) {
            let typename1 = &char_typenames[idx1];
            if char_replacements.get(typename1).is_none() {
                for typename2 in char_typenames.iter().skip(idx1 + 1) {
                    if char_replacements.get(typename2).is_none()
                        && autosar_types.character_types.get(typename1)
                            == autosar_types.character_types.get(typename2)
                    {
                        char_replacements.insert(typename2.to_owned(), typename1.to_owned());
                    }
                }
            }
        }

        for (_, artype) in autosar_types.element_types.iter_mut() {
            match artype {
                ElementDataType::Elements {
                    element_collection, ..
                }
                | ElementDataType::Mixed {
                    element_collection, ..
                }
                | ElementDataType::ElementsGroup { element_collection } => match element_collection
                {
                    ElementCollection::Choice { sub_elements, .. }
                    | ElementCollection::Sequence { sub_elements, .. } => {
                        for ec_item in sub_elements {
                            match ec_item {
                                ElementCollectionItem::Element(Element { typeref, .. })
                                | ElementCollectionItem::GroupRef(typeref) => {
                                    if let Some(rep) = elem_replacements.get(typeref) {
                                        *typeref = rep.to_owned();
                                    }
                                }
                            }
                        }
                    }
                },
                _ => {}
            }
            match artype {
                ElementDataType::Elements { attributes, .. }
                | ElementDataType::Characters { attributes, .. }
                | ElementDataType::Mixed { attributes, .. } => {
                    for attr in attributes {
                        if let Some(rep) = char_replacements.get(&attr.attribute_type) {
                            attr.attribute_type = rep.to_owned();
                        }
                    }
                }
                _ => {}
            }
            match artype {
                ElementDataType::Characters { basetype, .. }
                | ElementDataType::Mixed { basetype, .. } => {
                    if let Some(rep) = char_replacements.get(basetype) {
                        *basetype = rep.to_owned();
                    }
                }
                _ => {}
            }
        }
        for name in elem_replacements.keys() {
            autosar_types.element_types.remove(name);
        }
        for name in char_replacements.keys() {
            autosar_types.character_types.remove(name);
        }

        if elem_replacements.is_empty() && char_replacements.is_empty() {
            break;
        }
    }
    // println!("after dedup: {} element types, {} character types", autosar_types.element_types.len(), autosar_types.character_types.len());
}

fn dedup_keycmp(key1: &String, key2: &String) -> std::cmp::Ordering {
    match key1.len().cmp(&key2.len()) {
        std::cmp::Ordering::Equal => key1.cmp(key2),
        nonequal => nonequal,
    }
}

fn sanity_check(autosar_types: &AutosarDataTypes) {
    for (typename, elemcontent) in &autosar_types.element_types {
        if let Some(element_collection) = elemcontent.collection() {
            for item in element_collection.items() {
                if let ElementCollectionItem::Element(elem) = item {
                    if autosar_types.element_types.get(&elem.typeref).is_none() {
                        println!("sanity check failed - in type [{typename}] element <{elem:#?}> references non-existent type [{}]", elem.typeref);
                    }
                }
            }
        }
        if let Some(attributes) = elemcontent.attributes() {
            for attr in attributes {
                if autosar_types
                    .character_types
                    .get(&attr.attribute_type)
                    .is_none()
                {
                    println!(
                        "sanity check failed - in type [{typename}] attribute {} references non-existent type [{}]",
                        attr.name, attr.attribute_type
                    );
                }
            }
        }
    }
}

fn main() {
    match core() {
        Ok(()) => {}
        Err(errmsg) => {
            print!("{}", errmsg);
        }
    }
}

impl ElementDataType {
    fn collection(&self) -> Option<&ElementCollection> {
        match self {
            ElementDataType::ElementsGroup { element_collection }
            | ElementDataType::Elements {
                element_collection, ..
            }
            | ElementDataType::Mixed {
                element_collection, ..
            } => Some(element_collection),
            _ => None,
        }
    }

    fn attributes(&self) -> Option<&Vec<Attribute>> {
        match self {
            ElementDataType::Elements { attributes, .. }
            | ElementDataType::Characters { attributes, .. }
            | ElementDataType::Mixed { attributes, .. } => Some(attributes),
            ElementDataType::ElementsGroup { .. } => None,
        }
    }

    fn basetype(&self) -> Option<&str> {
        match self {
            ElementDataType::Characters { basetype, .. } => Some(basetype),
            ElementDataType::Mixed { basetype, .. } => Some(basetype),
            _ => None,
        }
    }
}

impl ElementCollection {
    fn items(&self) -> &Vec<ElementCollectionItem> {
        match self {
            ElementCollection::Choice { sub_elements, .. } => sub_elements,
            ElementCollection::Sequence { sub_elements, .. } => sub_elements,
        }
    }
}

impl ElementCollectionItem {
    fn name(&self) -> &str {
        match self {
            ElementCollectionItem::Element(Element { name, .. }) => name,
            ElementCollectionItem::GroupRef(name) => name,
        }
    }
}

impl AutosarDataTypes {
    fn new() -> Self {
        let mut adt = Self {
            character_types: FxHashMap::default(),
            element_types: FxHashMap::default(),
        };

        adt.character_types.insert(
            "xsd:string".to_string(),
            CharacterDataType::String {
                max_length: None,
                preserve_whitespace: false,
            },
        );
        adt.character_types.insert(
            "xsd:NMTOKEN".to_string(),
            CharacterDataType::String {
                max_length: None,
                preserve_whitespace: false,
            },
        );
        adt.character_types.insert(
            "xsd:NMTOKENS".to_string(),
            CharacterDataType::String {
                max_length: None,
                preserve_whitespace: false,
            },
        );
        adt.character_types.insert(
            "xsd:unsignedInt".to_string(),
            CharacterDataType::UnsignedInteger,
        );
        adt.character_types
            .insert("xsd:double".to_string(), CharacterDataType::Double);

        adt
    }
}
