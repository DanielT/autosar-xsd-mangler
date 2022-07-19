use std::io::BufReader;
use std::{collections::HashMap, fs::File};
use xml::{
    attribute::OwnedAttribute,
    common::{Position, TextPosition, XmlVersion},
    name::OwnedName,
    reader::{EventReader, XmlEvent},
};

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct XsdAttribute {
    pub(crate) name: String,
    pub(crate) typeref: String,
    pub(crate) required: bool,
}

#[derive(Debug)]
pub(crate) struct XsdAttributeGroup {
    pub(crate) attributes: Vec<XsdAttribute>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum XsdRestriction {
    EnumValues {
        enumvalues: Vec<String>,
    },
    Pattern {
        pattern: String,
        maxlength: Option<usize>,
    },
    Plain {
        basetype: String,
    },
    Literal,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct XsdExtension {
    pub(crate) basetype: String,
    pub(crate) attributes: Vec<XsdAttribute>,
    pub(crate) attribute_groups: Vec<String>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct XsdSimpleContent {
    pub(crate) extension: XsdExtension,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum XsdModelGroupItem {
    Group(String),
    //Sequence(Box<XsdSequence>),
    Choice(Box<XsdChoice>),
    Element(XsdElement),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct XsdSequence {
    pub(crate) items: Vec<XsdModelGroupItem>,
}
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct XsdChoice {
    pub(crate) min_occurs: usize,
    pub(crate) max_occurs: usize,
    pub(crate) items: Vec<XsdModelGroupItem>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum XsdGroupItem {
    Choice(XsdChoice),
    Sequence(XsdSequence),
    None,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct XsdGroup {
    pub(crate) item: XsdGroupItem,
}

#[derive(Debug, Eq, PartialEq, Hash)]
pub(crate) struct XsdElement {
    pub(crate) name: String,
    pub(crate) typeref: String,
    pub(crate) min_occurs: usize,
    pub(crate) max_occurs: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum XsdComplexTypeItem {
    SimpleContent(XsdSimpleContent),
    Group(String),
    Choice(XsdChoice),
    Sequence(XsdSequence),
    None,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct XsdComplexType {
    pub(crate) name: String,
    pub(crate) item: XsdComplexTypeItem,
    pub(crate) attribute_groups: Vec<String>,
    pub(crate) mixed_content: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum XsdSimpleType {
    Restriction(XsdRestriction),
    // Extension - this variant exists in the xsd specification, but is not used in the Autosar xsd files
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum XsdType {
    Base(String),
    Simple(XsdSimpleType),
    Complex(XsdComplexType),
}

#[derive(Debug)]
pub(crate) struct Xsd {
    pub(crate) root_elements: Vec<XsdElement>,
    pub(crate) groups: HashMap<String, XsdGroup>,
    pub(crate) types: HashMap<String, XsdType>,
    pub(crate) attribute_groups: HashMap<String, XsdAttributeGroup>,
    pub(crate) version_info: usize,
}

impl Xsd {
    pub(crate) fn load(file: File, version_info: usize) -> Result<Xsd, String> {
        let file = BufReader::new(file);
        let mut parser = EventReader::new(file);

        let mut data = Xsd {
            attribute_groups: HashMap::new(),
            groups: HashMap::new(),
            types: HashMap::new(),
            root_elements: Vec::new(),
            version_info,
        };
        // create the base type for the xml:space attribute directly instead of parsing xml.xsd
        data.types.insert(
            "XML:SPACE".to_string(),
            XsdType::Simple(XsdSimpleType::Restriction(XsdRestriction::EnumValues {
                enumvalues: vec!["default".to_string(), "preserve".to_string()],
            })),
        );
        data.types.insert(
            "xsd:string".to_string(),
            XsdType::Base("xsd:string".to_string()),
        );
        data.types.insert(
            "xsd:NMTOKEN".to_string(),
            XsdType::Base("xsd:NMTOKEN".to_string()),
        );
        data.types.insert(
            "xsd:NMTOKENS".to_string(),
            XsdType::Base("xsd:NMTOKENS".to_string()),
        );
        data.types.insert(
            "xsd:unsignedInt".to_string(),
            XsdType::Base("xsd:unsignedInt".to_string()),
        );
        data.types.insert(
            "xsd:double".to_string(),
            XsdType::Base("xsd:double".to_string()),
        );

        parse_schema(&mut parser, &mut data)?;

        Ok(data)
    }
}

fn parse_schema(parser: &mut EventReader<BufReader<File>>, data: &mut Xsd) -> Result<(), String> {
    let head = get_next_event(parser)?;
    if let XmlEvent::StartDocument {
        version: XmlVersion::Version10,
        ..
    } = head
    {
        // it's correct
    } else {
        return Err(format!(
            "Error: incorrect element at the start of the xsd document - found {:?}",
            head
        ));
    }

    let schema = get_next_event(parser)?;
    if let XmlEvent::StartElement {
        name: OwnedName { local_name, .. },
        ..
    } = schema
    {
        if local_name != "schema" {
            return Err(format!(
                "Error: not a valid xsd document, found element <{}> where <schema> was expected",
                local_name
            ));
        }
    } else {
        return Err(format!(
            "Error: not a valid xsd document. Found {:?} where element <schema> was expected",
            schema
        ));
    }

    while let Some((local_name, elem_attributes)) = get_next_element(parser, "schema")? {
        match local_name.as_ref() {
            "import" => {
                // no handling of imports, just consume the EndElement
                get_next_event(parser)?;
            }
            "group" => {
                parse_group(parser, data, &elem_attributes, vec![])?;
                // parse_group adds the parsed group to data.groups
            }
            "simpleType" => {
                parse_simple_type(parser, data, &elem_attributes)?;
                // parse_simple_type adds the parsed simpleType to data.types
            }
            "attributeGroup" => {
                parse_attribute_group(parser, data, &elem_attributes)?;
                // parse_attribute_group adds the parsed attributeGroup to data.attribute_groups
            }
            "complexType" => {
                parse_complex_type(parser, data, &elem_attributes, vec![])?;
                // parse_complex_type adds the parsed complexType to data.types
            }
            "element" => {
                let element = parse_element(parser, data, &elem_attributes, vec![])?;
                data.root_elements.push(element);
            }
            _ => {
                return Err(format!(
                    "Error: found unexpected start of element tag \"{}\" at {}",
                    local_name,
                    parser.position()
                ));
            }
        }
    }

    let end = get_next_event(parser)?;
    if let XmlEvent::EndDocument = end {
        Ok(())
    } else {
        Err(format!(
            "Error: found element {:?} when end of document was expected",
            end
        ))
    }
}

fn parse_element(
    parser: &mut EventReader<BufReader<File>>,
    data: &mut Xsd,
    attributes: &Vec<OwnedAttribute>,
    mut prev_names: Vec<String>,
) -> Result<XsdElement, String> {
    let attr_name = get_required_attribute_value("name", attributes, &parser.position())?;
    let attr_typeref = get_attribute_value("type", attributes);
    let attr_max_occurs = get_attribute_value("maxOccurs", attributes);
    let attr_min_occurs = get_attribute_value("minOccurs", attributes);

    let max_occurs = parse_occurs_attribute(&attr_max_occurs)?;
    let min_occurs = parse_occurs_attribute(&attr_min_occurs)?;

    if let Some(typeref) = attr_typeref {
        get_element_end_tag(parser, "element")?;
        Ok(XsdElement {
            name: attr_name.to_owned(),
            typeref: typeref.to_owned(),
            max_occurs,
            min_occurs,
        })
    } else {
        let mut typeref_opt = None;

        extend_prev_names(&mut prev_names, &Some(attr_name));

        while let Some((local_name, elem_attributes)) = get_next_element(parser, "element")? {
            match local_name.as_ref() {
                "annotation" => {
                    skip_annotation(parser)?;
                }
                "simpleType" => {
                    typeref_opt = Some(parse_simple_type(parser, data, &elem_attributes)?);
                }
                "complexType" => {
                    typeref_opt = Some(parse_complex_type(
                        parser,
                        data,
                        &elem_attributes,
                        prev_names.clone(),
                    )?);
                }
                _ => {
                    return Err(format!(
                        "Error: found unexpected start of element tag \"{}\" at {}",
                        local_name,
                        parser.position()
                    ));
                }
            }
        }

        if let Some(typeref) = typeref_opt {
            Ok(XsdElement {
                name: attr_name.to_owned(),
                typeref,
                max_occurs,
                min_occurs,
            })
        } else {
            Err(format!(
                "Error: missing type for element at {}",
                parser.position()
            ))
        }
    }
}

fn parse_group(
    parser: &mut EventReader<BufReader<File>>,
    data: &mut Xsd,
    attributes: &Vec<OwnedAttribute>,
    mut prev_names: Vec<String>,
) -> Result<String, String> {
    let attr_name = get_attribute_value("name", attributes);
    let attr_typeref = get_attribute_value("ref", attributes);

    if attr_name.is_some() && attr_typeref.is_some() {
        Err(format!(
            "Error: group at {} has both name and ref attributes",
            parser.position()
        ))
    } else if let Some(typeref) = attr_typeref {
        get_element_end_tag(parser, "group")?;
        Ok(typeref.to_owned())
    } else if let Some(name) = attr_name {
        let mut sequence: Option<XsdSequence> = None;
        let mut choice: Option<XsdChoice> = None;

        extend_prev_names(&mut prev_names, &Some(name));

        while let Some((local_name, elem_attributes)) = get_next_element(parser, "group")? {
            match local_name.as_ref() {
                "annotation" => {
                    skip_annotation(parser)?;
                }
                "sequence" => {
                    sequence = Some(parse_sequence(parser, data, prev_names.clone())?);
                }
                "choice" => {
                    choice = Some(parse_choice(
                        parser,
                        data,
                        &elem_attributes,
                        prev_names.clone(),
                    )?);
                }
                _ => {
                    return Err(format!(
                        "Error: found unexpected start of element tag \"{}\" at {}",
                        local_name,
                        parser.position()
                    ));
                }
            }
        }

        let item = match (choice, sequence) {
            (Some(_), Some(_)) => {
                return Err(format!(
                    "Error: group containing both sequence and choice (ends at {})",
                    parser.position()
                ))
            }
            (Some(choice), None) => XsdGroupItem::Choice(choice),
            (None, Some(seq)) => XsdGroupItem::Sequence(seq),
            (None, None) => XsdGroupItem::None,
        };

        let typeref = format!("AR:{}", name);
        data.groups.insert(typeref.clone(), XsdGroup { item });

        Ok(typeref)
    } else {
        Err(format!(
            "Error: group at {} has neither name nor ref attributes",
            parser.position()
        ))
    }
}

fn parse_simple_type(
    parser: &mut EventReader<BufReader<File>>,
    data: &mut Xsd,
    attributes: &Vec<OwnedAttribute>,
) -> Result<String, String> {
    let name = get_required_attribute_value("name", attributes, &parser.position())?;
    let mut restriction: Option<XsdRestriction> = None;

    while let Some((local_name, elem_attributes)) = get_next_element(parser, "simpleType")? {
        match local_name.as_ref() {
            "annotation" => {
                skip_annotation(parser)?;
            }
            "restriction" => {
                restriction = Some(parse_restriction(parser, &elem_attributes)?);
            }
            _ => {
                return Err(format!(
                    "Error: found unexpected start of element tag \"{}\" at {}",
                    local_name,
                    parser.position()
                ));
            }
        }
    }

    if let Some(restriction) = restriction {
        let nameref = format!("AR:{}", name);
        data.types.insert(
            nameref,
            XsdType::Simple(XsdSimpleType::Restriction(restriction)),
        );

        Ok(name.to_owned())
    } else {
        Err(format!(
            "Error: simpleType ending at {} contains no <restriction>",
            parser.position()
        ))
    }
}

fn parse_complex_type(
    parser: &mut EventReader<BufReader<File>>,
    data: &mut Xsd,
    attributes: &Vec<OwnedAttribute>,
    mut prev_names: Vec<String>,
) -> Result<String, String> {
    let attr_name = get_attribute_value("name", attributes);
    let mixed_content = get_attribute_value("mixed", attributes).unwrap_or("false") == "true";
    let mut item = XsdComplexTypeItem::None;
    let mut item_count = 0;
    let mut attribute_groups = Vec::new();

    let num_prev_names = prev_names.len();
    let name = if let Some(name) = attr_name {
        name.to_owned()
    } else if num_prev_names > 1 {
        format!(
            "{}-{}-TYPE",
            prev_names[num_prev_names - 2],
            prev_names[num_prev_names - 1]
        )
    } else if num_prev_names == 1 {
        format!("{}-TYPE", prev_names[0])
    } else {
        todo!()
    };

    extend_prev_names(&mut prev_names, &attr_name);

    while let Some((local_name, elem_attributes)) = get_next_element(parser, "complexType")? {
        match local_name.as_ref() {
            "annotation" => {
                skip_annotation(parser)?;
            }
            "simpleContent" => {
                item = XsdComplexTypeItem::SimpleContent(parse_simple_content(parser, data)?);
                item_count += 1;
            }
            "group" => {
                item = XsdComplexTypeItem::Group(parse_group(
                    parser,
                    data,
                    &elem_attributes,
                    prev_names.clone(),
                )?);
                item_count += 1;
            }
            "sequence" => {
                item =
                    XsdComplexTypeItem::Sequence(parse_sequence(parser, data, prev_names.clone())?);
                item_count += 1;
            }
            "choice" => {
                item = XsdComplexTypeItem::Choice(parse_choice(
                    parser,
                    data,
                    &elem_attributes,
                    prev_names.clone(),
                )?);
                item_count += 1;
            }
            "attributeGroup" => {
                attribute_groups.push(parse_attribute_group(parser, data, &elem_attributes)?);
                // any number of attribute groups allowed, without excluding any other items, so item_count is not incremented
            }
            _ => {
                return Err(format!(
                    "Error: found unexpected start of element tag \"{}\" at {}",
                    local_name,
                    parser.position()
                ));
            }
        }
        if item_count > 1 {
            return Err(format!(
                "Error: complexType has mutually exclusive child elements at {}",
                parser.position()
            ));
        }
    }

    let typeref = format!("AR:{}", name);
    let newtype = XsdType::Complex(XsdComplexType {
        name,
        item,
        attribute_groups,
        mixed_content,
    });

    if let Some(oldtype) = data.types.get(&typeref) {
        if *oldtype != newtype {
            println!("WARNING!! about to replace {oldtype:#?} by {newtype:#?}");
        }
    }

    data.types.insert(typeref.clone(), newtype);

    Ok(typeref)
}

fn parse_attribute_group(
    parser: &mut EventReader<BufReader<File>>,
    data: &mut Xsd,
    attributes: &Vec<OwnedAttribute>,
) -> Result<String, String> {
    let attr_name = get_attribute_value("name", attributes);
    let attr_ref = get_attribute_value("ref", attributes);

    if let Some(nameref) = attr_ref {
        // this attributeGroup element is a reference to a different declaration
        get_element_end_tag(parser, "attributeGroup")?;
        Ok(nameref.to_owned())
    } else if let Some(name) = attr_name {
        // a new attribute group is declared
        let mut attributes = Vec::new();

        while let Some((local_name, elem_attributes)) = get_next_element(parser, "attributeGroup")?
        {
            match local_name.as_ref() {
                "annotation" => {
                    skip_annotation(parser)?;
                }
                "attribute" => {
                    attributes.push(parse_attribute(parser, &elem_attributes)?);
                }
                _ => {
                    return Err(format!(
                        "Error: found unexpected start of element tag \"{}\" at {}",
                        local_name,
                        parser.position()
                    ));
                }
            }
        }

        let nameref = format!("AR:{}", name);
        data.attribute_groups
            .insert(nameref.clone(), XsdAttributeGroup { attributes });

        Ok(nameref)
    } else {
        Err(format!(
            "Error: attributeGroup at {} has neither name nor ref attributes",
            parser.position()
        ))
    }
}

fn parse_attribute(
    parser: &mut EventReader<BufReader<File>>,
    attributes: &Vec<OwnedAttribute>,
) -> Result<XsdAttribute, String> {
    let attr_name = get_attribute_value("name", attributes);
    let attr_nameref = get_attribute_value("ref", attributes);
    let attr_typeref = get_attribute_value("type", attributes);
    let required = if let Some(useage) = get_attribute_value("use", attributes) {
        useage == "required"
    } else {
        false
    };

    let (name, typeref) = if let Some(nameref) = attr_nameref {
        // hard coded special case - this is the only nameref used by attributes in the autosar xsd files
        if nameref == "xml:space" {
            ("xml:space".to_string(), "XML:SPACE".to_string())
        } else {
            return Err(format!(
                "Error: input file used an attribute with an unexpected ref value at {}",
                parser.position()
            ));
        }
    } else if let (Some(name), Some(typeref)) = (attr_name, attr_typeref) {
        (name.to_owned(), typeref.to_owned())
    } else {
        todo!()
    };

    get_element_end_tag(parser, "attribute")?;

    Ok(XsdAttribute {
        name,
        typeref,
        required,
    })
}

fn parse_simple_content(
    parser: &mut EventReader<BufReader<File>>,
    data: &mut Xsd,
) -> Result<XsdSimpleContent, String> {
    let mut extension = None;

    while let Some((local_name, elem_attributes)) = get_next_element(parser, "simpleContent")? {
        match local_name.as_ref() {
            "extension" => {
                extension = Some(parse_extension(parser, data, &elem_attributes)?);
            }
            _ => {
                return Err(format!(
                    "Error: found unexpected start of element tag \"{}\" at {}",
                    local_name,
                    parser.position()
                ));
            }
        }
    }

    if let Some(extension) = extension {
        Ok(XsdSimpleContent { extension })
    } else {
        Err(format!(
            "Error: simpleContent at {} has no extension",
            parser.position()
        ))
    }
}

fn parse_sequence(
    parser: &mut EventReader<BufReader<File>>,
    data: &mut Xsd,
    prev_names: Vec<String>,
) -> Result<XsdSequence, String> {
    let mut items = Vec::new();

    while let Some((local_name, elem_attributes)) = get_next_element(parser, "sequence")? {
        match local_name.as_ref() {
            "annotation" => {
                skip_annotation(parser)?;
            }
            "element" => {
                items.push(XsdModelGroupItem::Element(parse_element(
                    parser,
                    data,
                    &elem_attributes,
                    prev_names.clone(),
                )?));
            }
            "group" => {
                items.push(XsdModelGroupItem::Group(parse_group(
                    parser,
                    data,
                    &elem_attributes,
                    prev_names.clone(),
                )?));
            }
            "choice" => {
                let choice_item = parse_choice(parser, data, &elem_attributes, prev_names.clone())?;
                // transform every choice inside a sequence into a group(choice) instead
                // for this we want a unique-but-stable name
                let pns = prev_names.join("-");
                if let Some(first_inner_name) = choice_item.items.get(0).map(|inner| inner.name()) {
                    // only do anything else if the choice is not empty
                    let choice_name = format!("AR:SEQUENCE-CHOICE--{pns}-{first_inner_name}");
                    // now wrap the choice in a group and add it to the list of groups
                    let newgroup = XsdGroup {
                        item: XsdGroupItem::Choice(choice_item),
                    };
                    if let Some(existing_group) = data.groups.get(&choice_name) {
                        if newgroup != *existing_group {
                            println!("generated synthetic group {choice_name} twice with different content:\n{existing_group:#?}\n{newgroup:#?}");
                        }
                    } else {
                        data.groups.insert(choice_name.clone(), newgroup);
                    }
                    // add a group reference to the current type
                    items.push(XsdModelGroupItem::Group(choice_name));
                }
            }
            _ => {
                return Err(format!(
                    "Error: found unexpected start of element tag \"{}\" at {}",
                    local_name,
                    parser.position()
                ));
            }
        }
    }

    Ok(XsdSequence { items })
}

fn parse_choice(
    parser: &mut EventReader<BufReader<File>>,
    data: &mut Xsd,
    attributes: &Vec<OwnedAttribute>,
    prev_names: Vec<String>,
) -> Result<XsdChoice, String> {
    let mut items = Vec::new();
    let attr_max_occurs = get_attribute_value("maxOccurs", attributes);
    let attr_min_occurs = get_attribute_value("minOccurs", attributes);

    let max_occurs = parse_occurs_attribute(&attr_max_occurs)?;
    let min_occurs = parse_occurs_attribute(&attr_min_occurs)?;

    while let Some((local_name, elem_attributes)) = get_next_element(parser, "choice")? {
        match local_name.as_ref() {
            "element" => {
                items.push(XsdModelGroupItem::Element(parse_element(
                    parser,
                    data,
                    &elem_attributes,
                    prev_names.clone(),
                )?));
            }
            "group" => {
                items.push(XsdModelGroupItem::Group(parse_group(
                    parser,
                    data,
                    &elem_attributes,
                    prev_names.clone(),
                )?));
            }
            "choice" => {
                items.push(XsdModelGroupItem::Choice(Box::new(parse_choice(
                    parser,
                    data,
                    &elem_attributes,
                    prev_names.clone(),
                )?)));
            }
            _ => {
                return Err(format!(
                    "Error: found unexpected start of element tag \"{}\" at {}",
                    local_name,
                    parser.position()
                ));
            }
        }
    }

    Ok(XsdChoice {
        items,
        max_occurs,
        min_occurs,
    })
}

fn parse_restriction(
    parser: &mut EventReader<BufReader<File>>,
    attributes: &Vec<OwnedAttribute>,
) -> Result<XsdRestriction, String> {
    let mut enumvalues: Vec<String> = Vec::new();
    let mut pattern: Option<String> = None;
    let mut max_length: Option<usize> = None;
    let mut literal = false;
    let basetype = get_required_attribute_value("base", attributes, &parser.position())?;

    while let Some((local_name, elem_attributes)) = get_next_element(parser, "restriction")? {
        match local_name.as_ref() {
            "enumeration" => {
                let attrval =
                    get_required_attribute_value("value", &elem_attributes, &parser.position())?;
                enumvalues.push(attrval.to_owned());
            }
            "pattern" => {
                let attrval =
                    get_required_attribute_value("value", &elem_attributes, &parser.position())?;
                pattern = Some(attrval.to_owned());
            }
            "maxLength" => {
                let attrval =
                    get_required_attribute_value("value", &elem_attributes, &parser.position())?;
                if let Ok(val) = attrval.parse() {
                    max_length = Some(val);
                } else {
                    return Err(format!(
                        "Error: failed to parse maxLength value {} at {}",
                        attrval,
                        parser.position()
                    ));
                }
            }
            "whiteSpace" => {
                let attrval =
                    get_required_attribute_value("value", &elem_attributes, &parser.position())?;
                if attrval == "preserve" {
                    literal = true;
                }
            }
            _ => {
                return Err(format!(
                    "Error: found unexpected start of element tag \"{}\" at {}",
                    local_name,
                    parser.position()
                ));
            }
        }
        get_element_end_tag(parser, &local_name)?;
    }

    if (literal && (!enumvalues.is_empty() || pattern.is_some()))
        || (!enumvalues.is_empty() && pattern.is_some())
    {
        return Err(format!(
            "Error: properies for more than one variant found inside <restriction> anding at {}",
            parser.position()
        ));
    }

    if literal {
        Ok(XsdRestriction::Literal)
    } else if let Some(pat) = pattern {
        Ok(XsdRestriction::Pattern {
            pattern: pat,
            maxlength: max_length,
        })
    } else if !enumvalues.is_empty() {
        Ok(XsdRestriction::EnumValues { enumvalues })
    } else {
        Ok(XsdRestriction::Plain {
            basetype: basetype.to_owned(),
        })
    }
}

fn parse_extension(
    parser: &mut EventReader<BufReader<File>>,
    data: &mut Xsd,
    attributes: &Vec<OwnedAttribute>,
) -> Result<XsdExtension, String> {
    let basetype = get_required_attribute_value("base", attributes, &parser.position())?;
    let mut attributes = Vec::new();
    let mut attribute_groups = Vec::new();

    while let Some((local_name, elem_attributes)) = get_next_element(parser, "extension")? {
        match local_name.as_ref() {
            "attribute" => {
                attributes.push(parse_attribute(parser, &elem_attributes)?);
            }
            "attributeGroup" => {
                attribute_groups.push(parse_attribute_group(parser, data, &elem_attributes)?);
            }
            _ => {
                return Err(format!(
                    "Error: found unexpected start of element tag \"{}\" at {}",
                    local_name,
                    parser.position()
                ));
            }
        }
    }

    Ok(XsdExtension {
        basetype: basetype.to_owned(),
        attributes,
        attribute_groups,
    })
}

fn skip_annotation(parser: &mut EventReader<BufReader<File>>) -> Result<(), String> {
    let mut element_stack: Vec<String> = vec!["annotation".to_string()];
    while !element_stack.is_empty() {
        let next_element = get_next_event(parser)?;
        match next_element {
            XmlEvent::StartElement {
                name: OwnedName { local_name, .. },
                ..
            } => {
                element_stack.push(local_name);
            }
            XmlEvent::EndElement {
                name: OwnedName { local_name, .. },
            } => {
                let open_element = element_stack.pop().unwrap();
                if open_element != local_name {
                    return Err(format!(
                        "Error: found unexpected end tag \"{}\" inside <{}> at {}",
                        local_name,
                        open_element,
                        parser.position()
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn get_element_end_tag(parser: &mut EventReader<BufReader<File>>, tag: &str) -> Result<(), String> {
    let event = get_next_event(parser)?;
    if let XmlEvent::EndElement {
        name: OwnedName { local_name, .. },
        ..
    } = &event
    {
        if local_name == tag {
            return Ok(());
        }
    } else if let XmlEvent::StartElement {
        name: OwnedName { local_name, .. },
        ..
    } = &event
    {
        if local_name == "annotation" {
            skip_annotation(parser)?;
            return get_element_end_tag(parser, tag);
        }
    }
    Err(format!(
        "Error: expected end of element {}, but got {:?} instead at position {}",
        tag,
        event,
        parser.position()
    ))
}

fn get_next_event(parser: &mut EventReader<BufReader<File>>) -> Result<XmlEvent, String> {
    let mut next_element = parser.next();

    let mut done = false;
    while !done {
        match next_element {
            Ok(XmlEvent::Whitespace(_))
            | Ok(XmlEvent::Comment(_))
            | Ok(XmlEvent::ProcessingInstruction { .. }) => {
                next_element = parser.next();
            }
            _ => done = true,
        }
    }

    match next_element {
        Ok(elem) => Ok(elem),
        Err(err) => Err(format!("Error: {}", err)),
    }
}

fn get_next_element(
    parser: &mut EventReader<BufReader<File>>,
    parent_element: &str,
) -> Result<Option<(String, Vec<OwnedAttribute>)>, String> {
    loop {
        let cur_event = get_next_event(parser)?;
        match cur_event {
            XmlEvent::StartElement {
                name: OwnedName { local_name, .. },
                attributes: elem_attributes,
                ..
            } => {
                return Ok(Some((local_name, elem_attributes)));
            }
            XmlEvent::EndElement {
                name: OwnedName { local_name, .. },
            } => {
                if local_name == parent_element {
                    return Ok(None);
                } else {
                    return Err(format!(
                        "Error: found unexpected end tag \"{}\" inside <{}> at {}",
                        local_name,
                        parent_element,
                        parser.position()
                    ));
                }
            }
            XmlEvent::StartDocument { .. } | XmlEvent::EndDocument => {
                return Err(format!(
                    "Error: unexpected {:?} at {}",
                    cur_event,
                    parser.position()
                ));
            }
            _ => {}
        }
    }
}

fn get_attribute_value<'a>(key: &str, attributes: &'a Vec<OwnedAttribute>) -> Option<&'a str> {
    for OwnedAttribute {
        name: OwnedName { local_name, .. },
        value,
    } in attributes
    {
        if key == local_name {
            return Some(value);
        }
    }
    None
}

fn get_required_attribute_value<'a>(
    key: &str,
    attributes: &'a Vec<OwnedAttribute>,
    position: &TextPosition,
) -> Result<&'a str, String> {
    if let Some(name) = get_attribute_value(key, attributes) {
        Ok(name)
    } else {
        Err(format!(
            "Error: mandatory attribute \"{}\" is missing at {}",
            key, position
        ))
    }
}

fn parse_occurs_attribute(attr_occurs: &Option<&str>) -> Result<usize, String> {
    if let Some(occurs_str) = attr_occurs {
        if *occurs_str == "unbounded" {
            Ok(std::usize::MAX)
        } else {
            match occurs_str.parse() {
                Ok(val) => Ok(val),
                Err(err) => Err(format!("Error: parsing {} - {}", occurs_str, err)),
            }
        }
    } else {
        Ok(1)
    }
}

fn extend_prev_names(prev_names: &mut Vec<String>, attr_name: &Option<&str>) {
    if let Some(name) = attr_name {
        let len = prev_names.len();
        if len > 1 {
            if prev_names[len - 1].contains(*name) || name.contains(&prev_names[len - 1]) {
                prev_names.pop();
                prev_names.push((*name).to_owned());
            } else {
                prev_names.push((*name).to_owned());
            }
        } else {
            prev_names.push((*name).to_owned());
        }
    }
}

impl XsdModelGroupItem {
    fn name(&self) -> String {
        match self {
            XsdModelGroupItem::Group(groupref) => groupref.clone(),
            XsdModelGroupItem::Choice(_) => "CHOICE".to_string(),
            XsdModelGroupItem::Element(_) => "ELEMENT".to_string(),
        }
    }
}
