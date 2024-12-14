use crate::generator::name_to_identifier;
use crate::{AutosarDataTypes, CharacterDataType};
use rustc_hash::FxHashMap;

pub(crate) fn generate(autosar_schema: &AutosarDataTypes) -> String {
    let regexes: FxHashMap<String, String> = VALIDATOR_REGEX_MAPPING
        .iter()
        .map(|(regex, name)| ((*regex).to_string(), (*name).to_string()))
        .collect();

    let mut ctnames: Vec<&String> = autosar_schema.character_types.keys().collect();
    ctnames.sort();

    let mut generated = format!(
        "#[rustfmt::skip]\npub(crate) const CHARACTER_DATA: [CharacterDataSpec; {}] = [\n",
        ctnames.len()
    );
    for ctname in &ctnames {
        let chtype = autosar_schema.character_types.get(*ctname).unwrap();

        let chdef = match chtype {
            CharacterDataType::Pattern {
                pattern,
                max_length,
            } => {
                let fullmatch_pattern = format!("^({pattern})$");
                // no longer using proc-macro-regex due to unacceptably long run-times of the proc macro (> 5 Minutes!)
                // if regexes.get(&fullmatch_pattern).is_none() {
                //     let regex_validator_name = format!("validate_regex_{}", regexes.len() + 1);
                //     writeln!(validators, r#"regex!({regex_validator_name} br"{fullmatch_pattern}");"#).unwrap();
                //     regexes.insert(fullmatch_pattern.clone(), regex_validator_name);
                // }
                let regex_validator_name = regexes
                    .get(&fullmatch_pattern)
                    .unwrap_or_else(|| panic!("missing regex: {fullmatch_pattern}"));
                format!(
                    r#"CharacterDataSpec::Pattern{{check_fn: {regex_validator_name}, regex: r"{pattern}", max_length: {max_length:?}}}"#
                )
            }
            CharacterDataType::Enum(enumdef) => {
                let enumitem_strs: Vec<String> = enumdef
                    .enumitems
                    .iter()
                    .map(|(name, ver)| {
                        format!("(EnumItem::{}, 0x{ver:x})", name_to_identifier(name))
                    })
                    .collect();
                format!(
                    r#"CharacterDataSpec::Enum{{items: &[{}]}}"#,
                    enumitem_strs.join(", ")
                )
            }
            CharacterDataType::String {
                max_length,
                preserve_whitespace,
            } => {
                format!(
                    r#"CharacterDataSpec::String{{preserve_whitespace: {preserve_whitespace}, max_length: {max_length:?}}}"#
                )
            }
            CharacterDataType::UnsignedInteger => "CharacterDataSpec::UnsignedInteger".to_string(),
            CharacterDataType::Double => "CharacterDataSpec::Float".to_string(),
        };
        generated.push_str("    ");
        generated.push_str(&chdef);
        generated.push_str(",\n");
    }
    generated.push_str("];\n");

    let (reference_type_idx, _) = ctnames
        .iter()
        .enumerate()
        .find(|(_, name)| **name == "AR:REF--SIMPLE")
        .expect("reference type \"AR:REF--SIMPLE\" not found ?!");
    generated.push_str(&format!(
        "pub(crate) const REFERENCE_TYPE_IDX: u16 = {reference_type_idx};\n"
    ));

    generated
}

// map a regex to a validation function name
static VALIDATOR_REGEX_MAPPING: [(&str, &str); 28] = [
    (r"^(0[xX][0-9a-fA-F]+)$", "validate_regex_1"),
    (
        r"^([1-9][0-9]*|0[xX][0-9a-fA-F]*|0[bB][0-1]+|0[0-7]*|UNSPECIFIED|UNKNOWN|BOOLEAN|PTR)$",
        "validate_regex_2",
    ),
    (
        r"^([1-9][0-9]*|0[xX][0-9a-fA-F]+|0[0-7]*|0[bB][0-1]+|ANY|ALL)$",
        "validate_regex_3",
    ),
    (r"^([0-9]+|ANY)$", "validate_regex_4"),
    (r"^([0-9]+|STRING|ARRAY)$", "validate_regex_5"),
    (r"^(0|1|true|false)$", "validate_regex_6"),
    (r"^([a-zA-Z_][a-zA-Z0-9_]*)$", "validate_regex_7"),
    (r"^([a-zA-Z][a-zA-Z0-9_]*)$", "validate_regex_8"),
    (
        r"^(([0-9]{4}-[0-9]{2}-[0-9]{2})(T[0-9]{2}:[0-9]{2}:[0-9]{2}(Z|([+\-][0-9]{2}:[0-9]{2})))?)$",
        "validate_regex_9",
    ),
    (r"^([a-zA-Z][a-zA-Z0-9-]*)$", "validate_regex_10"),
    (r"^([0-9a-zA-Z_\-]+)$", "validate_regex_11"),
    (
        r"^(%[ \-+#]?[0-9]*(\.[0-9]+)?[bBdiouxXfeEgGcs])$",
        "validate_regex_12",
    ),
    (
        r"^(0|[\+\-]?[1-9][0-9]*|0[xX][0-9a-fA-F]+|0[bB][0-1]+|0[0-7]+)$",
        "validate_regex_13",
    ),
    (
        r"^((25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)|ANY)$",
        "validate_regex_14",
    ),
    (
        r"^([0-9A-Fa-f]{1,4}(:[0-9A-Fa-f]{1,4}){7,7}|ANY)$",
        "validate_regex_15",
    ),
    (
        r"^((0[xX][0-9a-fA-F]+)|(0[0-7]+)|(0[bB][0-1]+)|(([+\-]?[1-9][0-9]+(\.[0-9]+)?|[+\-]?[0-9](\.[0-9]+)?)([eE]([+\-]?)[0-9]+)?)|\.0|INF|-INF|NaN)$",
        "validate_regex_16",
    ),
    (
        r"^(([0-9a-fA-F]{2}:){5}[0-9a-fA-F]{2})$",
        "validate_regex_17",
    ),
    (
        r"^([a-zA-Z_][a-zA-Z0-9_]*(\[([a-zA-Z_][a-zA-Z0-9_]*|[0-9]+)\])*(\.[a-zA-Z_][a-zA-Z0-9_]*(\[([a-zA-Z_][a-zA-Z0-9_]*|[0-9]+)\])*)*)$",
        "validate_regex_18",
    ),
    (r"^([A-Z][a-zA-Z0-9_]*)$", "validate_regex_19"),
    (r"^([1-9][0-9]*)$", "validate_regex_20"),
    (
        r"^(0|[\+]?[1-9][0-9]*|0[xX][0-9a-fA-F]+|0[bB][0-1]+|0[0-7]+)$",
        "validate_regex_21",
    ),
    (
        r"^([a-zA-Z]([a-zA-Z0-9]|_[a-zA-Z0-9])*_?)$",
        "validate_regex_22",
    ),
    (
        r"^(-?([0-9]+|MAX-TEXT-SIZE|ARRAY-SIZE))$",
        "validate_regex_23",
    ),
    (
        r"^(/?[a-zA-Z][a-zA-Z0-9_]{0,127}(/[a-zA-Z][a-zA-Z0-9_]{0,127})*)$",
        "validate_regex_24",
    ),
    (
        r"^([0-9]+\.[0-9]+\.[0-9]+([\._;].*)?)$",
        "validate_regex_25",
    ),
    (
        r"^((0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(-((0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(\.(0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(\+([0-9a-zA-Z-]+(\.[0-9a-zA-Z-]+)*))?)$",
        "validate_regex_26",
    ),
    (r"^([0-1])$", "validate_regex_27"),
    (
        r"^((-?[a-zA-Z_]+)(( )+-?[a-zA-Z_]+)*)$",
        "validate_regex_28",
    ),
];
