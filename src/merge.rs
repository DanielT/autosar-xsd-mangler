use std::collections::HashMap;

use super::*;

pub(crate) fn merge(merged_xsd: &mut HashMap<String, DataType>, input_xsd: &HashMap<String, DataType>) -> Result<(), String> {
    let keys_base: HashSet<String> = merged_xsd.keys().map(|k| k.clone()).collect();
    let keys_xsd: HashSet<String> = input_xsd.keys().map(|k| k.clone()).collect();
    let mut type_names:Vec<String> = keys_base.union(&keys_xsd).map(|k| k.clone()).collect();
    type_names.sort();

    for type_name in &type_names {
        let element_type = merged_xsd.get_mut(type_name);
        let element_type_new = input_xsd.get(type_name);

        if element_type.is_none() && element_type_new.is_some() {
            merged_xsd.insert(type_name.clone(), element_type_new.unwrap().clone());
    //         println!("added element type [{}]", type_name);
        } else if element_type.is_some() && element_type_new.is_none() {
            //println!("removed element {}", key);
        } else if let (Some(element), Some(element_new)) = (element_type, element_type_new) {
            merge_element_types(type_name, element, element_new)?;
        }
    }
    println!("known element types: {}", type_names.len());
    Ok(())
}


fn merge_element_types(elem_type_name: &str, element: &mut DataType, element_new: &DataType) -> Result<(), String> {
    match (element, element_new) {
        (DataType::Elements { element_collection, attributes },
         DataType::Elements { element_collection: element_collection_new, attributes: attributes_new }) => {
            merge_element_collection(elem_type_name, element_collection, element_collection_new)?;
            merge_attributes(elem_type_name, attributes, attributes_new)?;
        }
        (DataType::Mixed { element_collection, attributes, .. },
         DataType::Mixed { element_collection: element_collection_new, attributes: attributes_new, .. }) => {
            merge_element_collection(elem_type_name, element_collection, element_collection_new)?;
            merge_attributes(elem_type_name, attributes, attributes_new)?;
        }
        (DataType::Characters { .. }, DataType::Characters { .. }) => {
            // nothing to merge
        }
        (DataType::Enum(enumdef), DataType::Enum(enumdef_new)) => {
            merge_enums(enumdef, enumdef_new)?;
        }
        (DataType::ElementsGroup { element_collection }, DataType::ElementsGroup { element_collection: element_collection_new }) => {
            merge_element_collection(elem_type_name, element_collection, element_collection_new)?;
        }
        _ => {
            //return Err(format!("Error: mismatched element content types for <{}>:\n{:#?}\n{:#?}", elem_type_name, cur, new));
        }
    }    
    Ok(())
}


fn merge_element_collection(elem_type_name: &str, element_collection: &mut ElementCollection, element_collection_new: &ElementCollection) -> Result<(), String> {
    match (element_collection, element_collection_new) {
        (ElementCollection::Choice{sub_elements, ..}, ElementCollection::Choice{sub_elements: sub_elements_new, ..}) |
        (ElementCollection::Sequence{sub_elements, ..}, ElementCollection::Sequence{sub_elements: sub_elements_new, ..}) => {
            let mut insert_pos: usize = 0;
    //         // let mut p_check = false;
    //         // let simplified_elements = elements.iter().map(|el| el.name.clone()).collect::<Vec<String>>();
    //         // let simplified_elements_new = elements_new.iter().map(|el| el.name.clone()).collect::<Vec<String>>();
            for newelem in sub_elements_new {
                if let Some(find_pos) = sub_elements
                    .iter()
                    .enumerate()
                    .find(|(_idx, e)| e.name() == newelem.name())
                    .map(|(idx, _e)| idx) {
                        match (&mut sub_elements[find_pos], newelem) {
                            (ElementCollectionItem::Element(cur_elem), ElementCollectionItem::Element(new_elem)) => {
                                cur_elem.version_info |= new_elem.version_info;
                            }
                            (ElementCollectionItem::GroupRef(_), ElementCollectionItem::GroupRef(_)) => {
                                // no merge needed
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
    //                 println!("for element type [{}]: adding {}", elem_type_name, newelem.name());
                    insert_pos += 1;
                }
            }
    //         // if p_check {
    //         //     println!("elem type: [{}]", elem_type_name);
    //         //     println!("before insert: existing = {:?}\nbefore insert: new = {:?}", simplified_elements, simplified_elements_new);
    //         //     let simplified_elements = elements.iter().map(|el| el.name.clone()).collect::<Vec<String>>();
    //         //     println!("after insert: merged = {:?}", simplified_elements);
    //         // }
        }
        (cur, new) => {
            // return Err(format!("Error: mismatched element collection types in <{}>, {:#?}, {:#?}", elem_type_name, cur, new));
        }
    }
    Ok(())
}


fn merge_attributes(_elem_type_name: &str, attributes: &mut Vec<Attribute>, attributes_new: &Vec<Attribute>) -> Result<(), String> {
    let mut insert_pos = 0;
    for newattr in attributes_new {
        if let Some(find_pos) = attributes
            .iter()
            .enumerate()
            .find(|(_idx, att)| att.name == newattr.name)
            .map(|(idx, _att)| idx) {
            attributes[find_pos].version_info |= newattr.version_info;
            insert_pos = find_pos + 1;
        } else {
            attributes.insert(insert_pos, newattr.clone());
    //         println!("for element type [{}]: adding attribute {} [{:?}]", _elem_type_name, newattr.name, newattr.attribute_type);
            insert_pos += 1;
        }
    }

    Ok(())
}


fn merge_enums(enumdef: &mut EnumDefinition, enumdef_new: &EnumDefinition) -> Result<(), String> {
    let EnumDefinition { name, enumitems } = enumdef;
    let EnumDefinition { name: name_new, enumitems: enumitems_new } = enumdef_new;

    if name != name_new {
        return Err(format!("warning: enum name mismatch {} != {}", name, name_new));
    }

    let mut insert_pos = 0;
    for (newitem, newver) in enumitems_new {
        if let Some(find_pos) = enumitems
            .iter()
            .enumerate()
            .find(|(_idx, (enval, _))| enval == newitem)
            .map(|(idx, _)| idx) {
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