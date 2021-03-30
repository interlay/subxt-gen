use color_eyre::eyre;
use frame_metadata::v12;
use heck::{CamelCase, SnakeCase};
use proc_macro2::TokenStream;
use quote::{format_ident, quote, TokenStreamExt};
use std::collections::HashSet;

mod call;
mod event;
mod store;

use call::add_call_to_module;
use event::add_event_to_module;
use store::add_storage_to_module;

pub fn add_module(module: v12::ModuleMetadata, stream: &mut TokenStream) -> color_eyre::Result<()> {
    let module_name = module.name.decode_or_else()?.to_camel_case();

    if module_name == "System" {
        return Ok(());
    }

    let mut runtime_types = Vec::new();
    let mut module_stream = TokenStream::new();

    if let Some(calls) = module.calls {
        let calls = calls.decode_or_else()?;
        for call in calls {
            runtime_types.extend(add_call_to_module(&module_name, call, &mut module_stream)?);
        }
    }

    if let Some(events) = module.event {
        let events = events.decode_or_else()?;
        for event in events {
            runtime_types.extend(add_event_to_module(
                &module_name,
                event,
                &mut module_stream,
            )?);
        }
    }

    if let Some(storages) = module.storage {
        let storage = storages.decode_or_else()?;
        runtime_types.extend(add_storage_to_module(
            &module_name,
            storage,
            &mut module_stream,
        )?);
    }

    let mut runtime_types = runtime_types
        .into_iter()
        .flat_map(|rt| rt.flatten_complex().into_iter())
        .collect::<HashSet<_>>();

    runtime_types.remove("BlockNumber");
    runtime_types.remove("AccountId");
    runtime_types.remove("AccountData");
    runtime_types.remove("Hash");

    let assoc_tys = runtime_types
        .iter()
        .map(|rt| format_ident!("{}", rt))
        .map(|rt| quote!(type #rt: Codec + EncodeLike + Member + Default + Send + Sync;));

    let module_ident = format_ident!("{}", module_name.to_snake_case());
    let module_trait = format_ident!("{}", module_name.to_camel_case());

    let module_stream = quote! {
        pub mod #module_ident {
            use parity_scale_codec::{Codec, EncodeLike, Encode, Decode};
            use sp_runtime::traits::Member;

            const MODULE: &str = #module_name;

            pub trait #module_trait: substrate_subxt::system::System {
                #(#assoc_tys)*
            }

            #module_stream
        }
    };

    stream.append_all(module_stream);

    Ok(())
}

pub trait DecodeArrayOrElse<T> {
    fn decode_array_or_else(self) -> color_eyre::Result<Vec<T>>;
}

impl<T> DecodeArrayOrElse<T> for v12::DecodeDifferentArray<T> {
    fn decode_array_or_else(self) -> color_eyre::Result<Vec<T>> {
        match self {
            v12::DecodeDifferentArray::Decoded(value) => Ok(value),
            v12::DecodeDifferentArray::Encode(_) => Err(eyre::eyre!("Metadata should be Decoded")),
        }
    }
}

pub trait DecodeOrElse<T> {
    fn decode_or_else(self) -> color_eyre::Result<T>;
}

impl<B, O> DecodeOrElse<O> for v12::DecodeDifferent<B, O> {
    fn decode_or_else(self) -> color_eyre::Result<O> {
        match self {
            v12::DecodeDifferent::Decoded(value) => Ok(value),
            v12::DecodeDifferent::Encode(_) => Err(eyre::eyre!("Metadata should be Decoded")),
        }
    }
}
