use inflections::Inflect;
use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, quote_spanned};
use serde::Deserialize;
use serde_tokenstream::from_tokenstream;
use syn::spanned::Spanned;
use syn::PathArguments::AngleBracketed;
use syn::{
    AngleBracketedGenericArguments, FnArg, GenericArgument, PatType, ReturnType, Signature,
    TraitItem, Type, TypePath,
};

#[derive(Deserialize)]
struct ManyModuleAttributes {
    pub id: Option<u32>,
    pub name: Option<String>,
    pub namespace: Option<String>,
    pub many_crate: Option<String>,
    pub drop_non_webauthn: Option<Vec<String>>,
}

#[derive(Debug)]
struct Endpoint {
    pub name: String,
    pub func: Ident,
    pub span: Span,
    pub is_async: bool,
    pub is_mut: bool,
    pub has_sender: bool,
    pub arg_type: Option<Box<Type>>,
    #[allow(unused)]
    pub ret_type: Box<Type>,
}

impl Endpoint {
    pub fn new(signature: &Signature) -> Result<Self, (String, Span)> {
        let func = signature.ident.clone();
        let name = func.to_string();
        let is_async = signature.asyncness.is_some();

        let mut has_sender = false;
        let arg_type: Option<Box<Type>>;
        let mut ret_type: Option<Box<Type>> = None;

        let mut inputs = signature.inputs.iter();
        let receiver = inputs.next().ok_or_else(|| {
            (
                "Must have at least 1 argument".to_string(),
                signature.span(),
            )
        })?;
        let is_mut = if let FnArg::Receiver(r) = receiver {
            r.mutability.is_some()
        } else {
            return Err((
                "Function in trait must have a receiver".to_string(),
                receiver.span(),
            ));
        };

        let maybe_identity = inputs.next();
        let maybe_argument = inputs.next();

        match (maybe_identity, maybe_argument) {
            (_id, Some(FnArg::Typed(PatType { ty, .. }))) => {
                has_sender = true;
                arg_type = Some(ty.clone());
            }
            (Some(FnArg::Typed(PatType { ty, .. })), None) => {
                arg_type = Some(ty.clone());
            }
            (None, None) => {
                arg_type = None;
            }
            (_, _) => {
                return Err(("Must have 2 or 3 arguments".to_string(), signature.span()));
            }
        }

        if let ReturnType::Type(_, ty) = &signature.output {
            if let Type::Path(TypePath {
                path: syn::Path { segments, .. },
                ..
            }) = ty.as_ref()
            {
                if segments[0].ident == "Result"
                    || segments
                        .iter()
                        .map(|x| x.ident.to_string())
                        .collect::<Vec<String>>()
                        .join("::")
                        == "std::result::Result"
                {
                    if let AngleBracketed(AngleBracketedGenericArguments { ref args, .. }) =
                        segments[0].arguments
                    {
                        ret_type = Some(
                            args.iter()
                                .find_map(|x| match x {
                                    GenericArgument::Type(t) => Some(Box::new(t.clone())),
                                    _ => None,
                                })
                                .unwrap(),
                        );
                    }
                }
            }
        }

        if ret_type.is_none() {
            return Err((
                "Must have a result return type.".to_string(),
                signature.output.span(),
            ));
        }

        Ok(Self {
            name,
            func,
            span: signature.span(),
            is_async,
            is_mut,
            has_sender,
            arg_type,
            ret_type: ret_type.unwrap(),
        })
    }
}

#[allow(clippy::too_many_lines)]
fn many_module_impl(attr: &TokenStream, item: TokenStream) -> Result<TokenStream, syn::Error> {
    let attrs: ManyModuleAttributes = from_tokenstream(attr)?;
    let many = Ident::new(
        attrs.many_crate.as_ref().map_or("many", String::as_str),
        attr.span(),
    );

    let namespace = attrs.namespace;
    let span = item.span();
    let tr: syn::ItemTrait = syn::parse2(item)
        .map_err(|_| syn::Error::new(span, "`many_module` only applies to traits.".to_string()))?;

    let struct_name = attrs.name.clone().unwrap_or_else(|| tr.ident.to_string());
    let struct_ident = Ident::new(
        struct_name.as_str(),
        attrs
            .name
            .as_ref()
            .map_or_else(|| attr.span(), |_| tr.ident.span()),
    );

    let mut trait_ = tr.clone();

    if attrs.name.is_none() {
        trait_.ident = Ident::new(&format!("{}Backend", struct_name), tr.ident.span());
    }
    let trait_ident = trait_.ident.clone();

    let vis = trait_.vis.clone();

    let attr_id = attrs.id.iter();
    let attr_name =
        inflections::Inflect::to_constant_case(format!("{}Attribute", struct_name).as_str());
    let attr_ident = Ident::new(&attr_name, attr.span());

    let info_name = format!("{}Info", struct_name);
    let info_ident = Ident::new(&info_name, attr.span());

    let endpoints: Result<Vec<_>, (String, Span)> = trait_
        .items
        .iter()
        .filter_map(|item| match item {
            TraitItem::Method(m) => Some(m),
            _ => None,
        })
        .map(|item| Endpoint::new(&item.sig))
        .collect();
    let endpoints = endpoints.map_err(|(msg, span)| syn::Error::new(span, msg))?;
    let ns = namespace.clone();
    let endpoint_strings: Vec<String> = endpoints
        .iter()
        .map(move |e| {
            let name = e.name.as_str().to_camel_case();
            match ns {
                Some(ref namespace) => format!("{}.{}", namespace, name),
                None => name,
            }
        })
        .collect();

    let ns = namespace.clone();
    let validate_endpoint_pat = endpoints.iter().map(|e| {
        let span = e.span;
        let name = e.name.as_str().to_camel_case();
        let ep = match ns {
            Some(ref namespace) => format!("{}.{}", namespace, name),
            None => name,
        };

        if let Some(ty) = &e.arg_type {
            quote_spanned! { span =>
                #ep => {
                    minicbor::decode::<'_, #ty>(data)
                        .map_err(|e| ManyError::deserialization_error(e.to_string()))?;
                }
            }
        } else {
            quote! {
                #ep => {}
            }
        }
    });
    let validate = quote! {
        fn validate(&self, message: & #many ::message::RequestMessage) -> Result<(),  #many ::ManyError> {
            let method = message.method.as_str();
            let data = message.data.as_slice();
            match method {
                #(#validate_endpoint_pat)*

                _ => return Err( #many ::ManyError::invalid_method_name(method.to_string())),
            };
            Ok(())
        }
    };

    let ns = namespace.clone();
    let validate_envelope = if let Some(endpoint) = attrs.drop_non_webauthn {
        let field_names = endpoint.iter().map(|e| match &ns {
            Some(namespace) => format!("{}.{}", namespace, e),
            None => e.to_string(),
        });
        // Note: The endpoint needs to be the endpoind method name, not the trait method
        // Ex: getFromAddress and NOT get_from_address
        field_names
            .clone()
            .any(|name| endpoint_strings.contains(&name))
            .then(|| 0)
            .ok_or_else(|| {
                syn::Error::new(span, "`drop_non_webauthn` endpoint non found in trait.")
            })?;
        quote! {
            fn validate_envelope(&self, envelope: &coset::CoseSign1, message: & #many ::message::RequestMessage) -> Result<(), #many ::ManyError> {
                let method = message.method.as_str();
                if vec![#(#field_names),*].contains(&method) {
                    let unprotected =
                        std::collections::BTreeMap::from_iter(envelope.unprotected.rest.clone().into_iter());
                    if !unprotected.contains_key(&coset::Label::Text("webauthn".to_string())) {
                        return Err( #many ::ManyError::non_webauthn_request_denied(&method))
                    }
                }
                Ok(())
            }
        }
    } else {
        quote! {
            fn validate_envelope(&self, envelope: &coset::CoseSign1, message: & #many ::message::RequestMessage) -> Result<(), #many ::ManyError> {
                Ok(())
            }
        }
    };

    let ns = namespace;
    let execute_endpoint_pat = endpoints.iter().map(|e| {
        let span = e.span;
        let name = e.name.as_str().to_camel_case();
        let ep = match ns {
            Some(ref namespace) => format!("{}.{}", namespace, name),
            None => name,
        };
        let ep_ident = &e.func;

        let backend_decl = if e.is_mut {
            quote! { let mut backend = self.backend.lock().unwrap(); }
        } else {
            quote! { let backend = self.backend.lock().unwrap(); }
        };

        let call = match (e.has_sender, e.arg_type.is_some(), e.is_async) {
            (false, true, false) => quote_spanned! { span => encode( backend . #ep_ident ( decode( data )? ) ) },
            (false, true, true) => quote_spanned! { span => encode( backend . #ep_ident ( decode( data )? ).await ) },
            (true, true, false) => quote_spanned! { span => encode( backend . #ep_ident ( &message.from.unwrap_or_default(), decode( data )? ) ) },
            (true, true, true) => quote_spanned! { span => encode( backend . #ep_ident ( &message.from.unwrap_or_default(), decode( data )? ).await ) },
            (false, false, false) => quote_spanned! { span => encode( backend . #ep_ident ( ) ) },
            (false, false, true) => quote_spanned! { span => encode( backend . #ep_ident ( ).await ) },
            (true, false, false) => quote_spanned! { span => encode( backend . #ep_ident ( &message.from.unwrap_or_default() ) ) },
            (true, false, true) => quote_spanned! { span => encode( backend . #ep_ident ( &message.from.unwrap_or_default() ).await ) },
        };

        quote_spanned! { span =>
            #ep => {
                #backend_decl
                #call
            }
        }
    });
    let execute = quote! {
        async fn execute(
            &self,
            message:  #many ::message::RequestMessage,
        ) -> Result< #many ::message::ResponseMessage,  #many ::ManyError> {
            use  #many ::ManyError;
            fn decode<'a, T: minicbor::Decode<'a>>(data: &'a [u8]) -> Result<T, ManyError> {
                minicbor::decode(data).map_err(|e| ManyError::deserialization_error(e.to_string()))
            }
            fn encode<T: minicbor::Encode>(result: Result<T, ManyError>) -> Result<Vec<u8>, ManyError> {
                minicbor::to_vec(result?).map_err(|e| ManyError::serialization_error(e.to_string()))
            }

            let data = message.data.as_slice();
            let result = match message.method.as_str() {
                #( #execute_endpoint_pat )*

                _ => Err(ManyError::internal_server_error()),
            }?;

            Ok( #many ::message::ResponseMessage::from_request(
                &message,
                &message.to,
                Ok(result),
            ))
        }
    };

    let attribute = if attrs.id.is_some() {
        quote! { Some(#attr_ident) }
    } else {
        quote! { None }
    };

    Ok(quote! {
        #( #vis const #attr_ident:  #many ::protocol::Attribute =  #many ::protocol::Attribute::id(#attr_id); )*

        #vis struct #info_ident;
        impl std::ops::Deref for #info_ident {
            type Target =  #many ::server::module::ManyModuleInfo;

            fn deref(&self) -> & #many ::server::module::ManyModuleInfo {
                use  #many ::server::module::ManyModuleInfo;
                static ONCE: std::sync::Once = std::sync::Once::new();
                static mut VALUE: *mut ManyModuleInfo = 0 as *mut ManyModuleInfo;

                unsafe {
                    ONCE.call_once(|| VALUE = Box::into_raw(Box::new(ManyModuleInfo {
                        name: #struct_name .to_string(),
                        attribute: #attribute,
                        endpoints: vec![ #( #endpoint_strings .to_string() ),* ],
                    })));
                    &*VALUE
                }
            }
        }

        #[async_trait::async_trait]
        #trait_

        #vis struct #struct_ident<T: #trait_ident> {
            backend: std::sync::Arc<std::sync::Mutex<T>>
        }

        impl<T: #trait_ident> std::fmt::Debug for #struct_ident<T> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(#struct_name).finish()
            }
        }

        impl<T: #trait_ident> #struct_ident<T> {
            pub fn new(backend: std::sync::Arc<std::sync::Mutex<T>>) -> Self {
                Self { backend }
            }
        }

        #[async_trait::async_trait]
        impl<T: #trait_ident>  #many ::ManyModule for #struct_ident<T> {
            fn info(&self) -> & #many ::server::module::ManyModuleInfo {
                & #info_ident
            }

            #validate_envelope

            #validate

            #execute
        }
    })
}

#[proc_macro_attribute]
pub fn many_module(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    many_module_impl(&attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}
