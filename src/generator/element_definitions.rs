use crate::generator::{name_to_identifier, SimpleElement};
use crate::{ElementAmount, ElementCollectionItem, XsdRestrictToStandard};
use rustc_hash::FxHashMap;
use std::collections::HashSet;

use super::MergedElementDataType;

pub(crate) fn build_info(
    element_types: &FxHashMap<String, MergedElementDataType>,
) -> Vec<SimpleElement> {
    // make a hashset of all elements to eliminate any duplicates
    let all_elements: HashSet<SimpleElement> = element_types
        .values()
        .filter_map(|etype| etype.collection())
        .flat_map(|ec| {
            ec.items().iter().filter_map(|item| match item {
                ElementCollectionItem::Element(element) => Some(SimpleElement::from(element)),
                ElementCollectionItem::GroupRef(_) => None,
            })
        })
        .collect();
    let mut element_definitions_array: Vec<SimpleElement> = all_elements.into_iter().collect();
    element_definitions_array.sort_by(|e1, e2| {
        e1.name
            .cmp(&e2.name)
            .then(e1.typeref.cmp(&e2.typeref))
            .then(e1.docstring.cmp(&e2.docstring))
            .then(e1.amount.cmp(&e2.amount))
            .then(e1.ordered.cmp(&e2.ordered))
            .then(e1.splittable_ver.cmp(&e2.splittable_ver))
            .then(e1.restrict_std.cmp(&e2.restrict_std))
    });

    // create an element definition for the AUTOSAR element - the xsd files contain this info, but it is lost before we get here
    element_definitions_array.insert(
        0,
        SimpleElement {
            name: String::from("AUTOSAR"),
            typeref: String::from("AR:AUTOSAR"),
            amount: ElementAmount::One,
            splittable_ver: 0xFFFF_FFFF,
            ordered: false,
            restrict_std: XsdRestrictToStandard::NotSet,
            docstring: Some(String::from("Root element of an AUTOSAR description.")),
        },
    );
    element_definitions_array
}

pub(crate) fn build_docstrings_info(
    element_definitions_array: &[SimpleElement],
) -> FxHashMap<String, usize> {
    // first, put all docstrings into a HashSet to elimitate duplicates
    let docstrings: HashSet<String> = element_definitions_array
        .iter()
        .filter_map(|e| e.docstring.clone())
        .collect();
    // transform the HashSet into a Vec and sort the list
    let mut docstrings: Vec<String> = docstrings.into_iter().collect();
    docstrings.sort();
    // enable lookup of entries by transferring everything into a HashMap<docstring, position>

    docstrings
        .into_iter()
        .enumerate()
        .map(|(idx, ds)| (ds, idx))
        .collect()
}

pub(crate) fn generate(
    element_types: &FxHashMap<String, MergedElementDataType>,
    elements: &[SimpleElement],
    docstring_ids: &FxHashMap<String, usize>,
) -> String {
    let mut elemtypenames: Vec<&String> = element_types.keys().collect();
    elemtypenames.sort();
    let elemtype_nameidx: FxHashMap<&str, usize> = elemtypenames
        .iter()
        .enumerate()
        .map(|(idx, name)| (&***name, idx))
        .collect();
    let mut generated = format!(
        "\n#[rustfmt::skip]\npub(crate) static ELEMENTS: [ElementDefinition; {}] = [\n",
        elements.len()
    );
    for (idx, elem) in elements.iter().enumerate() {
        generated.push_str(&build_element_string(
            elem,
            &elemtype_nameidx,
            docstring_ids,
            idx,
        ));
    }
    generated.push_str("];\n");

    let autosar_idx = elements
        .iter()
        .position(|elem| elem.name == "AUTOSAR")
        .unwrap();
    generated.push_str(&format!(
        "\npub(crate) static AUTOSAR_ELEMENT: u16 = {autosar_idx};\n"
    ));

    generated
}

fn build_element_string(
    elem: &SimpleElement,
    elemtype_nameidx: &FxHashMap<&str, usize>,
    docstring_ids: &FxHashMap<String, usize>,
    idx: usize,
) -> String {
    // let mut sub_element_strings: Vec<String> = Vec::new();
    let elem_docstring_id = elem
        .docstring
        .as_ref()
        .and_then(|ds| docstring_ids.get(ds))
        .copied();
    let restrict_txt = restrict_std_to_text(elem.restrict_std);
    format!(
        "    /* {idx:4} */ element!({}, {}, {:?}, {}, 0x{:X}, {}, {:?}),\n",
        name_to_identifier(&elem.name),
        elemtype_nameidx.get(&*elem.typeref).unwrap(),
        elem.amount,
        elem.ordered,
        elem.splittable_ver,
        restrict_txt,
        elem_docstring_id,
    )
}

fn restrict_std_to_text(restrict_std: XsdRestrictToStandard) -> &'static str {
    match restrict_std {
        XsdRestrictToStandard::NotSet | XsdRestrictToStandard::Both => "NotRestricted",
        XsdRestrictToStandard::ClassicPlatform => "ClassicPlatform",
        XsdRestrictToStandard::AdaptivePlatform => "AdaptivePlatform",
    }
}

pub(crate) fn generate_docstrings(docstring_ids: &FxHashMap<String, usize>) -> String {
    let mut docstrings: Vec<String> = docstring_ids.keys().cloned().collect();
    docstrings.sort_by(|a, b| docstring_ids.get(a).cmp(&docstring_ids.get(b)));

    let mut output = String::from("\n#[cfg(feature = \"docstrings\")]\n");
    output.push_str("#[rustfmt::skip]\n");
    output.push_str(&format!(
        "pub(crate) static ELEMENT_DOCSTRINGS: [&str; {}] = [\n",
        docstrings.len()
    ));
    for ds in docstrings {
        output.push_str(&format!("    {ds:?},\n"));
    }
    output.push_str("];\n");
    output
}
