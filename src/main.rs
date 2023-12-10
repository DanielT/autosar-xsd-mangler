use rustc_hash::FxHashMap;
use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::path::Path;

use xsd::{Xsd, XsdRestrictToStandard};

mod dedup;
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
    pub(crate) attr_type: String,
    pub(crate) required: bool,
    pub(crate) version_info: usize,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub(crate) struct Element {
    pub(crate) name: String,
    pub(crate) typeref: String,
    pub(crate) amount: ElementAmount,
    pub(crate) version_info: usize,
    pub(crate) splittable: bool,
    pub(crate) ordered: bool,
    pub(crate) restrict_std: XsdRestrictToStandard,
    pub(crate) docstring: Option<String>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
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
        group_ref: String,
        attributes: Vec<Attribute>,
        xsd_typenames: HashSet<String>,
    },
    Characters {
        attributes: Vec<Attribute>,
        basetype: String,
    },
    Mixed {
        group_ref: String,
        attributes: Vec<Attribute>,
        basetype: String,
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
    group_types: FxHashMap<String, ElementCollection>,
}

pub(crate) struct XsdFileInfo {
    name: &'static str,
    ident: &'static str,
    desc: &'static str,
}

const XSD_CONFIG: [XsdFileInfo; 19] = [
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
        desc: "AUTOSAR R20-11",
    },
    XsdFileInfo {
        name: "AUTOSAR_00050.xsd",
        ident: "Autosar_00050",
        desc: "AUTOSAR R21-11",
    },
    XsdFileInfo {
        name: "AUTOSAR_00051.xsd",
        ident: "Autosar_00051",
        desc: "AUTOSAR R22-11",
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
            // println!("\n\n######################\nXSD {}:\n{xsd:#?}\n##################\n\n", xsd_file_info.desc);
            autosar_schema_version.push((xsd_file_info.desc, flatten::flatten_schema(&xsd)?));
        } else {
            println!(
                "Error: XSD file \"{}\" for the standard {} was not found",
                filepath.to_string_lossy(),
                xsd_file_info.desc
            );
        }
    }

    let (base_name, mut autosar_schema) = autosar_schema_version.pop().unwrap();
    //let mut merged = HashMap::new();
    sanity_check(&autosar_schema);

    println!("merge base: {base_name}");
    for (input_name, xsd) in autosar_schema_version.iter().rev() {
        println!("merging: {input_name}");
        merge::merge(&mut autosar_schema, xsd)?;
        sanity_check(&autosar_schema);
    }

    dedup::dedup_types(&mut autosar_schema);
    sanity_check(&autosar_schema);

    generator::generate(&XSD_CONFIG, &autosar_schema);

    Ok(())
}

fn sanity_check(autosar_types: &AutosarDataTypes) {
    for (groupname, group) in &autosar_types.group_types {
        for item in group.items() {
            if let ElementCollectionItem::Element(elem) = item {
                if autosar_types.element_types.get(&elem.typeref).is_none() {
                    println!("sanity check failed - in group [{groupname}] element <{elem:#?}> references non-existent type [{}]", elem.typeref);
                }
            }
        }
    }
    for (typename, elemcontent) in &autosar_types.element_types {
        if let Some(group_name) = elemcontent.group_ref() {
            if autosar_types.group_types.get(&group_name).is_none() {
                println!("sanity check failed - type [{typename}] references non-existent group [{group_name}]");
            }
        }
        for attr in elemcontent.attributes() {
            if autosar_types.character_types.get(&attr.attr_type).is_none() {
                println!(
                        "sanity check failed - in type [{typename}] attribute {} references non-existent type [{}]",
                        attr.name, attr.attr_type
                    );
            }
        }
    }
}

fn main() {
    match core() {
        Ok(()) => {}
        Err(errmsg) => {
            print!("{errmsg}");
        }
    }
}

impl ElementDataType {
    fn group_ref(&self) -> Option<String> {
        match self {
            ElementDataType::Elements { group_ref, .. }
            | ElementDataType::Mixed { group_ref, .. } => Some(group_ref.clone()),
            ElementDataType::Characters { .. } => None,
        }
    }

    fn attributes(&self) -> &Vec<Attribute> {
        match self {
            ElementDataType::Elements { attributes, .. }
            | ElementDataType::Characters { attributes, .. }
            | ElementDataType::Mixed { attributes, .. } => attributes,
        }
    }

    fn xsd_typenames(&self) -> Option<&HashSet<String>> {
        if let ElementDataType::Elements { xsd_typenames, .. } = self {
            Some(xsd_typenames)
        } else {
            None
        }
    }

    fn basetype(&self) -> Option<&str> {
        match self {
            ElementDataType::Characters { basetype, .. }
            | ElementDataType::Mixed { basetype, .. } => Some(basetype),
            _ => None,
        }
    }
}

impl ElementCollection {
    fn items(&self) -> &Vec<ElementCollectionItem> {
        match self {
            ElementCollection::Choice { sub_elements, .. }
            | ElementCollection::Sequence { sub_elements, .. } => sub_elements,
        }
    }
}

impl ElementCollectionItem {
    fn name(&self) -> &str {
        match self {
            ElementCollectionItem::Element(Element { name, .. })
            | ElementCollectionItem::GroupRef(name) => name,
        }
    }
}

impl AutosarDataTypes {
    fn new() -> Self {
        let mut adt = Self {
            character_types: FxHashMap::default(),
            element_types: FxHashMap::default(),
            group_types: FxHashMap::default(),
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
