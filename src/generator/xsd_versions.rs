use crate::XsdFileInfo;
use std::fmt::Write;
use std::fs::File;
use std::io::Write as IoWrite;

pub(crate) fn generate(xsd_config: &[XsdFileInfo]) {
    let mut match_lines = String::new();
    let mut filename_lines = String::new();
    let mut desc_lines = String::new();
    let mut generated = String::from(
        r"use num_derive::FromPrimitive;
use num_traits::cast::FromPrimitive;

#[derive(Debug)]
/// Error type returned when `from_str()` / `parse()` for `AutosarVersion` fails
pub struct ParseAutosarVersionError;

#[allow(non_camel_case_types)]
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Hash, FromPrimitive)]
#[repr(u32)]
#[non_exhaustive]
/// Enum of all Autosar versions
pub enum AutosarVersion {
",
    );

    for (idx, xsd_file_info) in xsd_config.iter().enumerate() {
        writeln!(
            generated,
            r#"    /// {} - xsd file name: `{}`"#,
            xsd_file_info.desc, xsd_file_info.name
        )
        .unwrap();
        writeln!(
            generated,
            r#"    {} = 0x{:x},"#,
            xsd_file_info.ident,
            1 << idx
        )
        .unwrap();
        writeln!(
            match_lines,
            r#"            "{}" => Ok(Self::{}),"#,
            xsd_file_info.name, xsd_file_info.ident
        )
        .unwrap();
        writeln!(
            filename_lines,
            r#"            Self::{} => "{}","#,
            xsd_file_info.ident, xsd_file_info.name
        )
        .unwrap();
        writeln!(
            desc_lines,
            r#"            Self::{} => "{}","#,
            xsd_file_info.ident, xsd_file_info.desc
        )
        .unwrap();
    }
    let lastident = xsd_config[xsd_config.len() - 1].ident;
    writeln!(
        generated,
        r#"}}

impl AutosarVersion {{
    /// get the name of the xsd file matching the Autosar version
    #[must_use]
    pub fn filename(&self) -> &'static str {{
        match self {{
{filename_lines}
        }}
    }}

    /// Human readable description of the Autosar version
    ///
    /// This is particularly useful for the later versions, where the xsd files are just sequentially numbered.
    /// For example `Autosar_00050` -> "AUTOSAR R21-11"
    #[must_use]
    pub fn describe(&self) -> &'static str {{
        match self {{
{desc_lines}
        }}
    }}

    /// make an `AutosarVersion` from a u32 value
    ///
    /// All `AutosarVersion`s are associated with a power of two u32 value, for example `Autosar_4_3_0` == 0x100
    /// If the given value is a valid constant of `AutosarVersion`, the enum value will be returnd
    ///
    /// This is useful in order to decode version masks
    #[must_use]
    pub fn from_val(n: u32) -> Option<Self> {{
        Self::from_u32(n)
    }}

    /// `AutosarVersion::LATEST` is an alias of which ever is the latest version
    pub const LATEST: AutosarVersion = AutosarVersion::{lastident};
}}

impl std::str::FromStr for AutosarVersion {{
    type Err = ParseAutosarVersionError;
    fn from_str(input: &str) -> Result<Self, Self::Err> {{
        match input {{
{match_lines}
            _ => Err(ParseAutosarVersionError),
        }}
    }}
}}

impl std::fmt::Display for AutosarVersion {{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{
        f.write_str(self.describe())
    }}
}}
"#,
    )
    .unwrap();

    let mut file = File::create("gen/autosarversion.rs").unwrap();
    file.write_all(generated.as_bytes()).unwrap();
}
