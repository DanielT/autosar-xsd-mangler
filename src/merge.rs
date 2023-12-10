use std::collections::HashSet;

use super::{
    Attribute, AutosarDataTypes, CharacterDataType, Element, ElementCollection,
    ElementCollectionItem, ElementDataType, EnumDefinition, FxHashMap,
};

#[derive(Debug, Eq, PartialEq, Hash)]
enum ElemOrGroup {
    Element(String, String),
    Group(String, String),
}

#[derive(Debug)]
struct MergeItems {
    elem_types: Vec<ElemOrGroup>,
    char_types: Vec<(String, String)>,
}

// merge the content of of input_xsd into merged_xsd
// merged_xsd is modified, input_xsd is not
pub(crate) fn merge(
    merged_xsd: &mut AutosarDataTypes,
    input_xsd: &AutosarDataTypes,
) -> Result<(), String> {
    // begin the merge at the top-level AR:AUTOSAR type, which must exist by definition
    let mut merge_queue = MergeItems::from_vecs(
        vec![ElemOrGroup::Element(
            "AR:AUTOSAR".to_owned(),
            "AR:AUTOSAR".to_owned(),
        )],
        Vec::new(),
    );

    let mut already_checked: HashSet<ElemOrGroup> = HashSet::new();
    // while let Some((typename_merged, typename_input)) = merge_queue.elem_types.pop() {
    while let Some(elem_or_group) = merge_queue.elem_types.pop() {
        if already_checked.get(&elem_or_group).is_none() {
            match &elem_or_group {
                ElemOrGroup::Element(typename_merged, typename_input) => {
                    // typename_merged might not exist in merged_xsd if an element requiring this type was only just copied by the merge
                    if merged_xsd.element_types.get(typename_merged).is_none() {
                        if let Some(input_type) = input_xsd.element_types.get(typename_input) {
                            merged_xsd
                                .element_types
                                .insert(typename_merged.clone(), input_type.clone());
                        }
                    }
                    if merged_xsd.element_types.get(typename_merged).is_some() {
                        let mut additional_items = merge_elem_types(
                            merged_xsd,
                            typename_merged,
                            input_xsd,
                            typename_input,
                        );
                        merge_queue.append(&mut additional_items);
                    }
                    already_checked.insert(elem_or_group);
                }
                ElemOrGroup::Group(typename_merged, typename_input) => {
                    // typename_merged might not exist in merged_xsd if an element requiring this type was only just copied by the merge
                    if merged_xsd.group_types.get(typename_merged).is_none() {
                        if let Some(input_type) = input_xsd.group_types.get(typename_input) {
                            merged_xsd
                                .group_types
                                .insert(typename_merged.clone(), input_type.clone());
                        }
                    }
                    if merged_xsd.group_types.get(typename_merged).is_some() {
                        let mut additional_items = merge_group_types(
                            merged_xsd,
                            typename_merged,
                            input_xsd,
                            typename_input,
                        )?;
                        merge_queue.append(&mut additional_items);
                    }
                    already_checked.insert(elem_or_group);
                }
            }
        }
    }

    let mut already_checked = HashSet::new();
    while let Some((typename_merged, typename_input)) = merge_queue.char_types.pop() {
        if already_checked
            .get(&(typename_merged.clone(), typename_input.clone()))
            .is_none()
        {
            if merged_xsd.character_types.get(&typename_merged).is_none() {
                if let Some(input_type) = input_xsd.character_types.get(&typename_input) {
                    merged_xsd
                        .character_types
                        .insert(typename_merged.clone(), input_type.clone());
                }
            }

            if merged_xsd.character_types.get(&typename_merged).is_some() {
                merge_char_types(merged_xsd, &typename_merged, input_xsd, &typename_input);
            }

            already_checked.insert((typename_merged, typename_input));
        }
    }

    Ok(())
}

fn merge_char_types(
    merged_xsd: &mut AutosarDataTypes,
    typename: &str,
    input_xsd: &AutosarDataTypes,
    typename_input: &str,
) {
    let a = merged_xsd.character_types.get_mut(typename).unwrap();
    let b = input_xsd.character_types.get(typename_input).unwrap();

    match (a, b) {
        (CharacterDataType::Enum(enumdef), CharacterDataType::Enum(enumdef_new)) => {
            merge_enums(enumdef, enumdef_new);
        }
        (CharacterDataType::Pattern { .. }, CharacterDataType::Pattern { .. })
        | (CharacterDataType::String { .. }, CharacterDataType::String { .. })
        | (CharacterDataType::UnsignedInteger, CharacterDataType::UnsignedInteger)
        | (CharacterDataType::Double, CharacterDataType::Double) => {}
        (_aa, _bb) => {
            // println!("mixed character types: {typename}={_aa:#?} - {typename_input}={_bb:#?}");
        }
    }
}

fn merge_elem_types(
    merged_xsd: &mut AutosarDataTypes,
    typename: &str,
    input_xsd: &AutosarDataTypes,
    typename_input: &str,
) -> MergeItems {
    let a = merged_xsd.element_types.get_mut(typename).unwrap();
    let b = input_xsd.element_types.get(typename_input).unwrap();
    let mut result = MergeItems::new();

    match (a, b) {
        (
            ElementDataType::Elements {
                group_ref,
                attributes,
                xsd_typenames,
            },
            ElementDataType::Elements {
                group_ref: group_ref_new,
                attributes: attributes_new,
                xsd_typenames: xsd_typenames_new,
            },
        ) => {
            result
                .elem_types
                .push(ElemOrGroup::Group(group_ref.clone(), group_ref_new.clone()));
            result.append(&mut merge_attributes(attributes, attributes_new));
            for xtn in xsd_typenames_new {
                // most of these are duplicates, but that doesn't matter
                xsd_typenames.insert(xtn.to_owned());
            }
        }
        (
            ElementDataType::Characters {
                attributes,
                basetype,
                ..
            },
            ElementDataType::Characters {
                attributes: attributes_new,
                basetype: basetype_new,
                ..
            },
        ) => {
            result.append(&mut merge_attributes(attributes, attributes_new));
            result
                .char_types
                .push(((*basetype).to_string(), basetype_new.to_string()));
        }
        (
            ElementDataType::Mixed {
                group_ref,
                attributes,
                basetype,
                ..
            },
            ElementDataType::Mixed {
                group_ref: group_ref_new,
                attributes: attributes_new,
                basetype: basetype_new,
                ..
            },
        ) => {
            result
                .elem_types
                .push(ElemOrGroup::Group(group_ref.clone(), group_ref_new.clone()));
            result.append(&mut merge_attributes(attributes, attributes_new));
            result
                .char_types
                .push(((*basetype).to_string(), basetype_new.to_string()));
        }
        (
            ElementDataType::Mixed { attributes, .. },
            ElementDataType::Characters {
                attributes: attributes_new,
                ..
            },
        ) => {
            result.append(&mut merge_attributes(attributes, attributes_new));
        }
        (_aa, _bb) => {
            // println!("mixed element types: {typename}={_aa:#?} - {typename_input}={_bb:#?}");
        }
    }

    result
}

fn merge_group_types(
    merged_xsd: &mut AutosarDataTypes,
    typename: &str,
    input_xsd: &AutosarDataTypes,
    typename_input: &str,
) -> Result<MergeItems, String> {
    let element_collection = merged_xsd.group_types.get_mut(typename).unwrap();
    let element_collection_new = input_xsd.group_types.get(typename_input).unwrap();
    let mut insert_pos: usize = 0;

    // never insert any element in position 0 ahead of the SHORT-NAME
    if let Some(ElementCollectionItem::Element(Element { ref name, .. })) =
        element_collection.items().first()
    {
        if name == "SHORT-NAME" {
            insert_pos = 1;
        }
    }

    let mut typesvec = MergeItems::new();
    // let is_sequence = if let ElementCollection::Sequence { .. } = element_collection {true} else {false};
    match element_collection {
        ElementCollection::Choice { sub_elements, .. }
        | ElementCollection::Sequence { sub_elements, .. } => {
            for newelem in element_collection_new.items() {
                if let Some(find_pos) = sub_elements
                    .iter()
                    .enumerate()
                    .find(|(_idx, e)| {
                        e.name() == newelem.name()
                            && element_is_compatible(
                                e,
                                newelem,
                                &merged_xsd.character_types,
                                &input_xsd.character_types,
                            )
                    })
                    // .find(|(_idx, e)| e.name() == newelem.name())
                    .map(|(idx, _e)| idx)
                {
                    // if is_sequence && find_pos < insert_pos {
                    //     println!("ordering difference? {src_typename} -> elem: {}, find_pos: {find_pos} < insert pos {insert_pos}\n{sub_elements:#?}", newelem.name());
                    // }
                    match (&mut sub_elements[find_pos], newelem) {
                        (
                            ElementCollectionItem::Element(cur_elem),
                            ElementCollectionItem::Element(new_elem),
                        ) => {
                            cur_elem.version_info |= new_elem.version_info;
                            typesvec.elem_types.push(ElemOrGroup::Element(
                                cur_elem.typeref.clone(),
                                new_elem.typeref.clone(),
                            ));
                        }
                        (
                            ElementCollectionItem::GroupRef(cur_group),
                            ElementCollectionItem::GroupRef(new_group),
                        ) => {
                            typesvec
                                .elem_types
                                .push(ElemOrGroup::Group(cur_group.clone(), new_group.clone()));
                        }
                        (a, b) => {
                            return Err(format!(
                                "Error: merge of incompatible types: a:{a:#?} b:{b:#?}"
                            ));
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
                        ElementCollectionItem::Element(Element { typeref, .. }) => {
                            typesvec
                                .elem_types
                                .push(ElemOrGroup::Element(typeref.clone(), typeref.clone()));
                        }
                        ElementCollectionItem::GroupRef(typeref) => {
                            typesvec
                                .elem_types
                                .push(ElemOrGroup::Group(typeref.clone(), typeref.clone()));
                        }
                    }

                    insert_pos += 1;
                }
            }
        }
    }
    Ok(typesvec)
}

fn merge_attributes(
    attributes: &mut Vec<Attribute>,
    attributes_new: &Vec<Attribute>,
) -> MergeItems {
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

            result.char_types.push((
                attributes[find_pos].attr_type.clone(),
                newattr.attr_type.clone(),
            ));

            insert_pos = find_pos + 1;
        } else {
            attributes.insert(insert_pos, newattr.clone());
            insert_pos += 1;
        }
    }

    result
}

fn merge_enums(enumdef: &mut EnumDefinition, enumdef_new: &EnumDefinition) {
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
            insert_pos += 1;
        }
    }
}

fn element_is_compatible(
    item: &ElementCollectionItem,
    item_new: &ElementCollectionItem,
    character_types: &FxHashMap<String, CharacterDataType>,
    character_types_new: &FxHashMap<String, CharacterDataType>,
) -> bool {
    match (item, item_new) {
        (
            ElementCollectionItem::Element(Element { typeref, .. }),
            ElementCollectionItem::Element(Element {
                typeref: typeref_new,
                ..
            }),
        ) => {
            matches!(
                (
                    character_types.get(typeref),
                    character_types_new.get(typeref_new),
                ),
                (
                    Some(CharacterDataType::Pattern { .. }),
                    Some(CharacterDataType::Pattern { .. }),
                ) | (
                    Some(CharacterDataType::Enum(_)),
                    Some(CharacterDataType::Enum(_))
                ) | (
                    Some(CharacterDataType::String { .. }),
                    Some(CharacterDataType::String { .. }),
                ) | (
                    Some(CharacterDataType::UnsignedInteger),
                    Some(CharacterDataType::UnsignedInteger),
                ) | (
                    Some(CharacterDataType::Double),
                    Some(CharacterDataType::Double)
                ) | (None, None)
            )
        }
        _ => true,
    }
}

impl MergeItems {
    fn new() -> Self {
        Self {
            elem_types: Vec::new(),
            char_types: Vec::new(),
        }
    }

    fn from_vecs(elem_types: Vec<ElemOrGroup>, char_types: Vec<(String, String)>) -> Self {
        Self {
            elem_types,
            char_types,
        }
    }

    fn append(&mut self, other: &mut Self) {
        self.elem_types.append(&mut other.elem_types);
        self.char_types.append(&mut other.char_types);
    }
}
