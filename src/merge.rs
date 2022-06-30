use std::collections::HashSet;

use super::*;

struct MergeItems {
    elem_types: Vec<(String, String)>,
    char_types: Vec<(String, String)>,
}

pub(crate) fn merge(merged_xsd: &mut AutosarDataTypes, input_xsd: &AutosarDataTypes) -> Result<(), String> {
    let mut merge_queue = MergeItems::from_vecs(vec![("AR:AUTOSAR".to_owned(), "AR:AUTOSAR".to_owned())], Vec::new());

    let mut already_checked = HashSet::new();
    while let Some((typename_merged, typename_input)) = merge_queue.elem_types.pop() {
        if already_checked.get(&typename_merged).is_none() {
            // println!("  {typename_merged} <-> {typename_input}");

            // typename_merged might not exist in merged_xsd if an element requiring this type was only just copied by the merge
            if merged_xsd.element_types.get(&typename_merged).is_none() {
                if let Some(input_type) = input_xsd.element_types.get(&typename_input) {
                    merged_xsd
                        .element_types
                        .insert(typename_merged.clone(), input_type.clone());
                }
            }

            if merged_xsd.element_types.get(&typename_merged).is_some() {
                let mut additional_items = merge_elem_types(merged_xsd, &typename_merged, input_xsd, &typename_input)?;
                merge_queue.append(&mut additional_items);
            }
            already_checked.insert(typename_merged);
        }
    }

    let mut already_checked = HashSet::new();
    while let Some((typename_merged, typename_input)) = merge_queue.char_types.pop() {
        if already_checked.get(&typename_merged).is_none() {
            if merged_xsd.character_types.get(&typename_merged).is_none() {
                if let Some(input_type) = input_xsd.character_types.get(&typename_input) {
                    merged_xsd
                        .character_types
                        .insert(typename_merged.clone(), input_type.clone());
                }
            }

            if merged_xsd.character_types.get(&typename_merged).is_some() {
                merge_char_types(merged_xsd, &typename_merged, input_xsd, &typename_input)?;
            }

            already_checked.insert(typename_merged);
        }
    }

    Ok(())
}

fn merge_char_types(
    merged_xsd: &mut AutosarDataTypes,
    typename: &str,
    input_xsd: &AutosarDataTypes,
    typename_input: &str,
) -> Result<(), String> {
    let a = merged_xsd.character_types.get_mut(typename).unwrap();
    let b = input_xsd.character_types.get(typename_input).unwrap();

    match (a, b) {
        (CharacterDataType::Enum(enumdef), CharacterDataType::Enum(enumdef_new)) => {
            merge_enums(enumdef, enumdef_new)?;
        }
        (CharacterDataType::Pattern { .. }, CharacterDataType::Pattern { .. }) => {}
        (CharacterDataType::String { .. }, CharacterDataType::String { .. }) => {}
        (CharacterDataType::UnsignedInteger, CharacterDataType::UnsignedInteger) => {}
        (CharacterDataType::Double, CharacterDataType::Double) => {}
        (_aa, _bb) => {
            // println!("mixed character types: {typename}={_aa:#?} - {typename_input}={_bb:#?}");
        }
    }
    Ok(())
}

fn merge_elem_types(
    merged_xsd: &mut AutosarDataTypes,
    typename: &str,
    input_xsd: &AutosarDataTypes,
    typename_input: &str,
) -> Result<MergeItems, String> {
    let a = merged_xsd.element_types.get_mut(typename).unwrap();
    let b = input_xsd.element_types.get(typename_input).unwrap();
    let mut result = MergeItems::new();

    match (a, b) {
        (
            ElementDataType::Elements {
                element_collection,
                attributes,
            },
            ElementDataType::Elements {
                element_collection: elem_collection_new,
                attributes: attributes_new,
            },
        ) => {
            result.append(&mut merge_element_collection(element_collection, elem_collection_new, typename_input)?);
            result.append(&mut merge_attributes(attributes, attributes_new)?);
        }
        (
            ElementDataType::Characters {
                attributes, basetype, ..
            },
            ElementDataType::Characters {
                attributes: attributes_new,
                basetype: basetype_new,
                ..
            },
        ) => {
            result.append(&mut merge_attributes(attributes, attributes_new)?);
            result.char_types.push((basetype.to_string(), basetype_new.to_string()));
        }
        (
            ElementDataType::Mixed {
                element_collection,
                attributes,
                basetype,
                ..
            },
            ElementDataType::Mixed {
                element_collection: elem_collection_new,
                attributes: attributes_new,
                basetype: basetype_new,
                ..
            },
        ) => {
            result.append(&mut merge_element_collection(element_collection, elem_collection_new, typename_input)?);
            result.append(&mut merge_attributes(attributes, attributes_new)?);
            result.char_types.push((basetype.to_string(), basetype_new.to_string()));
        }
        (
            ElementDataType::ElementsGroup { element_collection },
            ElementDataType::ElementsGroup {
                element_collection: elem_collection_new,
            },
        ) => {
            result.append(&mut merge_element_collection(element_collection, elem_collection_new, typename_input)?);
        }
        (
            ElementDataType::Mixed { attributes, .. },
            ElementDataType::Characters {
                attributes: attributes_new,
                ..
            },
        ) => {
            result.append(&mut merge_attributes(attributes, attributes_new)?);
        }
        (_aa, _bb) => {
            // println!("mixed element types: {typename}={_aa:#?} - {typename_input}={_bb:#?}");
        }
    }

    Ok(result)
}

fn merge_element_collection(
    element_collection: &mut ElementCollection,
    element_collection_new: &ElementCollection,
    _src_typename: &str
) -> Result<MergeItems, String> {
    let mut insert_pos: usize = 0;
    let mut typesvec = MergeItems::new();
    // let is_sequence = if let ElementCollection::Sequence { .. } = element_collection {true} else {false};
    match element_collection {
        ElementCollection::Choice { sub_elements, .. } | ElementCollection::Sequence { sub_elements, .. } => {
            for newelem in element_collection_new.items() {
                if let Some(find_pos) = sub_elements
                    .iter()
                    .enumerate()
                    .find(|(_idx, e)| e.name() == newelem.name())
                    .map(|(idx, _e)| idx)
                {
                    // if is_sequence && find_pos < insert_pos {
                    //     println!("ordering difference? {src_typename} -> elem: {}, find_pos: {find_pos} < insert pos {insert_pos}\n{sub_elements:#?}", newelem.name());
                    // }
                    match (&mut sub_elements[find_pos], newelem) {
                        (ElementCollectionItem::Element(cur_elem), ElementCollectionItem::Element(new_elem)) => {
                            cur_elem.version_info |= new_elem.version_info;
                            // println!("    element {} requires merge of types {} and {}", cur_elem.name, cur_elem.typeref, new_elem.typeref);
                            typesvec
                                .elem_types
                                .push((cur_elem.typeref.clone(), new_elem.typeref.clone()));
                        }
                        (ElementCollectionItem::GroupRef(cur_group), ElementCollectionItem::GroupRef(new_group)) => {
                            typesvec.elem_types.push((cur_group.clone(), new_group.clone()));
                        }
                        (a, b) => {
                            return Err(format!("Error: merge of incompatible types: a:{a:#?} b:{b:#?}"));
                        }
                    }
                    //                 // we've found a matching existing item
                    //                 elements[find_pos].version_info |= newelem.version_info;
                    //                 // next possible insertion position is after this item
                    //                 // if find_pos < insert_pos {
                    //                 //     p_check = true;
                    //                 // }
                    insert_pos = find_pos + 1;
                } else {
                    sub_elements.insert(insert_pos, newelem.clone());
                    match &newelem {
                        ElementCollectionItem::Element(Element { typeref, .. })
                        | ElementCollectionItem::GroupRef(typeref) => {
                            typesvec.elem_types.push((typeref.clone(), typeref.clone()));
                            // println!("  # inserting element <{}> of type [{typeref:?}]", newelem.name());
                        }
                    }

                    //                 println!("for element type [{}]: adding {}", elem_type_name, newelem.name());
                    insert_pos += 1;
                }
            }
        }
    }
    Ok(typesvec)
}

fn merge_attributes(attributes: &mut Vec<Attribute>, attributes_new: &Vec<Attribute>) -> Result<MergeItems, String> {
    let mut result = MergeItems::new();
    let mut insert_pos = 0;
    for newattr in attributes_new {
        if let Some(find_pos) = attributes
            .iter()
            .enumerate()
            .find(|(_idx, att)| att.name == newattr.name)
            .map(|(idx, _att)| idx)
        {
            attributes[find_pos].version_info |= newattr.version_info;

            result
                .char_types
                .push((attributes[find_pos].attribute_type.clone(), newattr.attribute_type.clone()));

            insert_pos = find_pos + 1;
        } else {
            attributes.insert(insert_pos, newattr.clone());
            //         println!("for element type [{}]: adding attribute {} [{:?}]", _elem_type_name, newattr.name, newattr.attribute_type);
            insert_pos += 1;
        }
    }

    Ok(result)
}

fn merge_enums(enumdef: &mut EnumDefinition, enumdef_new: &EnumDefinition) -> Result<(), String> {
    let EnumDefinition { name, enumitems } = enumdef;
    let EnumDefinition {
        name: name_new,
        enumitems: enumitems_new,
    } = enumdef_new;

    if name != name_new {
        //return Err(format!("warning: enum name mismatch {} != {}", name, name_new));
    }

    let mut insert_pos = 0;
    for (newitem, newver) in enumitems_new {
        if let Some(find_pos) = enumitems
            .iter()
            .enumerate()
            .find(|(_idx, (enval, _))| enval == newitem)
            .map(|(idx, _)| idx)
        {
            enumitems[find_pos].1 |= newver;
            insert_pos = find_pos + 1;
        } else {
            enumitems.insert(insert_pos, (newitem.clone(), *newver));
            //         println!("for enum {}: adding item {}", name, newitem);
            insert_pos += 1;
        }
    }
    Ok(())
}

impl MergeItems {
    fn new() -> Self {
        Self {
            elem_types: Vec::new(),
            char_types: Vec::new(),
        }
    }

    fn from_vecs(elem_types: Vec<(String, String)>, char_types: Vec<(String, String)>) -> Self {
        Self { elem_types, char_types }
    }

    fn append(&mut self, other: &mut Self) {
        self.elem_types.append(&mut other.elem_types);
        self.char_types.append(&mut other.char_types);
    }
}
