use crate::generator::SimpleElement;
use crate::{ElementAmount, ElementCollectionItem, XsdRestrictToStandard};
use rustc_hash::FxHashMap;
use std::collections::HashSet;

use super::{MergedElementDataType, VersionMasks};

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
    version_masks: &VersionMasks,
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
            version_masks,
            idx,
        ));
    }
    generated.push_str("];\n");

    // the docstring ids are stored separately, so that the ELEMENTS array is smaller when the docstrings feature is disabled
    generated.push_str(&format!(
        "\n#[cfg(feature = \"docstrings\")]\n#[rustfmt::skip]\npub(crate) static ELEMENT_DOCSTRING_IDS: [u16; {}] = [\n",
        elements.len()
    ));
    let docstring_id_strs: Vec<String> = elements
        .iter()
        .map(|elem| {
            let id = elem
                .docstring
                .as_ref()
                .and_then(|ds| docstring_ids.get(ds))
                .copied()
                .unwrap_or(NO_DOCSTRING);
            assert!(id <= NO_DOCSTRING);
            format!("{id}")
        })
        .collect();
    for chunk in docstring_id_strs.chunks(20) {
        generated.push_str(&format!("    {},\n", chunk.join(", ")));
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

/// marker value in `ELEMENT_DOCSTRING_IDS` for elements without a docstring
const NO_DOCSTRING: usize = u16::MAX as usize;

fn build_element_string(
    elem: &SimpleElement,
    elemtype_nameidx: &FxHashMap<&str, usize>,
    version_masks: &VersionMasks,
    idx: usize,
) -> String {
    let restrict_txt = restrict_std_to_text(elem.restrict_std);
    format!(
        "    /* {idx:4} {} */ element!({}, {:?}, {}, {}, {}),\n",
        elem.name,
        elemtype_nameidx.get(&*elem.typeref).unwrap(),
        elem.amount,
        elem.ordered,
        version_masks.idx(elem.splittable_ver),
        restrict_txt,
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

    // all docstrings are concatenated into one string, and an offset table gives the position of each docstring.
    // This avoids the overhead of a fat pointer (and a relocation) for each docstring
    let mut offsets = vec![0];
    let mut output = String::from("\n#[cfg(feature = \"docstrings\")]\n");
    output.push_str("#[rustfmt::skip]\n");
    output.push_str("pub(crate) static ELEMENT_DOCSTRINGS: &str = concat!(\n");
    let mut total_len = 0;
    for ds in &docstrings {
        total_len += ds.len();
        offsets.push(total_len);
        output.push_str(&format!("    {ds:?},\n"));
    }
    output.push_str(");\n");
    assert!(u32::try_from(total_len).is_ok());

    output.push_str("\n#[cfg(feature = \"docstrings\")]\n");
    output.push_str("#[rustfmt::skip]\n");
    output.push_str(&format!(
        "pub(crate) static ELEMENT_DOCSTRING_OFFSETS: [u32; {}] = [\n",
        offsets.len()
    ));
    for chunk in offsets.chunks(10) {
        let chunk_strs: Vec<String> = chunk.iter().map(ToString::to_string).collect();
        output.push_str(&format!("    {},\n", chunk_strs.join(", ")));
    }
    output.push_str("];\n");
    output
}
