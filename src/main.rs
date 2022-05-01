use std::collections::{HashSet, HashMap};
use std::env;
use std::path::Path;
use std::fs::File;

use xsd::Xsd;

mod xsd;
mod flatten;
mod merge;
mod generator;

#[derive(Debug, Clone)]
struct EnumDefinition {
    name: String,
    enumitems: Vec<(String, usize)>
}

#[derive(Debug, Clone)]
pub(crate) enum AttributeType {
    Basic(String),
    Pattern {
        typename: String,
        pattern: String,
        maxlength: Option<usize>
    },
    Enum(String)
}

#[derive(Debug, Clone)]
pub(crate) struct Attribute {
    pub(crate) name: String,
    pub(crate) attribute_type: AttributeType,
    pub(crate) required: bool,
    pub(crate) version_info: usize
}

#[derive(Debug, Clone)]
pub(crate) struct Element {
    pub(crate) name: String,
    pub(crate) typeref: String,
    pub(crate) amount: ElementAmount,
    pub(crate) version_info: usize
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ElementAmount {
    ZeroOrOne,
    One,
    Any
}


#[derive(Debug, Clone)]
enum ElementCollectionItem {
    Element(Element),
    GroupRef(String),
}

#[derive(Debug, Clone)]
pub(crate) enum ElementCollection {
    Choice {
        name: String,
        sub_elements: Vec<ElementCollectionItem>,
        amount: ElementAmount
    },
    Sequence {
        name: String,
        sub_elements: Vec<ElementCollectionItem>
    },
}

#[derive(Debug, Clone)]
pub(crate) enum DataType {
    Elements {
        element_collection: ElementCollection,
        attributes: Vec<Attribute>
    },
    Characters {
        basetype: String,
        restriction_pattern: Option<String>,
        max_length: Option<usize>,
        attributes: Vec<Attribute>
    },
    Mixed {
        element_collection: ElementCollection,
        attributes: Vec<Attribute>,
        basetype: String,
    },
    Enum(EnumDefinition),
    ElementsGroup {
        element_collection: ElementCollection,
    }
}



const XSD_CONFIG: [(&'static str, &'static str); 18] = [
    ("AUTOSAR_4-0-1.xsd", "AUTOSAR 4.0.1"),
    ("AUTOSAR_4-0-2.xsd", "AUTOSAR 4.0.2"),
    ("AUTOSAR_4-0-3.xsd", "AUTOSAR 4.0.3"),
    ("AUTOSAR_4-1-1.xsd", "AUTOSAR 4.1.1"),
    ("AUTOSAR_4-1-2.xsd", "AUTOSAR 4.1.2"),
    ("AUTOSAR_4-1-3.xsd", "AUTOSAR 4.1.3"),
    ("AUTOSAR_4-2-1.xsd", "AUTOSAR 4.2.1"),
    ("AUTOSAR_4-2-2.xsd", "AUTOSAR 4.2.2"),
    ("AUTOSAR_4-3-0.xsd", "AUTOSAR 4.3.0"),
    ("AUTOSAR_00042.xsd", "AUTOSAR Adaptive 17-03"),
    ("AUTOSAR_00043.xsd", "AUTOSAR Adaptive 17-10"),
    ("AUTOSAR_00044.xsd", "AUTOSAR Classic 4.3.1"),
    ("AUTOSAR_00045.xsd", "AUTOSAR Adaptive 18-03"),
    ("AUTOSAR_00046.xsd", "AUTOSAR Classic 4.4.0 / Adaptive 18-10"),
    ("AUTOSAR_00047.xsd", "AUTOSAR Adaptive 19-03"),
    ("AUTOSAR_00048.xsd", "AUTOSAR 4.5.0"),
    ("AUTOSAR_00049.xsd", "AUTOSAR 4.6.0"),
    ("AUTOSAR_00050.xsd", "AUTOSAR 4.7.0")
];


fn core() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        println!("usage: {} <xsd path>", &args[0]);
        std::process::exit(1);
    }

    let path = Path::new(&args[1]);
    if !path.exists() {
        println!("Error: path \"{}\" does not exist.", &args[1]);
        println!("usage: {} <xsd path>", &args[0]);
        std::process::exit(2);
    }

    let mut xsd_desc = Vec::new();
    for (index, (filename, friendly_name)) in XSD_CONFIG.iter().enumerate() {
        let filepath = path.join(Path::new(filename));
        if filepath.exists() {
            let file = File::open(filepath).unwrap();
            println!("loading {}", filename);
            let xsd = Xsd::load(file, 1 << index)?;
            // println!("\n\n######################\nXSD {friendly_name}:\n{xsd:#?}\n##################\n\n");
            xsd_desc.push((friendly_name, flatten::flatten_schema(&xsd)?));
        } else {
            println!("Error: XSD file \"{}\" for the standard {} was not found", filepath.to_string_lossy(), friendly_name);
        }
    }

    let (_base_name, mut merged) = xsd_desc.pop().unwrap();
    sanity_check(&merged);

    // println!("merge base: {}", _base_name);
    for (_input_name, xsd) in xsd_desc.iter().rev() {
        // println!("----------- merging: {} ---------------", _input_name);
        merge::merge(&mut merged, &xsd)?;
        sanity_check(&merged);
    }


    // println!("\n\n-----------post merge -----------\n\n");
    // let mut keys = merged.keys().collect::<Vec<&String>>();
    // keys.sort();
    // for ct_key in keys {
    //     println!("{}: {:#?}", ct_key, merged.get(ct_key).unwrap());
    // }
    //println!("\n------------------------------\n{:#?}\n------------------------------\n", merged);
   
    generator::generate(&merged)?;

    Ok(())
}


fn sanity_check(merged: &HashMap<String, DataType>) {
    for (typename, elemcontent) in merged {
        if let Some(element_collection) = elemcontent.collection() {
            for item in element_collection.items() {
                if let ElementCollectionItem::Element(elem) = item {
                    if merged.get(&elem.typeref).is_none() {
                        println!("sanity check failed - in type [{}] element <{}> references non-existent type [{}]", typename, elem.name, elem.typeref);
                    }    
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


impl DataType {
    fn collection(&self) -> Option<&ElementCollection> {
        match self {
            DataType::ElementsGroup { element_collection } |
            DataType::Elements { element_collection, .. } |
            DataType::Mixed { element_collection, .. } => Some(element_collection),
            _ => None,
        }
    }

    fn attributes(&self) -> Option<&Vec<Attribute>> {
        match self {
            DataType::Elements { attributes, .. } |
            DataType::Characters { attributes, .. } |
            DataType::Mixed { attributes, .. } => Some(attributes),
            DataType::Enum(_) => None,
            DataType::ElementsGroup { .. } => None,
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
            ElementCollectionItem::Element ( Element { name, ..} ) => name,
            ElementCollectionItem::GroupRef (name ) => name,
        }
    }
}
