use super::DecodeOrElse;
use crate::RuntimeType;
use frame_metadata::v12;
use heck::{CamelCase, SnakeCase};
use proc_macro2::TokenStream;
use quote::{format_ident, quote, TokenStreamExt};
use std::str::FromStr;

pub fn add_event_to_module(
    module_name: &str,
    event_meta: v12::EventMetadata,
    stream: &mut TokenStream,
) -> color_eyre::Result<Vec<RuntimeType>> {
    let name = event_meta.name.decode_or_else()?;
    let arguments = event_meta.arguments.decode_or_else()?;
    let arguments = arguments
        .into_iter()
        .map(|arg| RuntimeType::from_str(&arg))
        .collect::<color_eyre::Result<Vec<_>>>()?;

    let subxt = format_ident!("substrate_subxt");
    let codec = format_ident!("parity_scale_codec");

    let module = format_ident!("{}", module_name);
    let event_name = name.clone();
    let event = format_ident!("{}", event_name.to_snake_case());
    let event_struct = format_ident!("{}Event", event_name.to_camel_case());
    let event_trait = format_ident!("{}EventExt", event_name);

    let event_fields = arguments
        .clone()
        .into_iter()
        .map(|ty| syn::parse_str::<syn::Type>(&format!("{}", ty)).unwrap())
        .collect::<Vec<_>>();

    stream.append_all(quote! {
        #[derive(Decode)]
        pub struct #event_struct<T: #module> (
            pub core::marker::PhantomData<T>,
            #(pub #event_fields,)*
        );

        impl<T: #module> #subxt::Event<T> for #event_struct<T> {
            const MODULE: &'static str = MODULE;
            const EVENT: &'static str = #event_name;
        }

        /// Event extension trait.
        pub trait #event_trait<T: #module> {
            /// Retrieves the event.
            fn #event(&self) -> Result<Option<#event_struct<T>>, #codec::Error>;
        }

        impl<T: #module> #event_trait<T> for #subxt::ExtrinsicSuccess<T> {
            fn #event(&self) -> Result<Option<#event_struct<T>>, #codec::Error> {
                self.find_event()
            }
        }
    });

    Ok(arguments)
}
