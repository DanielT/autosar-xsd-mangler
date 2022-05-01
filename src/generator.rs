use super::*;
use std::fmt::Write;

pub(crate) fn generate(autosar_types: &HashMap<String, DataType>) -> Result<(), String> {
    let generated = genenerate_types(autosar_types)?;

    println!("-----------------------\n{generated}");

    Ok(())
}


fn genenerate_types(autosar_types: &HashMap<String, DataType>) -> Result<String, String> {
    let mut names: Vec<&String> = autosar_types.keys().collect();
    let mut outstr = String::with_capacity(1000);
    names.sort();
    for elemtype_name in &names {
        match autosar_types.get(*elemtype_name).unwrap() {
            DataType::Elements { element_collection, attributes } => {
                outstr.write_str(&generate_datatype_elements(autosar_types, elemtype_name, element_collection, attributes)?).unwrap();
            }
            DataType::Characters { basetype, attributes, .. } => {
                println!("Characters: {:40} -> {} [[{}]]", elemtype_name, derive_type_name(autosar_types, elemtype_name), basetype);
            }
            DataType::Mixed { element_collection, basetype, attributes } => {
                println!("Mixed: {:40} -> {}", elemtype_name, derive_type_name(autosar_types, elemtype_name));
                match element_collection {
                    ElementCollection::Choice { name, sub_elements, amount } => {
                        assert_eq!(*amount, ElementAmount::Any);
                        // println!("{name} = {sub_elements:#?} - {attributes:#?}");
                        assert!(sub_elements.len() > 0);
                    }
                    ElementCollection::Sequence { name, sub_elements } => todo!(),
                }
            }
            DataType::Enum(_) => {
                println!("Enum: {:40} -> {}", elemtype_name, derive_type_name(autosar_types, elemtype_name));
            }
            DataType::ElementsGroup { element_collection } => {
                outstr.write_str(&generate_datatype_elementgroup(autosar_types, elemtype_name, element_collection)?).unwrap();
            }
        }
    }

    Ok(outstr)
}


fn generate_datatype_elements(autosar_types: &HashMap<String, DataType>, elemtype_name: &str, element_collection: &ElementCollection, attributes: &Vec<Attribute>) -> Result<String, String> {
    let mut outstr = String::new();
    let rust_typename = derive_type_name(autosar_types, elemtype_name);
    let mut content_group: Option<String> = None;
    writeln!(outstr, "pub struct {rust_typename} {{").unwrap();
    outstr.write_str(&generate_attribute_fields(autosar_types, attributes)?).unwrap();
    match element_collection {
        ElementCollection::Choice { sub_elements, amount, .. } => {
            if sub_elements.len() == 0 {
                todo!()
            } else if sub_elements.len() == 1 {
                let (partial_elem_type, elem_amount) = generate_element_type(autosar_types, &sub_elements[0]);
                let combined_amount = flatten::combine_amounts(elem_amount, *amount);
                let elem_type = wrap_element_type(&partial_elem_type, combined_amount);
                let varname = generate_element_varname(&sub_elements[0]);
                writeln!(outstr, "    pub {varname}: {elem_type},").unwrap();
            } else {
                // let mut groupstr = "".to_string();
                // writeln!(groupstr, "pub enum {rust_typename}Content {{").unwrap();
                // for ec_item in sub_elements {
                //     let (partial_elem_type, elem_amount) = generate_element_type(ec_item);
                //     let elem_type = wrap_element_type(&partial_elem_type, elem_amount);
                //     writeln!(groupstr, "    {partial_elem_type}({elem_type}),").unwrap();
                // }
                // writeln!(groupstr, "}}\n").unwrap();
                // let content_group = Some(groupstr);
                // let elem_type = wrap_element_type(&partial_elem_type, amount);
                // writeln!(outstr, "    pub {varname}: {elem_type},").unwrap();
            }
        }
        ElementCollection::Sequence { sub_elements, .. } => {
            for ec_item in sub_elements {
                let (partial_elem_type, elem_amount) = generate_element_type(autosar_types, ec_item);
                let elem_type = wrap_element_type(&partial_elem_type, elem_amount);
                let varname = generate_element_varname(ec_item);
                writeln!(outstr, "    pub {varname}: {elem_type},").unwrap();
            }
        }
    }
    writeln!(outstr, "}}\n").unwrap();
    Ok(outstr)
}


fn generate_datatype_elementgroup(autosar_types: &HashMap<String, DataType>, elemtype_name: &str, element_collection: &ElementCollection) -> Result<String, String> {
    let mut outstr = String::new();
    let rust_typename = derive_type_name(autosar_types, elemtype_name);
    
    match element_collection {
        ElementCollection::Choice { sub_elements, amount, .. } => {
            writeln!(outstr, "pub enum {rust_typename} {{").unwrap();
            println!("amount of elementgroup {elemtype_name}: {amount:#?}");
            for element in sub_elements {
                writeln!(outstr, "    // {}", element.name()).unwrap();
            }
        }
        ElementCollection::Sequence { name, sub_elements } => {
            writeln!(outstr, "pub struct {rust_typename} {{").unwrap();
            for element in sub_elements {
                writeln!(outstr, "    // {}", element.name()).unwrap();
            }
        }
    }

    writeln!(outstr, "}}\n").unwrap();
    Ok(outstr)
}


fn derive_type_name(autosar_types: &HashMap<String, DataType>, autosar_name: &str) -> String {
    if let Some(DataType::Characters { basetype, .. }) = autosar_types.get(autosar_name) {
        match &**basetype {
            "xsd:double" => "f64".to_string(),
            _ => "String".to_string(),
        }
    } else {
        let mut chars: Vec<char> = vec![];
        let mut uppercase = true;
    
        let stripped_name = if autosar_name.starts_with("AR:") {
            autosar_name.split_at(3).1
        } else {
            autosar_name
        };
    
        for c in stripped_name.chars() {
            if c == '-' {
                uppercase = true;
            } else {
                if uppercase {
                    chars.push(c.to_ascii_uppercase());
                    uppercase = false;
                } else {
                    chars.push(c.to_ascii_lowercase());
                }
            }
        }
    
        chars.iter().collect()
    }
}


fn derive_var_name(autosar_name: &str) -> String {
    let mut chars: Vec<char> = vec![];
    let mut underscore = false;

    let stripped_name = if autosar_name.starts_with("AR:") {
        autosar_name.split_at(3).1
    } else {
        autosar_name
    };

    for c in stripped_name.chars() {
        if c == '-' {
            underscore = true;
        } else {
            if underscore {
                chars.push('_');
                underscore = false;
            }
            chars.push(c.to_ascii_lowercase());
        }
    }

    chars.iter().collect()
}


fn generate_element_type(autosar_types: &HashMap<String, DataType>, ec_item: &ElementCollectionItem) -> (String, ElementAmount) {
    match ec_item {
        ElementCollectionItem::Element(elem) => {
            (derive_type_name(autosar_types, &elem.typeref), elem.amount)
        }
        ElementCollectionItem::GroupRef(groupref) => {
            (derive_type_name(autosar_types, groupref), ElementAmount::One)
        }
    }
}


fn wrap_element_type(typestring: &str, amount: ElementAmount) -> String {
    match amount {
        ElementAmount::One => typestring.to_owned(),
        ElementAmount::ZeroOrOne => format!("Option<{typestring}>"),
        ElementAmount::Any => format!("Vec<{typestring}>"),
    }
}


fn generate_element_varname(ec_item: &ElementCollectionItem) -> String {
    match ec_item {
        ElementCollectionItem::Element(elem) => {
            derive_var_name(&elem.name)
        }
        ElementCollectionItem::GroupRef(groupref) => {
            derive_var_name(groupref)
        }
    }
}


fn generate_attribute_fields(autosar_types: &HashMap<String, DataType>, attributes: &Vec<Attribute>) -> Result<String, String> {
    let mut result = String::new();
    for attribute in attributes {
        let mut rust_type = match &attribute.attribute_type {
            AttributeType::Basic(typename) => {
                if (typename == "xsd:string") || (typename == "xsd:NMTOKEN") || (typename == "xsd:NMTOKENS") {
                    "String".to_owned()
                } else {
                    println!("attribute with type {}", typename);
                    todo!()
                }
            }
            AttributeType::Pattern { .. } => {
                "String".to_owned()
            }
            AttributeType::Enum(enumref) => {
                derive_type_name(autosar_types, enumref)
            }
        };
        let varname = derive_var_name(&attribute.name);
        if !attribute.required {
            rust_type = format!("Option<{rust_type}>");
        }
        writeln!(result, "    pub {varname}: {rust_type}, // attribute {}", attribute.name).unwrap();
    }
    Ok(result)
}