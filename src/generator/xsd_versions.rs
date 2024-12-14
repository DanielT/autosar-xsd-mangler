use crate::XsdFileInfo;
use std::fs::File;
use std::io::Write as IoWrite;

pub(crate) fn generate(xsd_config: &[XsdFileInfo]) {
    let mut match_lines = Vec::new();
    let mut filename_lines = Vec::new();
    let mut desc_lines = Vec::new();
    let mut from_lines = Vec::new();
    let mut generated = String::from(
        r"use num_traits::cast::FromPrimitive;

#[derive(Debug)]
/// Error type returned when `from_str()` / `parse()` for `AutosarVersion` fails
pub struct ParseAutosarVersionError;

#[allow(non_camel_case_types)]
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Hash)]
#[repr(u32)]
#[non_exhaustive]
/// Enum of all Autosar versions
pub enum AutosarVersion {
",
    );

    for (idx, xsd_file_info) in xsd_config.iter().enumerate() {
        // generate the content of enum AutosarVersion directly
        generated.push_str(&format!(
            "    /// {} - xsd file name: `{}`\n    {} = 0x{:x},\n",
            xsd_file_info.desc,
            xsd_file_info.name,
            xsd_file_info.ident,
            1 << idx
        ));

        // generate the match arms for the `from_str()` method
        match_lines.push(format!(
            r#"            "{}" => Ok(Self::{})"#,
            xsd_file_info.name, xsd_file_info.ident
        ));
        // generate the match arms for the `filename()` method
        filename_lines.push(format!(
            r#"            Self::{} => "{}""#,
            xsd_file_info.ident, xsd_file_info.name
        ));
        // generate the match arms for the `describe()` method
        desc_lines.push(format!(
            r#"            Self::{} => "{}""#,
            xsd_file_info.ident, xsd_file_info.desc
        ));
        // generate the match arms for the `from_u32()` method
        from_lines.push(format!(
            r#"            0x{:x} => Some(Self::{})"#,
            1 << idx,
            xsd_file_info.ident
        ));
    }
    let match_lines = match_lines.join(",\n");
    let filename_lines = filename_lines.join(",\n");
    let desc_lines = desc_lines.join(",\n");
    let from_lines = from_lines.join(",\n");

    let lastident = xsd_config[xsd_config.len() - 1].ident;
    generated.push_str(&format!(
        r#"}}

impl AutosarVersion {{
    /// get the name of the xsd file matching the Autosar version
    #[must_use]
    pub fn filename(&self) -> &'static str {{
        match self {{
{filename_lines},
        }}
    }}

    /// Human readable description of the Autosar version
    ///
    /// This is particularly useful for the later versions, where the xsd files are just sequentially numbered.
    /// For example `Autosar_00050` -> "AUTOSAR R21-11"
    #[must_use]
    pub fn describe(&self) -> &'static str {{
        match self {{
{desc_lines},
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

impl core::str::FromStr for AutosarVersion {{
    type Err = ParseAutosarVersionError;
    fn from_str(input: &str) -> Result<Self, Self::Err> {{
        match input {{
{match_lines},
            _ => Err(ParseAutosarVersionError),
        }}
    }}
}}

impl core::fmt::Display for AutosarVersion {{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {{
        f.write_str(self.describe())
    }}
}}

impl FromPrimitive for AutosarVersion {{
    #[inline]
    fn from_i64(n: i64) -> Option<Self> {{
        if n < 0 {{
            return None;
        }}
        Self::from_u64(n as u64)
    }}

    #[inline]
    fn from_u64(n: u64) -> Option<Self> {{
        match n {{
{from_lines},
            _ => None,
        }}
    }}
}}
"#,
    ));

    let mut file = File::create("gen/autosarversion.rs").unwrap();
    file.write_all(generated.as_bytes()).unwrap();
}
