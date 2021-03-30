use color_eyre::eyre;
use frame_metadata::{RuntimeMetadata, RuntimeMetadataPrefixed};
use proc_macro2::TokenStream;
use quote::quote;
use std::str::FromStr;

mod v12;

use v12::DecodeArrayOrElse;

pub fn decode_metadata(metadata: RuntimeMetadataPrefixed) -> color_eyre::Result<TokenStream> {
    let mut stream = quote! {
        #![allow(dead_code)]
        #![allow(unused_imports)]
    };

    match metadata.1 {
        RuntimeMetadata::V12(v12) => {
            let modules = v12.modules.decode_array_or_else()?;

            for module in modules {
                v12::add_module(module, &mut stream)?;
            }
        }
        _ => return Err(eyre::eyre!("Unsupported metadata version")),
    };

    Ok(stream)
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum RuntimeType {
    Primitive(String),
    Complex(String),
    Option(Box<RuntimeType>),
    Vec(Box<RuntimeType>),
    Tuple(Box<Vec<RuntimeType>>),
}

impl RuntimeType {
    fn flatten_complex(&self) -> Vec<String> {
        match self {
            Self::Primitive(_) => vec![],
            Self::Complex(ty) => vec![ty.to_string()],
            Self::Option(rt) => (**rt).flatten_complex(),
            Self::Vec(rt) => (**rt).flatten_complex(),
            Self::Tuple(rt) => (**rt).iter().flat_map(|rt| rt.flatten_complex()).collect(),
        }
    }
}

impl std::fmt::Display for RuntimeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Primitive(ty) => write!(f, "{}", ty),
            Self::Complex(ty) => write!(f, "T::{}", ty),
            Self::Option(rt) => write!(f, "Option<{}>", rt),
            Self::Vec(rt) => write!(f, "Vec<{}>", rt),
            Self::Tuple(rt) => write!(
                f,
                "({})",
                (**rt)
                    .iter()
                    .map(|rt| format!("{}", rt))
                    .collect::<Vec<String>>()
                    .join(",")
            ),
        }
    }
}

macro_rules! match_literal {
    ($string:expr, $start:expr, $stop:expr) => {
        $string.starts_with($start) && $string.contains($stop)
    };
}

macro_rules! inner_literal {
    ($line:expr, $start:expr, $stop:expr) => {{
        let start_bytes = $line.find($start).map(|i| i + $start.len()).unwrap_or(0);
        let stop_bytes = $line.rfind($stop).unwrap_or($line.len());
        &$line[start_bytes..stop_bytes]
    }};
}

macro_rules! outer_literal {
    ($line:expr, $start:expr, $stop:expr) => {{
        if let (Some(start), Some(stop)) = ($line.find($start), $line.rfind($stop)) {
            $line
                .chars()
                .take(start)
                .chain($line.chars().skip(stop + 1))
                .collect::<String>()
        } else {
            $line.to_string()
        }
    }};
}

impl FromStr for RuntimeType {
    type Err = color_eyre::Report;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().trim_start_matches("T::");

        let runtime_type = match s {
            "bool" | "usize" | "u8" | "u16" | "u32" | "u64" | "u128" | "isize" | "i8" | "i16"
            | "i32" | "i64" | "i128" => RuntimeType::Primitive(s.to_string()),
            _ if match_literal!(s, "Option<", ">") => RuntimeType::Option(Box::new(
                RuntimeType::from_str(inner_literal!(s, "Option<", ">"))?,
            )),
            _ if match_literal!(s, "Vec<", ">") => RuntimeType::Vec(Box::new(
                RuntimeType::from_str(inner_literal!(s, "Vec<", ">"))?,
            )),
            _ if match_literal!(s, "(", ")") => RuntimeType::Tuple(Box::new(
                inner_literal!(s, "(", ")")
                    .split(",")
                    .map(|ty| RuntimeType::from_str(ty))
                    .collect::<color_eyre::Result<Vec<_>>>()?,
            )),
            _ => RuntimeType::Complex(
                outer_literal!(s, "<", ">")
                    .trim_start_matches("::")
                    .to_string(),
            ),
        };
        Ok(runtime_type)
    }
}
