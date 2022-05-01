use std::collections::HashMap;

use super::*;
use super::xsd::*;

pub(crate) fn flatten_schema(data: &Xsd) -> Result<HashMap<String, DataType>, String> {

    let mut work_queue = Vec::new();
    let mut complete_types = HashMap::<String, DataType>::new();

    if data.root_elements.len() != 1 {
        return Err(format!("Error: There should only be one root element, <AUTOSAR>, but instead there are these: {:#?}", data.root_elements));
    }

    for element in &data.root_elements {
        work_queue.push(element.typeref.clone());
    }

    while !work_queue.is_empty() {
        let cur_element_typeref = work_queue.pop().unwrap();

        if complete_types.get(&cur_element_typeref).is_none() {
            let element_content = flatten_any(&cur_element_typeref, data)?;

            if let Some(element_collection) = element_content.collection() {
                for item in element_collection.items() {
                    match item {
                        ElementCollectionItem::Element(Element { typeref, .. }) |
                        ElementCollectionItem::GroupRef(typeref) => {
                            work_queue.push(typeref.clone());
                        }
                    }
                }
            }

            if let Some(attributes) = element_content.attributes() {
                for attr in attributes {
                    match &attr.attribute_type {
                        AttributeType::Basic(_) => {},
                        AttributeType::Pattern { typename, maxlength, pattern } => {
                            if complete_types.get(typename).is_none() {
                                complete_types.insert(typename.clone(), DataType::Characters {
                                    basetype: "xsd:string".to_owned(),
                                    attributes: vec![],
                                    max_length: maxlength.to_owned(),
                                    restriction_pattern: Some(pattern.to_owned())
                                });
                            }
                        },
                        AttributeType::Enum(enumref) => {
                            work_queue.push(enumref.clone());
                        }
                    }
                }
            }

            complete_types.insert(cur_element_typeref.clone(), element_content); 
        }
    }

    Ok(complete_types)
}


fn flatten_any(cur_element_typeref: &String, data: &Xsd) -> Result<DataType, String> {
    let trlen = cur_element_typeref.len();
    if cur_element_typeref.ends_with("-ELEMENTGROUP") && data.groups.get(&cur_element_typeref[0..trlen-13]).is_some() {
        let xsd_group = data.groups.get(&cur_element_typeref[0..trlen-13]).unwrap();
        let group_elements = flatten_group(data, xsd_group)?;
        Ok(DataType::ElementsGroup {
            element_collection: group_elements
        })
    } else {
        if let Some(element_type) = data.types.get(cur_element_typeref) {
            flatten_type(data, element_type, cur_element_typeref)
        } else {
            Err(format!("Error: unresolvable type {}", cur_element_typeref))
        }
    }
}


fn flatten_type<'a>(data: &'a Xsd, element_type: &'a XsdType, typename: &str) -> Result<DataType, String> {
    match element_type {
        XsdType::Base(typename) => {
            Ok(DataType::Characters {
                basetype: typename.clone(),
                restriction_pattern: None,
                max_length: None,
                attributes: Vec::new()
            })
        }
        XsdType::Simple(simple_type) => {
            flatten_simple_type(data, simple_type, typename)
        }
        XsdType::Complex(complex_type) => {
            flatten_complex_type(data, complex_type)
        }
    }
}


fn flatten_complex_type<'a>(data: &'a Xsd, complex_type: &'a XsdComplexType) -> Result<DataType, String> {
    let attributes = build_attribute_list(data, &Vec::new(), &complex_type.attribute_groups)?;
    match &complex_type.item {
        XsdComplexTypeItem::SimpleContent(simple_content) => {
            flatten_simple_content(data, simple_content)
        }
        XsdComplexTypeItem::Group(group_ref) => {
            if let Some(group) = data.groups.get(group_ref) {
                let elements = flatten_group(data, group)?;
                Ok(DataType::Elements {
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
                if elements.items().len() == 0 {
                    Ok(DataType::Characters {
                        basetype: "xsd:string".to_string(),
                        restriction_pattern: None,
                        max_length: None,
                        attributes
                    })
                } else {
                    Ok(DataType::Mixed {
                        element_collection: elements,
                        attributes,
                        basetype: "xsd:string".to_string()
                    })
                }
            } else {
                Ok(DataType::Elements {
                    element_collection: elements,
                    attributes
                })
            }
        }
        XsdComplexTypeItem::Sequence(sequence) => {
            let elements = flatten_sequence(data, sequence)?;
            Ok(DataType::Elements {
                element_collection: elements,
                attributes
            })
        }
        XsdComplexTypeItem::None => {
            Err(format!("Error: empty complexType"))
        }
    }
}


fn flatten_simple_content(data: &Xsd, simple_content: &XsdSimpleContent) -> Result<DataType, String> {
    if let Some(basetype) = data.types.get(&simple_content.extension.basetype) {
        let mut attributes = build_attribute_list(data, &simple_content.extension.attributes, &simple_content.extension.attribute_groups)?;
        match basetype {
            XsdType::Base(typename) => {
                Ok(DataType::Characters {
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
                    DataType::Elements { attributes: inner_attributes, .. } |
                    DataType::Characters { attributes: inner_attributes, .. } => {
                        inner_attributes.append(&mut attributes);
                    }
                    _ => {}
                };
                Ok(simple_type)
            }
            XsdType::Complex(complex_type) => {
                let mut complex_type = flatten_complex_type(data, complex_type)?;
                // append the attributes attached to the <extension> to the attributes gathered inside the <complexType>
                match &mut complex_type {
                    DataType::Elements { attributes: inner_attributes, .. } |
                    DataType::Characters { attributes: inner_attributes, .. } => {
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
    let mut elements: Vec<ElementCollectionItem> = Vec::new();
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
                            if sub_elements.len() == 1 {
                                elements.push(sub_elements[0].clone());
                            } else if sub_elements.len() > 0 {
                                elements.push(ElementCollectionItem::GroupRef(format!("AR:{inner_name}-ELEMENTGROUP")));
                            }
                        }
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
                }
            }
            XsdModelGroupItem::Element(xsd_element) => {
                elements.push(ElementCollectionItem::Element(
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
    elements: &mut Vec<ElementCollectionItem>,
    sub_elements: &mut Vec<ElementCollectionItem>,
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
            todo!()
        }
    }
}


fn flatten_sequence<'a>(data: &'a Xsd, sequence: &'a XsdSequence) -> Result<ElementCollection, String> {
    let mut flat_items = Vec::new();
    for item in &sequence.items {
        match item {
            XsdModelGroupItem::Group(group_ref) => {
                if let Some(group) = data.groups.get(group_ref) {
                    flat_items.push(flatten_group(data, group)?);
                } else {
                    return Err(format!("Error: unknown group ref {} found in sequence", group_ref));
                }
            }
            XsdModelGroupItem::Choice(choice) => {
                flat_items.push(flatten_choice(data, choice)?);
            }
            XsdModelGroupItem::Element(xsd_element) => {
                flat_items.push(ElementCollection::Sequence {
                    name: "".to_string(),
                    sub_elements: vec![
                        ElementCollectionItem::Element(
                            Element::new(xsd_element, data.version_info)
                        )
                    ]
                });
            }
        }
    }

    let nonempty_inputs = flat_items.iter().filter(|item| item.items().len() > 0).count();
    let mut elements: Vec<ElementCollectionItem> = Vec::new();
    let mut replacement = None;

    for (idx, item) in flat_items.iter_mut().enumerate() {
        match item {
            ElementCollection::Choice { name, sub_elements, amount } => {
                if sub_elements.len() == 1 {
                    // choice of only one element is actually no choice at all. The element can be added to the containing sequence
                    // combine the amount of the choice structure and the amount of the single contained element
                    match &mut sub_elements[0] {
                        ElementCollectionItem::Element(Element { amount: element_amount, .. }) => {
                            *element_amount = combine_amounts(*amount, *element_amount);
                        }
                        _ => {}
                    }
                    elements.append(sub_elements);
                } else if sub_elements.len() > 0 {
                    // only do anything with this Choice item if it actually contains any elements
                    if nonempty_inputs == 1 {
                        // this Choice item is the only item in the sequence that contains any elements, so the sequence can be turned into a choice
                        replacement = Some(ElementCollection::Choice {
                            sub_elements: sub_elements.clone(),
                            amount: amount.clone(),
                            name: name.clone()
                        });
                    } else if let XsdModelGroupItem::Group(group_ref) = &sequence.items[idx] {
                        // the choice came from a group, we'll only keep a reference to that group here
                        elements.push(
                            ElementCollectionItem::GroupRef(format!("{group_ref}-ELEMENTGROUP"))
                        );
                    } else if data.groups.get(&format!("AR:{name}")).is_some() {
                        // the choice came from a group, we'll only keep a reference to that group here
                        elements.push(
                            ElementCollectionItem::GroupRef(format!("AR:{name}-ELEMENTGROUP"))
                        );
                    } else {
                        // println!("FALLBACK: weakening of sequence(choice [{name}], ...) to sequence\n{sequence:#?}");
                        for sub_elem in sub_elements.iter_mut() {
                            match sub_elem {
                                ElementCollectionItem::Element(Element { amount: element_amount, .. }) => {
                                    *element_amount = combine_amounts(*amount, *element_amount);
                                }
                                _ => {}
                            }
                        }
                        elements.append(sub_elements);
                    }
                }
            }
            ElementCollection::Sequence { sub_elements, .. } => {
                elements.append(sub_elements);
            }
        }
    }

    if let Some(repl) = replacement {
        Ok(repl)
    } else {
        Ok(ElementCollection::Sequence {
            sub_elements: elements,
            name: "".to_string()
        })
    }
}


fn flatten_simple_type(data: &Xsd, simple_type: &XsdSimpleType, typename: &str) -> Result<DataType, String> {
    match simple_type {
        XsdSimpleType::Restriction(XsdRestriction::Pattern { pattern, maxlength }) => {
            Ok(DataType::Characters {
                basetype: "xsd:string".to_string(),
                restriction_pattern: Some(pattern.clone()),
                max_length: maxlength.clone(),
                attributes: Vec::new()
            })
        }
        XsdSimpleType::Restriction(XsdRestriction::Plain { basetype }) => {
            Ok(DataType::Characters {
                basetype: basetype.to_owned(),
                restriction_pattern: None,
                max_length: None,
                attributes: Vec::new()
            })
        }
        XsdSimpleType::Restriction(XsdRestriction::Literal) => {
            Ok(DataType::Characters {
                basetype: "xsd:string".to_string(),
                restriction_pattern: None,
                max_length: None,
                attributes: Vec::new()
            })
        }
        XsdSimpleType::Restriction(XsdRestriction::EnumValues { enumvalues }) => {
            let enumitems = enumvalues.iter().map(|e| (e.clone(), data.version_info)).collect();
            Ok(DataType::Enum(EnumDefinition {
                name: typename.to_string(),
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
                            typename: attr.typeref.clone(),
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
                    XsdSimpleType::Restriction(XsdRestriction::EnumValues { .. }) => {
                        // let enumitems = enumvalues.iter().map(|e| (e.clone(), data.version_info)).collect();
                        AttributeType::Enum(attr.typeref.to_owned())
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


pub(crate) fn combine_amounts(amount_1: ElementAmount, amount_2: ElementAmount) -> ElementAmount {
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
