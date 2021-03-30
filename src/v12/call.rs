use super::DecodeOrElse;
use crate::RuntimeType;
use frame_metadata::v12;
use heck::CamelCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote, TokenStreamExt};
use std::convert::TryInto;
use std::str::FromStr;

#[derive(Clone)]
struct FunctionArgument {
    name: String,
    ty: RuntimeType,
}

impl TryInto<FunctionArgument> for v12::FunctionArgumentMetadata {
    type Error = color_eyre::Report;
    fn try_into(self) -> Result<FunctionArgument, Self::Error> {
        let ty = self.ty.clone().decode_or_else()?;
        Ok(FunctionArgument {
            name: self.name.decode_or_else()?,
            ty: RuntimeType::from_str(ty.as_ref())?,
        })
    }
}

pub fn add_call_to_module(
    module_name: &str,
    fn_meta: v12::FunctionMetadata,
    stream: &mut TokenStream,
) -> color_eyre::Result<Vec<RuntimeType>> {
    let name = fn_meta.name.decode_or_else()?;
    let arguments = fn_meta
        .arguments
        .decode_or_else()?
        .into_iter()
        .map(TryInto::try_into)
        .collect::<color_eyre::Result<Vec<FunctionArgument>>>()?;

    let subxt = format_ident!("substrate_subxt");

    let module = format_ident!("{}", module_name);
    let call_name = name.clone();
    let call_struct = format_ident!("{}Call", call_name.to_camel_case());
    let call_trait = format_ident!("{}CallExt", call_name.to_camel_case());
    let call = format_ident!("{}", call_name);
    let call_and_watch = format_ident!("{}_and_watch", call_name);

    let fields = arguments
        .clone()
        .into_iter()
        .map(|FunctionArgument { name, ty }| {
            (
                format_ident!("{}", name),
                syn::parse_str::<syn::Type>(&format!("{}", ty)).unwrap(),
            )
        })
        .collect::<Vec<_>>();

    let call_args = fields.iter().map(|(field, ty)| quote!(#field: #ty,));
    let call_args = quote!(#(#call_args)*);

    let call_fields = fields
        .iter()
        .map(|(name, ty)| quote! { #name: #ty })
        .collect::<Vec<_>>();

    let call_init = fields.iter().map(|(field, _)| quote!(#field,));
    let call_init = quote!(#(#call_init)*);

    stream.append_all(quote! {
        #[derive(Encode)]
        pub struct #call_struct<T: #module + #subxt::Runtime> {
            pub _marker: core::marker::PhantomData<T>,
            #(pub #call_fields,)*
        }

        impl<T: #module + #subxt::Runtime> #subxt::Call<T> for #call_struct<T> {
            const MODULE: &'static str = MODULE;
            const FUNCTION: &'static str = #call_name;
        }

        pub trait #call_trait<T: #module + #subxt::Runtime> {
            /// Create and submit an extrinsic.
            fn #call<'a>(
                &'a self,
                signer: &'a (dyn #subxt::Signer<T> + Send + Sync),
                #call_args
            ) -> core::pin::Pin<Box<dyn core::future::Future<Output = Result<T::Hash, #subxt::Error>> + Send + 'a>>;

            /// Create, submit and watch an extrinsic.
            fn #call_and_watch<'a>(
                &'a self,
                signer: &'a (dyn #subxt::Signer<T> + Send + Sync),
                #call_args
            ) -> core::pin::Pin<Box<dyn core::future::Future<Output = Result<#subxt::ExtrinsicSuccess<T>, #subxt::Error>> + Send + 'a>>;
        }

        impl<T: #module + #subxt::Runtime> #call_trait<T> for #subxt::Client<T>
        where
            <<T::Extra as #subxt::SignedExtra<T>>::Extra as #subxt::SignedExtension>::AdditionalSigned: Send + Sync,
        {
            fn #call<'a>(
                &'a self,
                signer: &'a (dyn #subxt::Signer<T> + Send + Sync),
                #call_args
            ) -> core::pin::Pin<Box<dyn core::future::Future<Output = Result<T::Hash, #subxt::Error>> + Send + 'a>> {
                let _marker = core::marker::PhantomData::<T>;
                Box::pin(self.submit(#call_struct { _marker, #call_init }, signer))
            }

            fn #call_and_watch<'a>(
                &'a self,
                signer: &'a (dyn #subxt::Signer<T> + Send + Sync),
                #call_args
            ) -> core::pin::Pin<Box<dyn core::future::Future<Output = Result<#subxt::ExtrinsicSuccess<T>, #subxt::Error>> + Send + 'a>> {
                let _marker = core::marker::PhantomData::<T>;
                Box::pin(self.watch(#call_struct { _marker, #call_init }, signer))
            }
        }

    });

    Ok(arguments.into_iter().map(|arg| arg.ty).collect())
}
