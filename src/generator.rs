use super::*;

pub(crate) fn generate(autosar_types: &HashMap<String, ElementContent>) -> Result<(), String> {
    genenerate_types(autosar_types)?;

    Ok(())
}


fn genenerate_types(autosar_types: &HashMap<String, ElementContent>) -> Result<(), String> {
    for (elemtype_name, elemtype) in autosar_types {
        match elemtype {
            ElementContent::Elements { .. } => {
                println!("Elements: {:40} -> {}", elemtype_name, derive_type_name(elemtype_name));
            }
            ElementContent::Characters { basetype, .. } => {
                println!("Characters: {:40} -> {} [[{}]]", elemtype_name, derive_type_name(elemtype_name), basetype);
            }
            ElementContent::Mixed { .. } => {
                println!("Mixed: {:40} -> {}", elemtype_name, derive_type_name(elemtype_name));
            }
            ElementContent::Enum(_) => {
                println!("Enum: {:40} -> {}", elemtype_name, derive_type_name(elemtype_name));
            }
        }
        
    }

    Ok(())
}

fn derive_type_name(autosar_name: &str) -> String {
    let mut chars: Vec<char> = vec!['A', 'r'];
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