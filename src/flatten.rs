use std::collections::HashMap;

use super::*;
use super::xsd::*;

pub(crate) fn flatten_schema(data: &Xsd) -> Result<HashMap<String, ElementContent>, String> {

    let mut work_queue = Vec::new();
    let mut complete_types = HashMap::<String, ElementContent>::new();

    if data.root_elements.len() != 1 {
        return Err(format!("Error: There should only be one root element, <AUTOSAR>, but instead there are these: {:#?}", data.root_elements));
    }

    for element in &data.root_elements {
        work_queue.push((element.name.clone(), element.typeref.clone()));
    }

    while !work_queue.is_empty() {
        let (cur_element_name, cur_element_typeref) = work_queue.pop().unwrap();

        if complete_types.get(&cur_element_typeref).is_none() {
            let element_content = if let Some(element_type) = data.types.get(&cur_element_typeref) {
                flatten_type(data, element_type, &cur_element_name)?
            } else {
                return Err(format!("Error: unresolvable type {}", cur_element_typeref))
            };

            match &element_content {
                ElementContent::Elements { element_collection, .. } |
                ElementContent::Mixed { element_collection, .. } => {
                    for sub_elem in element_collection {
                        work_queue.push((sub_elem.name.clone(), sub_elem.typeref.clone()));
                    }
                }
                _ => {}
            }
            complete_types.insert(cur_element_typeref.clone(), element_content); 
        }
    }

    Ok(complete_types)
}


fn flatten_type<'a>(data: &'a Xsd, element_type: &'a XsdType, name: &str) -> Result<ElementContent, String> {
    match element_type {
        XsdType::Base(typename) => {
            Ok(ElementContent::Characters {
                basetype: typename.clone(),
                restriction_pattern: None,
                max_length: None,
                attributes: Vec::new()
            })
        }
        XsdType::Simple(simple_type) => {
            flatten_simple_type(data, simple_type, name)
        }
        XsdType::Complex(complex_type) => {
            flatten_complex_type(data, complex_type, name)
        }
    }
}


fn flatten_complex_type<'a>(data: &'a Xsd, complex_type: &'a XsdComplexType, _name: &str) -> Result<ElementContent, String> {
    let attributes = build_attribute_list(data, &Vec::new(), &complex_type.attribute_groups)?;
    match &complex_type.item {
        XsdComplexTypeItem::SimpleContent(simple_content) => {
            flatten_simple_content(data, simple_content)
        }
        XsdComplexTypeItem::Group(group_ref) => {
            if let Some(group) = data.groups.get(group_ref) {
                let elements = flatten_group(data, group)?;
                Ok(ElementContent::Elements {
                    element_collection: elements,
                    attributes
                })
            } else {
                return Err(format!("Error: unknown group ref {} found in complexType {}", group_ref, complex_type.name));
            }
        }
        XsdComplexTypeItem::Choice(choice) => {
            let elements = flatten_choice(data, choice)?;
            if complex_type.mixed_content {
                if elements.into_iter().count() == 0 {
                    Ok(ElementContent::Characters {
                        basetype: "xsd:string".to_string(),
                        restriction_pattern: None,
                        max_length: None,
                        attributes
                    })
                } else {
                    Ok(ElementContent::Mixed {
                        element_collection: elements,
                        attributes,
                        basetype: "xsd:string".to_string()
                    })
                }
            } else {
                Ok(ElementContent::Elements {
                    element_collection: elements,
                    attributes
                })
            }
        }
        XsdComplexTypeItem::Sequence(sequence) => {
            let elements = flatten_sequence(data, sequence)?;
            Ok(ElementContent::Elements {
                element_collection: elements,
                attributes
            })
        }
        XsdComplexTypeItem::None => {
            Err(format!("Error: empty complexType"))
        }
    }
}


fn flatten_simple_content(data: &Xsd, simple_content: &XsdSimpleContent) -> Result<ElementContent, String> {
    if let Some(basetype) = data.types.get(&simple_content.extension.basetype) {
        let mut attributes = build_attribute_list(data, &simple_content.extension.attributes, &simple_content.extension.attribute_groups)?;
        match basetype {
            XsdType::Base(typename) => {
                Ok(ElementContent::Characters {
                    basetype: typename.clone(),
                    restriction_pattern: None,
                    max_length: None,
                    attributes
                })
            }
            XsdType::Simple(simple_type) => {
                let mut simple_type = flatten_simple_type(data, simple_type, &simple_content.extension.basetype)?;
                // append the attributes attached to the <extension> to the attributes gathered inside the <simpleType>
                match &mut simple_type {
                    ElementContent::Elements { attributes: inner_attributes, .. } |
                    ElementContent::Characters { attributes: inner_attributes, .. } => {
                        inner_attributes.append(&mut attributes);
                    }
                    _ => {}
                };
                Ok(simple_type)
            }
            XsdType::Complex(complex_type) => {
                let mut complex_type = flatten_complex_type(data, complex_type, &simple_content.extension.basetype)?;
                // append the attributes attached to the <extension> to the attributes gathered inside the <complexType>
                match &mut complex_type {
                    ElementContent::Elements { attributes: inner_attributes, .. } |
                    ElementContent::Characters { attributes: inner_attributes, .. } => {
                        inner_attributes.append(&mut attributes);
                    }
                    _ => {}
                };
                Ok(complex_type)
            }
        }
    } else {
        Err(format!("failed to find type {}", simple_content.extension.basetype))
    }
}


fn flatten_group(data: &Xsd, group: &XsdGroup) -> Result<ElementCollection, String> {
    match &group.item {
        XsdGroupItem::Sequence(sequence) => {
            flatten_sequence(data, sequence)
        }
        XsdGroupItem::Choice(choice) => {
            flatten_choice(data, choice)
        }
        XsdGroupItem::None => Err(format!("Error: empty group")),
    }
}


fn flatten_choice<'a>(data: &'a Xsd, choice: &'a XsdChoice) -> Result<ElementCollection, String> {
    let mut elements: Vec<ElementCollection> = Vec::new();
    let mut outer_amount = occurs_to_amount(choice.min_occurs, choice.max_occurs);
    let mut name = "".to_string();

    for item in &choice.items {
        match item {
            XsdModelGroupItem::Group(group_ref) => {
                if let Some(group) = data.groups.get(group_ref) {
                    match flatten_group(data, group)? {
                        ElementCollection::Choice {mut sub_elements, amount: inner_choice_amount, name: mut inner_name} => {
                            if inner_name == "" {
                                inner_name = group_ref.split_at(3).1.to_owned();
                            }
                            flatten_choice_choice(choice, &mut elements, &mut sub_elements, &mut outer_amount, inner_choice_amount, &mut name, inner_name);
                        }
                        ElementCollection::Sequence { sub_elements, name: mut inner_name } => {
                            if inner_name == "" {
                                inner_name = group_ref.split_at(3).1.to_owned();
                            }
                            if sub_elements.len() > 0 {
                                elements.push(ElementCollection::Sequence {
                                    name: inner_name,
                                    sub_elements
                                });
                            }
                        }
                        ElementCollection::Element(_) => todo!(),
                    }
                } else {
                    return Err(format!("Error: unknown group ref {} found in sequence", group_ref));
                }
            }
            XsdModelGroupItem::Choice(choice_inner) => {
                match flatten_choice(data, choice_inner)? {
                    ElementCollection::Choice {mut sub_elements, amount: inner_choice_amount, name: inner_name} => {
                        flatten_choice_choice(choice, &mut elements, &mut sub_elements, &mut outer_amount, inner_choice_amount, &mut name, inner_name);
                    }
                    ElementCollection::Sequence {..} => {
                        todo!();
                    }
                    ElementCollection::Element(_) => todo!(),
                }
            }
            XsdModelGroupItem::Element(xsd_element) => {
                elements.push(ElementCollection::Element(
                    Element::new(xsd_element, data.version_info)
                ));
            }
        }
    }
    
    Ok(ElementCollection::Choice {
        sub_elements: elements,
        amount: outer_amount,
        name
    })
}

fn flatten_choice_choice(
    outer_choice: &XsdChoice,
    elements: &mut Vec<ElementCollection>,
    sub_elements: &mut Vec<ElementCollection>,
    outer_amount: &mut ElementAmount,
    inner_amount: ElementAmount,
    outer_name: &mut String,
    inner_name: String
) {
    if outer_choice.items.len() == 1 {
        // adjust the amount of the outer choice
        *outer_amount = combine_amounts(*outer_amount, inner_amount);
        elements.append(sub_elements);
        if *outer_name == "" && inner_name != "" {
            *outer_name = inner_name;
        }
    } else {
        if *outer_amount == inner_amount {
            elements.append(sub_elements);
        } else {
            // adjust the amount of each individual element
            //     for elem in &mut choice_elements {
            //         elem.amount = combine_amounts(elem.amount, inner_choice_amount);
            //     }
            todo!()
        }
    }
}


fn flatten_sequence<'a>(data: &'a Xsd, sequence: &'a XsdSequence) -> Result<ElementCollection, String> {
    let mut elements: Vec<ElementCollection> = Vec::new();

    for item in &sequence.items {
        match item {
            XsdModelGroupItem::Group(group_ref) => {
                if let Some(group) = data.groups.get(group_ref) {
                    match flatten_group(data, group)? {
                        ElementCollection::Choice {..} => {
                            todo!();
                        }
                        ElementCollection::Sequence { mut sub_elements, ..} => {
                            elements.append(&mut sub_elements);
                        }
                        ElementCollection::Element(_) => todo!(),
                    }
                } else {
                    return Err(format!("Error: unknown group ref {} found in sequence", group_ref));
                }
            }
            XsdModelGroupItem::Choice(choice) => {
                match flatten_choice(data, choice)? {
                    ElementCollection::Choice {mut sub_elements, amount: inner_choice_amount, name: inner_name} => {
                        if sequence.items.len() == 1 {
                            todo!()
                        } else if sub_elements.len() == 1 {
                            match &mut sub_elements[0] {
                                ElementCollection::Choice { amount, .. } |
                                ElementCollection::Element(Element { amount, .. }) => {
                                    *amount = combine_amounts(*amount, inner_choice_amount);
                                }
                                _ => {}
                            }
                            elements.append(&mut sub_elements);
                        } else {
                            elements.push(
                                ElementCollection::Choice { name: inner_name, sub_elements, amount: inner_choice_amount }
                            );
                        }
                    }
                    ElementCollection::Sequence {..} => {
                        // elements.append(&mut sequence_elements);
                        todo!()
                    }
                    ElementCollection::Element(_) => todo!(),
                }
            }
            XsdModelGroupItem::Element(xsd_element) => {
                elements.push(ElementCollection::Element(
                    Element::new(xsd_element, data.version_info)
                ));
            }
        }
    }

    Ok(ElementCollection::Sequence {
        sub_elements: elements,
        name: "".to_string()
    })
}


fn flatten_simple_type(data: &Xsd, simple_type: &XsdSimpleType, name: &str) -> Result<ElementContent, String> {
    match simple_type {
        XsdSimpleType::Restriction(XsdRestriction::Pattern { pattern, maxlength }) => {
            Ok(ElementContent::Characters {
                basetype: "xsd:string".to_string(),
                restriction_pattern: Some(pattern.clone()),
                max_length: maxlength.clone(),
                attributes: Vec::new()
            })
        }
        XsdSimpleType::Restriction(XsdRestriction::Plain { basetype }) => {
            Ok(ElementContent::Characters {
                basetype: basetype.to_owned(),
                restriction_pattern: None,
                max_length: None,
                attributes: Vec::new()
            })
        }
        XsdSimpleType::Restriction(XsdRestriction::Literal) => {
            Ok(ElementContent::Characters {
                basetype: "xsd:string".to_string(),
                restriction_pattern: None,
                max_length: None,
                attributes: Vec::new()
            })
        }
        XsdSimpleType::Restriction(XsdRestriction::EnumValues { enumvalues }) => {
            let enumitems = enumvalues.iter().map(|e| (e.clone(), data.version_info)).collect();
            Ok(ElementContent::Enum(EnumDefinition {
                name: name.to_string(),
                enumitems
            }))
        }
    }
}


fn build_attribute_list(data: &Xsd, xsd_attributes: &Vec<XsdAttribute>, xsd_attribute_groups: &Vec<String>) -> Result<Vec<Attribute>, String> {
    let mut attributes = Vec::new();

    for attr in xsd_attributes {
        attributes.push(
            build_attribute(data, attr)?
        );
    
    }

    for attr_group_name in xsd_attribute_groups {
        if attr_group_name == "AR:WHITESPACE-CONTROLLED" {
            
        }
        if let Some(attr_group) = data.attribute_groups.get(attr_group_name) {
            for attr in &attr_group.attributes {
                attributes.push(
                    build_attribute(data, attr)?
                );
            }
        } else {
            return Err(format!("Error: attribute group {} is referenced but not found", attr_group_name));
        }
    }

    Ok(attributes)
}


fn build_attribute(data: &Xsd, attr: &XsdAttribute) -> Result<Attribute, String> {
    let attribute_type = if let Some(attr_type) = data.types.get(&attr.typeref) {
        match attr_type {
            XsdType::Base(basetype) => AttributeType::Basic(basetype.clone()),
            XsdType::Simple(simple_type) => {
                match simple_type {
                    XsdSimpleType::Restriction(XsdRestriction::Pattern { pattern, maxlength }) => {
                        AttributeType::Pattern {
                            pattern: pattern.to_owned(),
                            maxlength: maxlength.clone()
                        }
                    }
                    XsdSimpleType::Restriction(XsdRestriction::Plain { basetype }) => {
                        AttributeType::Basic(basetype.to_owned())
                    }
                    XsdSimpleType::Restriction(XsdRestriction::Literal) => {
                        return Err("WTF: preserve whitespace in an attribute ?!?!?!".to_string());
                    }
                    XsdSimpleType::Restriction(XsdRestriction::EnumValues { enumvalues }) => {
                        let enumitems = enumvalues.iter().map(|e| (e.clone(), data.version_info)).collect();
                        AttributeType::Enum(EnumDefinition {
                            name: attr.typeref.to_owned(),
                            enumitems
                        })
                    }                       
                }
            }
            XsdType::Complex(_) => {
                return Err("Error: Complex type for attribute ?!?!".to_string());
            }
        }
    } else {
        return Err(format!("Error: attribute references type {}, but the type was not found", attr.typeref));
    };

    Ok(Attribute {
        name: attr.name.clone(),
        attribute_type,
        required: attr.required,
        version_info: data.version_info
    })
}


fn combine_amounts(amount_1: ElementAmount, amount_2: ElementAmount) -> ElementAmount {
    match (amount_1, amount_2) {
        (ElementAmount::ZeroOrOne, ElementAmount::ZeroOrOne) |        
        (ElementAmount::ZeroOrOne, ElementAmount::One) |
        (ElementAmount::One, ElementAmount::ZeroOrOne) => {
            ElementAmount::ZeroOrOne
        }

        (ElementAmount::One, ElementAmount::One) => {
            ElementAmount::One
        },

        (ElementAmount::ZeroOrOne, ElementAmount::Any) |
        (ElementAmount::One, ElementAmount::Any) |
        (ElementAmount::Any, ElementAmount::ZeroOrOne) |
        (ElementAmount::Any, ElementAmount::One) |
        (ElementAmount::Any, ElementAmount::Any) => {
            ElementAmount::Any
        }
    }
}


fn occurs_to_amount(min_occurs: usize, max_occurs: usize) -> ElementAmount {
    if min_occurs == 1 && max_occurs == 1 {
        ElementAmount::One
    } else if min_occurs == 0 && max_occurs == 1 {
        ElementAmount::ZeroOrOne
    } else {
        ElementAmount::Any
    }
}


impl Element {
    fn new(xsd_element: &XsdElement, version_info: usize) -> Self {
        Self {
            name: xsd_element.name.to_owned(),
            typeref: xsd_element.typeref.to_owned(),
            amount: occurs_to_amount(xsd_element.min_occurs, xsd_element.max_occurs),
            version_info
        }
    }
}
