use std::collections::HashSet;

use super::{
    Attribute, AutosarDataTypes, CharacterDataType, Element, ElementAmount, ElementCollection,
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
        if !already_checked.contains(&elem_or_group) {
            match &elem_or_group {
                ElemOrGroup::Element(typename_merged, typename_input) => {
                    // typename_merged might not exist in merged_xsd if an element requiring this type was only just copied by the merge
                    if !merged_xsd.element_types.contains_key(typename_merged) {
                        if let Some(input_type) = input_xsd.element_types.get(typename_input) {
                            merged_xsd
                                .element_types
                                .insert(typename_merged.clone(), input_type.clone());
                        }
                    }
                    if merged_xsd.element_types.contains_key(typename_merged) {
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
                    if !merged_xsd.group_types.contains_key(typename_merged) {
                        if let Some(input_type) = input_xsd.group_types.get(typename_input) {
                            merged_xsd
                                .group_types
                                .insert(typename_merged.clone(), input_type.clone());
                        }
                    }
                    if merged_xsd.group_types.contains_key(typename_merged) {
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
        if !already_checked.contains(&(typename_merged.clone(), typename_input.clone())) {
            if !merged_xsd.character_types.contains_key(&typename_merged) {
                if let Some(input_type) = input_xsd.character_types.get(&typename_input) {
                    merged_xsd
                        .character_types
                        .insert(typename_merged.clone(), input_type.clone());
                }
            }

            if merged_xsd.character_types.contains_key(&typename_merged) {
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
    let AutosarDataTypes {
        element_types,
        character_types,
        ..
    } = merged_xsd;
    let a = element_types.get_mut(typename).unwrap();
    let b = input_xsd.element_types.get(typename_input).unwrap();
    let mut result = MergeItems::new();

    match (a, b) {
        (
            ElementDataType::Elements {
                group_ref,
                attributes,
                xsd_typenames,
                ..
            },
            ElementDataType::Elements {
                group_ref: group_ref_new,
                attributes: attributes_new,
                xsd_typenames: xsd_typenames_new,
                ..
            },
        ) => {
            result
                .elem_types
                .push(ElemOrGroup::Group(group_ref.clone(), group_ref_new.clone()));
            result.append(&mut merge_attributes(
                attributes,
                attributes_new,
                character_types,
                input_xsd,
            ));
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
            result.append(&mut merge_attributes(
                attributes,
                attributes_new,
                character_types,
                input_xsd,
            ));
            separate_cross_merged_enum(character_types, input_xsd, basetype, basetype_new);
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
            result.append(&mut merge_attributes(
                attributes,
                attributes_new,
                character_types,
                input_xsd,
            ));
            separate_cross_merged_enum(character_types, input_xsd, basetype, basetype_new);
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
            result.append(&mut merge_attributes(
                attributes,
                attributes_new,
                character_types,
                input_xsd,
            ));
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
    // work on a copy of the group, so that merged_xsd remains available for type lookups and insertions
    let mut element_collection = merged_xsd.group_types.get(typename).unwrap().clone();
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
    match &mut element_collection {
        ElementCollection::Choice { sub_elements, .. }
        | ElementCollection::Sequence { sub_elements, .. } => {
            for newelem in element_collection_new.items() {
                if let Some(find_pos) = sub_elements.iter().position(|e| {
                    e.name() == newelem.name()
                        && element_is_compatible(e, newelem, merged_xsd, input_xsd)
                }) {
                    // if is_sequence && find_pos < insert_pos {
                    //     println!("ordering difference? {src_typename} -> elem: {}, find_pos: {find_pos} < insert pos {insert_pos}\n{sub_elements:#?}", newelem.name());
                    // }
                    match (&mut sub_elements[find_pos], newelem) {
                        (
                            ElementCollectionItem::Element(cur_elem),
                            ElementCollectionItem::Element(new_elem),
                        ) => {
                            cur_elem.version_info |= new_elem.version_info;
                            cur_elem.splittable_ver |= new_elem.splittable_ver;
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
                    let mut newelem = newelem.clone();
                    match &mut newelem {
                        ElementCollectionItem::Element(Element {
                            typeref,
                            version_info,
                            ..
                        }) => {
                            let typeref_input = typeref.clone();
                            if !element_types_compatible(
                                merged_xsd,
                                &typeref_input,
                                input_xsd,
                                &typeref_input,
                            ) {
                                // a different type with the same name already exists in the merged data;
                                // the older type must be stored under a new name to keep it separate
                                *typeref = copy_renamed_type(
                                    merged_xsd,
                                    input_xsd,
                                    &typeref_input,
                                    *version_info,
                                );
                            }
                            typesvec
                                .elem_types
                                .push(ElemOrGroup::Element(typeref.clone(), typeref_input));
                        }
                        ElementCollectionItem::GroupRef(typeref) => {
                            typesvec
                                .elem_types
                                .push(ElemOrGroup::Group(typeref.clone(), typeref.clone()));
                        }
                    }
                    sub_elements.insert(insert_pos, newelem);

                    insert_pos += 1;
                }
            }
        }
    }
    merged_xsd
        .group_types
        .insert(typename.to_owned(), element_collection);
    Ok(typesvec)
}

/// Copy the element type `typename` from `input_xsd` into `merged_xsd` under a new name derived from
/// the version, because `merged_xsd` already contains an incompatible type with the same name.
/// The group of the copied type is renamed as well, so that it is not merged into the existing group.
/// Likewise, its character data type is copied under a new name if the existing one is incompatible.
fn copy_renamed_type(
    merged_xsd: &mut AutosarDataTypes,
    input_xsd: &AutosarDataTypes,
    typename: &str,
    version_info: usize,
) -> String {
    let new_name = format!("{typename}@{version_info:x}");
    if !merged_xsd.element_types.contains_key(&new_name) {
        let mut new_type = input_xsd.element_types.get(typename).unwrap().clone();
        match &mut new_type {
            ElementDataType::Elements { group_ref, .. }
            | ElementDataType::Mixed { group_ref, .. } => {
                *group_ref = format!("{group_ref}@{version_info:x}");
            }
            ElementDataType::Characters { basetype, .. } => {
                let input_chartype = input_xsd.character_types.get(basetype.as_str());
                if !char_types_compatible(
                    merged_xsd.character_types.get(basetype.as_str()),
                    input_chartype,
                ) {
                    let new_basetype = format!("{basetype}@{version_info:x}");
                    if let Some(chartype) = input_chartype {
                        merged_xsd
                            .character_types
                            .entry(new_basetype.clone())
                            .or_insert_with(|| chartype.clone());
                    }
                    *basetype = new_basetype;
                }
            }
        }
        merged_xsd.element_types.insert(new_name.clone(), new_type);
    }
    new_name
}

fn merge_attributes(
    attributes: &mut Vec<Attribute>,
    attributes_new: &Vec<Attribute>,
    character_types: &mut FxHashMap<String, CharacterDataType>,
    input_xsd: &AutosarDataTypes,
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
            // attributes are looked up by name only, so they can't be split by version like elements
            let attr_type = &attributes[find_pos].attr_type;
            if !char_types_compatible(
                character_types
                    .get(attr_type)
                    .or_else(|| input_xsd.character_types.get(attr_type)),
                input_xsd.character_types.get(&newattr.attr_type),
            ) {
                println!(
                    "warning: attribute {}: type {attr_type} does not accept all values of the older type {}",
                    newattr.name, newattr.attr_type
                );
            }
            separate_cross_merged_enum(
                character_types,
                input_xsd,
                &mut attributes[find_pos].attr_type,
                &newattr.attr_type,
            );

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

/// Prevent enum items from leaking between differently named enum types.
///
/// `typename` (in the merged data) is about to be merged with the enum `typename_input`
/// from an older version, because one attribute or element switched between the two types.
/// Merging in place would add the items of `typename_input` to every user of `typename`.
/// If `typename_input` contains items that the older version of `typename` does not have,
/// the reference is pointed at a combined copy "`typename`|`typename_input`" instead.
fn separate_cross_merged_enum(
    character_types: &mut FxHashMap<String, CharacterDataType>,
    input_xsd: &AutosarDataTypes,
    typename: &mut String,
    typename_input: &str,
) {
    // same type, or a combined type which already includes the input type
    if typename.split('|').any(|part| part == typename_input) {
        return;
    }
    let (Some(CharacterDataType::Enum(enumdef)), Some(CharacterDataType::Enum(enumdef_input))) = (
        character_types.get(typename.as_str()),
        input_xsd.character_types.get(typename_input),
    ) else {
        return;
    };

    // Items the merged enum may legitimately gain in the input version: those of the
    // same-named enum in the input. Without such an enum, only the items it already has.
    let reference_items = match input_xsd.character_types.get(typename.as_str()) {
        Some(CharacterDataType::Enum(enumdef_same_name)) => &enumdef_same_name.enumitems,
        _ => &enumdef.enumitems,
    };
    let adds_items = enumdef_input
        .enumitems
        .iter()
        .any(|(item, _)| !reference_items.iter().any(|(refitem, _)| refitem == item));
    if !adds_items {
        return;
    }

    let combined_name = format!("{typename}|{typename_input}");
    if !character_types.contains_key(&combined_name) {
        let combined = CharacterDataType::Enum(EnumDefinition {
            name: combined_name.clone(),
            enumitems: enumdef.enumitems.clone(),
        });
        character_types.insert(combined_name.clone(), combined);
    }
    *typename = combined_name;
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

/// The content model of an element type, used to decide if two element types can be merged
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypeShape {
    Characters,
    Mixed,
    Sequence,
    Choice,
    Bag,
}

fn type_shape(autosar_types: &AutosarDataTypes, typename: &str) -> Option<TypeShape> {
    match autosar_types.element_types.get(typename)? {
        ElementDataType::Characters { .. } => Some(TypeShape::Characters),
        ElementDataType::Mixed { .. } => Some(TypeShape::Mixed),
        ElementDataType::Elements { group_ref, .. } => {
            match autosar_types.group_types.get(group_ref)? {
                ElementCollection::Sequence { .. } => Some(TypeShape::Sequence),
                ElementCollection::Choice {
                    amount: ElementAmount::Any,
                    ..
                } => Some(TypeShape::Bag),
                ElementCollection::Choice { .. } => Some(TypeShape::Choice),
            }
        }
    }
}

/// Can content of shape `older` be merged into a type of shape `merged`?
/// This is possible if the shapes are equal, or if `merged` accepts everything that `older` accepts.
fn type_shapes_compatible(merged: Option<TypeShape>, older: Option<TypeShape>) -> bool {
    match (merged, older) {
        (Some(merged), Some(older)) => {
            merged == older
                || matches!(
                    (merged, older),
                    (
                        TypeShape::Mixed,
                        TypeShape::Characters
                            | TypeShape::Sequence
                            | TypeShape::Choice
                            | TypeShape::Bag
                    ) | (TypeShape::Bag, TypeShape::Sequence | TypeShape::Choice)
                )
        }
        // the type is unknown on at least one side, so there is nothing to compare
        _ => true,
    }
}

fn element_is_compatible(
    item: &ElementCollectionItem,
    item_new: &ElementCollectionItem,
    merged_xsd: &AutosarDataTypes,
    input_xsd: &AutosarDataTypes,
) -> bool {
    match (item, item_new) {
        (
            ElementCollectionItem::Element(Element { typeref, .. }),
            ElementCollectionItem::Element(Element {
                typeref: typeref_new,
                ..
            }),
        ) => element_types_compatible(merged_xsd, typeref, input_xsd, typeref_new),
        _ => true,
    }
}

/// Can the content of the element type `typename_input` (from an older version) be merged into
/// the element type `typename` in the merged data, without rejecting any content that was valid before?
fn element_types_compatible(
    merged_xsd: &AutosarDataTypes,
    typename: &str,
    input_xsd: &AutosarDataTypes,
    typename_input: &str,
) -> bool {
    // the merged type may have been copied from the input during the current merge,
    // in which case it (and its character data type) has not been copied yet
    let shape = type_shape(merged_xsd, typename).or_else(|| type_shape(input_xsd, typename));
    let shape_input = type_shape(input_xsd, typename_input);
    if !type_shapes_compatible(shape, shape_input) {
        return false;
    }

    let basetype = merged_xsd
        .element_types
        .get(typename)
        .or_else(|| input_xsd.element_types.get(typename))
        .and_then(ElementDataType::basetype);
    let basetype_input = input_xsd
        .element_types
        .get(typename_input)
        .and_then(ElementDataType::basetype);
    match (basetype, basetype_input) {
        (Some(basetype), Some(basetype_input)) => char_types_compatible(
            merged_xsd
                .character_types
                .get(basetype)
                .or_else(|| input_xsd.character_types.get(basetype)),
            input_xsd.character_types.get(basetype_input),
        ),
        _ => true,
    }
}

/// Can character data of type `older` be merged into the character data type `merged`?
///
/// Enums are merged item by item, and patterns of the same type are assumed to
/// evolve compatibly (the newest pattern is kept). Any other change of the
/// character data type is only compatible if the merged type accepts arbitrary strings.
fn char_types_compatible(
    merged: Option<&CharacterDataType>,
    older: Option<&CharacterDataType>,
) -> bool {
    match (merged, older) {
        (Some(merged), Some(older)) => matches!(
            (merged, older),
            (CharacterDataType::String { .. }, _)
                | (CharacterDataType::Enum(_), CharacterDataType::Enum(_))
                | (
                    CharacterDataType::Pattern { .. },
                    CharacterDataType::Pattern { .. }
                )
                | (
                    CharacterDataType::UnsignedInteger,
                    CharacterDataType::UnsignedInteger
                )
                | (CharacterDataType::Double, CharacterDataType::Double)
        ),
        // the type is unknown on at least one side, so there is nothing to compare
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
