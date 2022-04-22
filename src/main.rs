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
        pattern: String,
        maxlength: Option<usize>
    },
    Enum(EnumDefinition)
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
pub(crate) enum ElementCollection {
    Choice {
        name: String,
        sub_elements: Vec<ElementCollection>,
        amount: ElementAmount
    },
    Sequence {
        name: String,
        sub_elements: Vec<ElementCollection>
    },
    Element(Element)
}

#[derive(Debug)]
pub(crate) struct ElementCollectionIter<'a> {
    iter_stack: Vec<std::slice::Iter<'a, ElementCollection>>,
    single_item: Option<&'a Element>
}

#[derive(Debug, Clone)]
pub(crate) enum ElementContent {
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
    Enum(EnumDefinition)
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


fn sanity_check(merged: &HashMap<String, ElementContent>) {
    for (typename, elemcontent) in merged {
        match elemcontent {
            ElementContent::Elements { element_collection, .. } |
            ElementContent::Mixed { element_collection, .. } => {
                for elem in element_collection {
                    if merged.get(&elem.typeref).is_none() {
                        println!("sanity check failed - in type [{}] element <{}> references non-existent type [{}]", typename, elem.name, elem.typeref);
                    }                    
                }
            }
            _ => {},
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


impl<'a> Iterator for ElementCollectionIter<'a> {
    type Item = &'a Element;

    fn next(&mut self) -> Option<Self::Item> {
        if self.single_item.is_some() {
            let out = self.single_item;
            self.single_item = None;
            out
        } else if !self.iter_stack.is_empty() {
            loop {
                match self.iter_stack.last_mut().unwrap().next() {
                    Some(ElementCollection::Choice { sub_elements, .. }) |
                    Some(ElementCollection::Sequence { sub_elements, .. }) => {
                        self.iter_stack.push(sub_elements.iter())
                    }
                    Some(ElementCollection::Element(elem)) => {
                        break Some(elem)
                    }
                    None => {
                        self.iter_stack.pop();
                        if self.iter_stack.is_empty() {
                            break None
                        }
                    }
                }
            }
        } else {
            None
        }
    }
}

impl<'a> IntoIterator for &'a ElementCollection {
    type Item = &'a Element;

    type IntoIter = ElementCollectionIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        match &self {
            ElementCollection::Choice { sub_elements, .. } |
            ElementCollection::Sequence { sub_elements, .. } => {
                Self::IntoIter {
                    iter_stack: vec![sub_elements.iter()],
                    single_item: None
                }
            }
            ElementCollection::Element(elem) => {
                Self::IntoIter {
                    iter_stack: vec![],
                    single_item: Some(elem)
                } 
            }
        }
    }
}

impl ElementCollection {
    fn name(&self) -> &str {
        match self {
            ElementCollection::Choice { name, .. } => name,
            ElementCollection::Sequence { name, .. } => name,
            ElementCollection::Element(Element {name, ..}) => name
        }
    }
}

#[test]
fn element_collection_iter_test() {
    let ec1 = ElementCollection::Sequence { name: "foo".to_string(), sub_elements: vec![] };
    assert_eq!(ec1.into_iter().count(), 0);

    let ec2 = ElementCollection::Sequence { name: "foo".to_string(), sub_elements: vec![
        ElementCollection::Choice {
            name: "FOO".to_string(),
            sub_elements: vec![
                ElementCollection::Element(
                    Element { name: "Elem1".to_string(), amount: ElementAmount::One, typeref: "".to_string(), version_info: 0}
                ),
                ElementCollection::Element(
                    Element { name: "Elem2".to_string(), amount: ElementAmount::One, typeref: "".to_string(), version_info: 0}
                )
            ],
            amount: ElementAmount::Any
        },
        ElementCollection::Sequence {
            name: "FOO".to_string(),
            sub_elements: vec![
                ElementCollection::Element(
                    Element { name: "Elem3".to_string(), amount: ElementAmount::One, typeref: "".to_string(), version_info: 0}
                ),
                ElementCollection::Element(
                    Element { name: "Elem4".to_string(), amount: ElementAmount::One, typeref: "".to_string(), version_info: 0}
                )
            ]
        },
    ]};
    assert_eq!(ec2.into_iter().count(), 4);
}