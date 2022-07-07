use super::xsd::*;
use super::*;

enum WorkQueueItem {
    ElementType(String),
    CharacterType(String),
    Group(String),
}

pub(crate) fn flatten_schema(data: &Xsd) -> Result<AutosarDataTypes, String> {
    let mut autosar_schema = AutosarDataTypes::new();
    let mut work_queue = Vec::new();

    if data.root_elements.len() != 1 {
        return Err(format!(
            "Error: There should only be one root element, <AUTOSAR>, but instead there are these: {:#?}",
            data.root_elements
        ));
    }

    for element in &data.root_elements {
        work_queue.push(WorkQueueItem::ElementType(element.typeref.clone()));
    }

    while !work_queue.is_empty() {
        match work_queue.pop().unwrap() {
            WorkQueueItem::ElementType(cur_element_typeref) => {
                if autosar_schema
                    .element_types
                    .get(&cur_element_typeref)
                    .is_none()
                {
                    if let Some(XsdType::Complex(complex_type)) =
                        data.types.get(&cur_element_typeref)
                    {
                        let elemtype = flatten_complex_type(data, complex_type)?;

                        enqueue_dependencies(&mut work_queue, &elemtype);
                        autosar_schema
                            .element_types
                            .insert(cur_element_typeref, elemtype);
                    } else {
                        autosar_schema.element_types.insert(
                            cur_element_typeref.clone(),
                            ElementDataType::Characters {
                                attributes: Vec::new(),
                                basetype: cur_element_typeref.clone(),
                            },
                        );
                        work_queue.push(WorkQueueItem::CharacterType(cur_element_typeref.clone()));
                    }
                }
            }
            WorkQueueItem::CharacterType(cur_char_typeref) => {
                if autosar_schema
                    .character_types
                    .get(&cur_char_typeref)
                    .is_none()
                {
                    if let Some(XsdType::Simple(simple_type)) = data.types.get(&cur_char_typeref) {
                        let chartype = flatten_simple_type(data, simple_type, &cur_char_typeref)?;
                        autosar_schema
                            .character_types
                            .insert(cur_char_typeref, chartype);
                    } else {
                        return Err(format!("Error: unresolvable type {}", cur_char_typeref));
                    }
                }
            }
            WorkQueueItem::Group(cur_group_typeref) => {
                if autosar_schema
                    .element_types
                    .get(&cur_group_typeref)
                    .is_none()
                {
                    let trlen = cur_group_typeref.len();
                    if cur_group_typeref.ends_with("-ELEMENTGROUP")
                        && data.groups.get(&cur_group_typeref[0..trlen - 13]).is_some()
                    {
                        let xsd_group = data.groups.get(&cur_group_typeref[0..trlen - 13]).unwrap();
                        let group_elements = ElementDataType::ElementsGroup {
                            element_collection: flatten_group(data, xsd_group)?,
                        };

                        enqueue_dependencies(&mut work_queue, &group_elements);
                        autosar_schema
                            .element_types
                            .insert(cur_group_typeref, group_elements);
                    }
                }
            }
        }
    }

    // these attributes of the root data type, AR:AUTOSAR, are not defined in the xsd files
    if let Some(ElementDataType::Elements { attributes, .. }) =
        autosar_schema.element_types.get_mut("AR:AUTOSAR")
    {
        attributes.push(Attribute {
            name: "xmlns".to_string(),
            attribute_type: "xsd:string".to_string(),
            required: true,
            version_info: data.version_info,
        });
        attributes.push(Attribute {
            name: "xmlns:xsi".to_string(),
            attribute_type: "xsd:string".to_string(),
            required: true,
            version_info: data.version_info,
        });
        attributes.push(Attribute {
            name: "xsi:schemaLocation".to_string(),
            attribute_type: "xsd:string".to_string(),
            required: true,
            version_info: data.version_info,
        });
    }

    Ok(autosar_schema)
}

fn enqueue_dependencies(work_queue: &mut Vec<WorkQueueItem>, elemtype: &ElementDataType) {
    if let Some(element_collection) = elemtype.collection() {
        for item in element_collection.items() {
            match item {
                ElementCollectionItem::Element(Element { typeref, .. }) => {
                    work_queue.push(WorkQueueItem::ElementType(typeref.clone()));
                }
                ElementCollectionItem::GroupRef(typeref) => {
                    work_queue.push(WorkQueueItem::Group(typeref.clone()));
                }
            }
        }
    }
    if let Some(attributes) = elemtype.attributes() {
        for attr in attributes {
            work_queue.push(WorkQueueItem::CharacterType(attr.attribute_type.clone()));
        }
    }
    if let Some(basetype) = elemtype.basetype() {
        work_queue.push(WorkQueueItem::CharacterType(basetype.to_string()));
    }
}

fn flatten_complex_type<'a>(
    data: &'a Xsd,
    complex_type: &'a XsdComplexType,
) -> Result<ElementDataType, String> {
    let attributes = build_attribute_list(data, &Vec::new(), &complex_type.attribute_groups)?;
    match &complex_type.item {
        XsdComplexTypeItem::SimpleContent(simple_content) => {
            flatten_simple_content(data, simple_content)
        }
        XsdComplexTypeItem::Group(group_ref) => {
            if let Some(group) = data.groups.get(group_ref) {
                let elements = flatten_group(data, group)?;
                Ok(ElementDataType::Elements {
                    element_collection: elements,
                    attributes,
                })
            } else {
                Err(format!(
                    "Error: unknown group ref {} found in complexType {}",
                    group_ref, complex_type.name
                ))
            }
        }
        XsdComplexTypeItem::Choice(choice) => {
            let elements = flatten_choice(data, choice)?;
            if complex_type.mixed_content {
                if elements.items().is_empty() {
                    Ok(ElementDataType::Characters {
                        basetype: "xsd:string".to_string(),
                        attributes,
                    })
                } else {
                    Ok(ElementDataType::Mixed {
                        element_collection: elements,
                        attributes,
                        basetype: "xsd:string".to_string(),
                    })
                }
            } else {
                Ok(ElementDataType::Elements {
                    element_collection: elements,
                    attributes,
                })
            }
        }
        XsdComplexTypeItem::Sequence(sequence) => {
            let elements = flatten_sequence(data, sequence)?;
            Ok(ElementDataType::Elements {
                element_collection: elements,
                attributes,
            })
        }
        XsdComplexTypeItem::None => Err("Error: empty complexType".to_string()),
    }
}

fn flatten_simple_content(
    data: &Xsd,
    simple_content: &XsdSimpleContent,
) -> Result<ElementDataType, String> {
    if let Some(basetype) = data.types.get(&simple_content.extension.basetype) {
        let mut attributes = build_attribute_list(
            data,
            &simple_content.extension.attributes,
            &simple_content.extension.attribute_groups,
        )?;
        match basetype {
            XsdType::Base(_) => Ok(ElementDataType::Characters {
                attributes,
                basetype: simple_content.extension.basetype.clone(),
            }),
            XsdType::Simple(_) => Ok(ElementDataType::Characters {
                attributes,
                basetype: simple_content.extension.basetype.clone(),
            }),
            XsdType::Complex(complex_type) => {
                let mut complex_type = flatten_complex_type(data, complex_type)?;
                // append the attributes attached to the <extension> to the attributes gathered inside the <complexType>
                match &mut complex_type {
                    ElementDataType::Elements {
                        attributes: inner_attributes,
                        ..
                    }
                    | ElementDataType::Characters {
                        attributes: inner_attributes,
                        ..
                    } => {
                        inner_attributes.append(&mut attributes);
                    }
                    _ => {}
                };
                Ok(complex_type)
            }
        }
    } else {
        Err(format!(
            "failed to find type {}",
            simple_content.extension.basetype
        ))
    }
}

fn flatten_group(data: &Xsd, group: &XsdGroup) -> Result<ElementCollection, String> {
    match &group.item {
        XsdGroupItem::Sequence(sequence) => flatten_sequence(data, sequence),
        XsdGroupItem::Choice(choice) => flatten_choice(data, choice),
        XsdGroupItem::None => Err("Error: empty group".to_string()),
    }
}

fn flatten_choice<'a>(data: &'a Xsd, choice: &'a XsdChoice) -> Result<ElementCollection, String> {
    let mut elements: Vec<ElementCollectionItem> = Vec::new();
    let mut outer_amount = occurs_to_amount(choice.min_occurs, choice.max_occurs);
    let mut name = "".to_string();
    let mut replacement = None;

    for item in &choice.items {
        match item {
            XsdModelGroupItem::Group(group_ref) => {
                if let Some(group) = data.groups.get(group_ref) {
                    match flatten_group(data, group)? {
                        ElementCollection::Choice {
                            mut sub_elements,
                            amount: inner_choice_amount,
                            name: mut inner_name,
                        } => {
                            if inner_name.is_empty() {
                                // split off the prefix "AR:" from the group name and only use the remainder
                                inner_name = group_ref.split_at(3).1.to_owned();
                            }
                            flatten_choice_choice(
                                choice,
                                &mut elements,
                                &mut sub_elements,
                                &mut outer_amount,
                                inner_choice_amount,
                                &mut name,
                                inner_name,
                            );
                        }
                        ElementCollection::Sequence {
                            mut sub_elements,
                            name: mut inner_name,
                        } => {
                            if inner_name.is_empty() {
                                inner_name = group_ref.split_at(3).1.to_owned();
                            }
                            if sub_elements.len() == 1 {
                                elements.push(sub_elements[0].clone());
                            } else if outer_amount == ElementAmount::Any {
                                // the outer type is a choice element that allows repetition.
                                // In this situation there is no point in preserving the inner sequence:
                                // sequence elements that occur out of order are equivalen to having multiple smaller ordered sequences
                                elements.append(&mut sub_elements);
                            } else if choice.items.len() == 1 && outer_amount != ElementAmount::Any
                            {
                                replacement = Some(ElementCollection::Sequence {
                                    name: inner_name,
                                    sub_elements,
                                });
                            } else if !sub_elements.is_empty() {
                                elements.push(ElementCollectionItem::GroupRef(format!(
                                    "AR:{inner_name}-ELEMENTGROUP"
                                )));
                            } else {
                                todo!()
                            }
                        }
                    }
                } else {
                    return Err(format!(
                        "Error: unknown group ref {} found in sequence",
                        group_ref
                    ));
                }
            }
            XsdModelGroupItem::Choice(choice_inner) => match flatten_choice(data, choice_inner)? {
                ElementCollection::Choice {
                    mut sub_elements,
                    amount: inner_choice_amount,
                    name: inner_name,
                } => {
                    flatten_choice_choice(
                        choice,
                        &mut elements,
                        &mut sub_elements,
                        &mut outer_amount,
                        inner_choice_amount,
                        &mut name,
                        inner_name,
                    );
                }
                ElementCollection::Sequence { .. } => {
                    todo!();
                }
            },
            XsdModelGroupItem::Element(xsd_element) => {
                elements.push(ElementCollectionItem::Element(Element::new(
                    xsd_element,
                    data.version_info,
                )));
            }
        }
    }

    if let Some(repl) = replacement {
        Ok(repl)
    } else {
        Ok(ElementCollection::Choice {
            sub_elements: elements,
            amount: outer_amount,
            name,
        })
    }
}

fn flatten_choice_choice(
    outer_choice: &XsdChoice,
    elements: &mut Vec<ElementCollectionItem>,
    sub_elements: &mut Vec<ElementCollectionItem>,
    outer_amount: &mut ElementAmount,
    inner_amount: ElementAmount,
    outer_name: &mut String,
    inner_name: String,
) {
    if outer_choice.items.len() == 1 {
        // adjust the amount of the outer choice
        *outer_amount = combine_amounts(*outer_amount, inner_amount);
        elements.append(sub_elements);
        if outer_name.is_empty() && !inner_name.is_empty() {
            *outer_name = inner_name;
        }
    } else if *outer_amount == inner_amount {
        elements.append(sub_elements);
    } else {
        todo!()
    }
}

fn flatten_sequence<'a>(
    data: &'a Xsd,
    sequence: &'a XsdSequence,
) -> Result<ElementCollection, String> {
    let mut flat_items = Vec::new();
    for item in &sequence.items {
        match item {
            XsdModelGroupItem::Group(group_ref) => {
                if let Some(group) = data.groups.get(group_ref) {
                    flat_items.push(flatten_group(data, group)?);
                } else {
                    return Err(format!(
                        "Error: unknown group ref {} found in sequence",
                        group_ref
                    ));
                }
            }
            XsdModelGroupItem::Choice(choice) => {
                flat_items.push(flatten_choice(data, choice)?);
            }
            XsdModelGroupItem::Element(xsd_element) => {
                flat_items.push(ElementCollection::Sequence {
                    name: "".to_string(),
                    sub_elements: vec![ElementCollectionItem::Element(Element::new(
                        xsd_element,
                        data.version_info,
                    ))],
                });
            }
        }
    }

    let nonempty_inputs = flat_items
        .iter()
        .filter(|item| !item.items().is_empty())
        .count();
    let mut elements: Vec<ElementCollectionItem> = Vec::new();
    let mut replacement = None;

    for (idx, item) in flat_items.iter_mut().enumerate() {
        match item {
            // outer: Sequence - content item: Choice
            ElementCollection::Choice {
                name,
                sub_elements,
                amount,
            } => {
                match sub_elements.len() {
                    0 => {}
                    1 => {
                        // choice of only one element is actually no choice at all. The element can be added to the containing sequence
                        // combine the amount of the choice structure and the amount of the single contained element
                        if let ElementCollectionItem::Element(Element {
                            amount: element_amount,
                            ..
                        }) = &mut sub_elements[0]
                        {
                            *element_amount = combine_amounts(*amount, *element_amount);
                        }
                        elements.append(sub_elements);
                    }
                    _ => {
                        // only do anything with this Choice item if it actually contains any elements
                        if nonempty_inputs == 1 {
                            // this Choice item is the only item in the sequence that contains any elements, so the sequence can be turned into a choice
                            replacement = Some(ElementCollection::Choice {
                                sub_elements: sub_elements.clone(),
                                amount: *amount,
                                name: name.clone(),
                            });
                        } else if let XsdModelGroupItem::Group(group_ref) = &sequence.items[idx] {
                            // the choice came from a group, we'll only keep a reference to that group here
                            elements.push(ElementCollectionItem::GroupRef(format!(
                                "{group_ref}-ELEMENTGROUP"
                            )));
                        } else if data.groups.get(&format!("AR:{name}")).is_some() {
                            // the choice came from a group, we'll only keep a reference to that group here
                            elements.push(ElementCollectionItem::GroupRef(format!(
                                "AR:{name}-ELEMENTGROUP"
                            )));
                        } else {
                            todo!()
                        }
                    }
                }
            }
            // outer: Sequence - content item: Sequence
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
            name: "".to_string(),
        })
    }
}

fn flatten_simple_type(
    data: &Xsd,
    simple_type: &XsdSimpleType,
    typename: &str,
) -> Result<CharacterDataType, String> {
    match simple_type {
        XsdSimpleType::Restriction(XsdRestriction::Pattern { pattern, maxlength }) => {
            Ok(CharacterDataType::Pattern {
                pattern: pattern.clone(),
                max_length: *maxlength,
            })
        }
        XsdSimpleType::Restriction(XsdRestriction::Plain { basetype }) => match &**basetype {
            "xsd:string" => Ok(CharacterDataType::String {
                max_length: None,
                preserve_whitespace: false,
            }),
            "xsd:double" => Ok(CharacterDataType::Double),
            "xsd:unsignedInt" => Ok(CharacterDataType::UnsignedInteger),
            "xsd:NMTOKEN" => Ok(CharacterDataType::String {
                max_length: None,
                preserve_whitespace: false,
            }),
            "xsd:NMTOKENS" => Ok(CharacterDataType::String {
                max_length: None,
                preserve_whitespace: false,
            }),
            _ => Err(format!("Error: unknown base type {basetype}")),
        },
        XsdSimpleType::Restriction(XsdRestriction::Literal) => Ok(CharacterDataType::String {
            max_length: None,
            preserve_whitespace: true,
        }),
        XsdSimpleType::Restriction(XsdRestriction::EnumValues { enumvalues }) => {
            let enumitems = enumvalues
                .iter()
                .map(|e| (e.clone(), data.version_info))
                .collect();
            Ok(CharacterDataType::Enum(EnumDefinition {
                name: typename.to_string(),
                enumitems,
            }))
        }
    }
}

fn build_attribute_list(
    data: &Xsd,
    xsd_attributes: &Vec<XsdAttribute>,
    xsd_attribute_groups: &Vec<String>,
) -> Result<Vec<Attribute>, String> {
    let mut attributes = Vec::new();

    for attr in xsd_attributes {
        attributes.push(build_attribute(data, attr)?);
    }

    for attr_group_name in xsd_attribute_groups {
        if attr_group_name == "AR:WHITESPACE-CONTROLLED" {}
        if let Some(attr_group) = data.attribute_groups.get(attr_group_name) {
            for attr in &attr_group.attributes {
                attributes.push(build_attribute(data, attr)?);
            }
        } else {
            return Err(format!(
                "Error: attribute group {} is referenced but not found",
                attr_group_name
            ));
        }
    }

    Ok(attributes)
}

fn build_attribute(data: &Xsd, attr: &XsdAttribute) -> Result<Attribute, String> {
    let attribute_type = if let Some(attr_type) = data.types.get(&attr.typeref) {
        match attr_type {
            XsdType::Base(_) | XsdType::Simple(_) => attr.typeref.clone(),
            XsdType::Complex(_) => {
                return Err("Error: Complex type for attribute ?!?!".to_string());
            }
        }
    } else {
        return Err(format!(
            "Error: attribute references type {}, but the type was not found",
            attr.typeref
        ));
    };

    Ok(Attribute {
        name: attr.name.clone(),
        attribute_type,
        required: attr.required,
        version_info: data.version_info,
    })
}

pub(crate) fn combine_amounts(amount_1: ElementAmount, amount_2: ElementAmount) -> ElementAmount {
    match (amount_1, amount_2) {
        (ElementAmount::ZeroOrOne, ElementAmount::ZeroOrOne)
        | (ElementAmount::ZeroOrOne, ElementAmount::One)
        | (ElementAmount::One, ElementAmount::ZeroOrOne) => ElementAmount::ZeroOrOne,

        (ElementAmount::One, ElementAmount::One) => ElementAmount::One,

        (ElementAmount::ZeroOrOne, ElementAmount::Any)
        | (ElementAmount::One, ElementAmount::Any)
        | (ElementAmount::Any, ElementAmount::ZeroOrOne)
        | (ElementAmount::Any, ElementAmount::One)
        | (ElementAmount::Any, ElementAmount::Any) => ElementAmount::Any,
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
            version_info,
        }
    }
}
