use super::DecodeOrElse;
use crate::RuntimeType;
use frame_metadata::v12;
use heck::{CamelCase, SnakeCase};
use proc_macro2::TokenStream;
use quote::{format_ident, quote, TokenStreamExt};
use std::str::FromStr;

pub fn add_storage_to_module(
    module_name: &str,
    storage_meta: v12::StorageMetadata,
    stream: &mut TokenStream,
) -> color_eyre::Result<Vec<RuntimeType>> {
    let _prefix = storage_meta.prefix.decode_or_else()?;
    let entries = storage_meta.entries.decode_or_else()?;

    let mut runtime_types = Vec::new();

    let mut parse_storage_ty = |ty: v12::DecodeDifferent<&str, String>| -> color_eyre::Result<_> {
        let ret_ty = RuntimeType::from_str(&ty.decode_or_else()?)?;
        runtime_types.push(ret_ty.clone());
        Ok(syn::parse_str::<syn::Type>(&format!("{}", ret_ty))?)
    };

    for entry in entries {
        let name = entry.name.decode_or_else()?;

        let store_ty = format_ident!(
            "{}",
            match entry.ty {
                v12::StorageEntryType::Plain(_) => "plain",
                v12::StorageEntryType::Map { .. } => "map",
                v12::StorageEntryType::DoubleMap { .. } => "double_map",
            }
        );

        let (key1_ty, key2_ty, ret_ty) = match entry.ty {
            v12::StorageEntryType::Plain(plain) => (None, None, plain),
            v12::StorageEntryType::Map { key, value, .. } => (Some(key), None, value),
            v12::StorageEntryType::DoubleMap {
                key1, key2, value, ..
            } => (Some(key1), Some(key2), value),
        };

        let ret_ty = parse_storage_ty(ret_ty)?;

        let key1_ty = if let Some(key1_ty) = key1_ty {
            let key1_ty = parse_storage_ty(key1_ty)?;
            Some(quote!(#key1_ty))
        } else {
            None
        };

        let key2_ty = if let Some(key2_ty) = key2_ty {
            let key2_ty = parse_storage_ty(key2_ty)?;
            Some(quote!(#key2_ty))
        } else {
            None
        };

        let (key_ty, key_arg, key_acc, key_our) = match (&key1_ty, &key2_ty) {
            (Some(kt1), Some(kt2)) => (
                Some(quote!(#kt1, #kt2,)),
                Some(quote!(key1: #kt1, key2: #kt2,)),
                Some(quote!(&self.1, &self.2)),
                Some(quote!(key1, key2)),
            ),
            (Some(kt), None) | (None, Some(kt)) => (
                Some(quote!(#kt,)),
                Some(quote!(key: #kt,)),
                Some(quote!(&self.1)),
                Some(quote!(key)),
            ),
            (None, None) => (None, None, None, None),
        };

        let fetch = quote!(fetch_or_default);

        let subxt = format_ident!("substrate_subxt");

        let module = format_ident!("{}", module_name);
        let store_name = name.clone();
        let store = format_ident!("{}", store_name.to_snake_case());
        let store_iter = format_ident!("{}_iter", store_name.to_snake_case());
        let store_struct = format_ident!("{}Store", store_name.to_camel_case());
        let store_trait = format_ident!("{}StoreExt", store_name);

        stream.append_all(quote! {
            #[derive(Encode, Decode)]
            pub struct #store_struct<T: #module> (
                core::marker::PhantomData<T>,
                #key_ty
            );

            impl<T: #module> #subxt::Store<T> for #store_struct<T> {
                const MODULE: &'static str = MODULE;
                const FIELD: &'static str = #store_name;

                type Returns = #ret_ty;

                fn prefix(
                    metadata: &#subxt::Metadata,
                ) -> Result<#subxt::sp_core::storage::StorageKey, #subxt::MetadataError> {
                    Ok(metadata
                        .module(Self::MODULE)?
                        .storage(Self::FIELD)?
                        .prefix())
                }

                fn key(
                    &self,
                    metadata: &#subxt::Metadata,
                ) -> Result<#subxt::sp_core::storage::StorageKey, #subxt::MetadataError> {
                    Ok(metadata
                        .module(Self::MODULE)?
                        .storage(Self::FIELD)?
                        .#store_ty()?
                        .key(#key_acc))
                }
            }

            /// Store extension trait.
            pub trait #store_trait<T: #module + #subxt::Runtime> {
                /// Retrieve the store element.
                fn #store<'a>(
                    &'a self,
                    #key_arg
                    hash: Option<T::Hash>,
                ) -> core::pin::Pin<Box<dyn core::future::Future<Output = Result<#ret_ty, #subxt::Error>> + Send + 'a>>;

                /// Iterate over the store element.
                fn #store_iter<'a>(
                    &'a self,
                    hash: Option<T::Hash>,
                ) -> core::pin::Pin<Box<dyn core::future::Future<Output = Result<#subxt::KeyIter<T, #store_struct<T>>, #subxt::Error>> + Send + 'a>>;
            }

            impl<T: #module + #subxt::Runtime> #store_trait<T> for #subxt::Client<T> {
                fn #store<'a>(
                    &'a self,
                    #key_arg
                    hash: Option<T::Hash>,
                ) -> core::pin::Pin<Box<dyn core::future::Future<Output = Result<#ret_ty, #subxt::Error>> + Send + 'a>> {
                    let _marker = core::marker::PhantomData::<T>;
                    Box::pin(async move { self.#fetch(&#store_struct(_marker, #key_our), hash).await })
                }

                fn #store_iter<'a>(
                    &'a self,
                    hash: Option<T::Hash>,
                ) -> core::pin::Pin<Box<dyn core::future::Future<Output = Result<#subxt::KeyIter<T, #store_struct<T>>, #subxt::Error>> + Send + 'a>> {
                    Box::pin(self.iter(hash))
                }
            }

        });
    }

    Ok(runtime_types)
}
