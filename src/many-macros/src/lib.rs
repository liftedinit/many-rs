use inflections::Inflect;
use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, quote_spanned};
use serde::Deserialize;
use serde_tokenstream::from_tokenstream;
use syn::parse::ParseStream;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{FnArg, Pat, PatType, ReturnType, Token, TraitItem, TraitItemMethod, Type, TypePath};

#[derive(Deserialize)]
struct ManyModuleAttributes {
    pub id: Option<u32>,
    pub name: Option<String>,
    pub namespace: Option<String>,
    pub many_modules_crate: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct EndpointManyAttribute {
    deny_anonymous: Option<bool>,
    check_webauthn: Option<bool>,
}

impl EndpointManyAttribute {
    pub fn deny_anonymous(&self) -> bool {
        self.deny_anonymous == Some(true)
    }

    pub fn check_webauthn(&self) -> bool {
        self.check_webauthn == Some(true)
    }

    pub fn merge(self, other: Self) -> syn::Result<Self> {
        fn either<T: quote::ToTokens>(a: Option<T>, b: Option<T>) -> syn::Result<Option<T>> {
            match (a, b) {
                (None, None) => Ok(None),
                (Some(val), None) | (None, Some(val)) => Ok(Some(val)),
                (Some(a), Some(b)) => {
                    let mut error = syn::Error::new_spanned(a, "redundant attribute argument");
                    error.combine(syn::Error::new_spanned(b, "note: first one here"));
                    Err(error)
                }
            }
        }

        Ok(Self {
            deny_anonymous: either(self.deny_anonymous, other.deny_anonymous)?,
            check_webauthn: either(self.check_webauthn, other.check_webauthn)?,
        })
    }
}

impl syn::parse::Parse for EndpointManyAttribute {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let arg_name: Ident = input.parse()?;

        if arg_name == "deny_anonymous" {
            Ok(Self {
                deny_anonymous: Some(true),
                check_webauthn: None,
            })
        } else if arg_name == "check_webauthn" {
            Ok(Self {
                deny_anonymous: None,
                check_webauthn: Some(true),
            })
        } else {
            Err(syn::Error::new_spanned(arg_name, "unsupported attribute"))
        }
    }
}

#[derive(Debug)]
struct Endpoint {
    pub attributes: Vec<syn::Attribute>,
    pub metadata: EndpointManyAttribute,
    pub name: String,
    pub func: Ident,
    pub span: Span,
    pub is_async: bool,
    pub is_mut: bool,
    pub sender: Option<(Box<Pat>, Box<Type>)>,
    pub arg: Option<(Box<Pat>, Box<Type>)>,
    pub context: Option<(Box<Pat>, Box<Type>)>,
    pub ret_type: Box<Type>,
    pub block: Option<syn::Block>,
}

impl Endpoint {
    pub fn new(item: &TraitItemMethod) -> syn::Result<Self> {
        let signature = &item.sig;

        let func = signature.ident.clone();
        let name = func.to_string();
        let is_async = signature.asyncness.is_some();

        let sender: Option<(Box<Pat>, Box<Type>)>;
        let context: Option<(Box<Pat>, Box<Type>)>;
        let arg: Option<(Box<Pat>, Box<Type>)>;
        let mut ret_type: Option<Box<Type>> = None;

        let mut inputs = signature.inputs.iter();
        let receiver = inputs.next().ok_or_else(|| {
            syn::Error::new(
                signature.span(),
                "Must have at least 1 argument".to_string(),
            )
        })?;
        let is_mut = if let FnArg::Receiver(r) = receiver {
            r.mutability.is_some()
        } else {
            return Err(syn::Error::new(
                receiver.span(),
                "Function in trait must have a receiver".to_string(),
            ));
        };

        let maybe_identity = inputs.next();
        let maybe_argument = inputs.next();
        let maybe_context = inputs.next();

        match (maybe_identity, maybe_argument, maybe_context) {
            (
                Some(FnArg::Typed(PatType {
                    ty: id_ty,
                    pat: id_pat,
                    ..
                })),
                Some(FnArg::Typed(PatType { ty, pat, .. })),
                Some(FnArg::Typed(PatType {
                    ty: ctx_ty,
                    pat: ctx_pat,
                    ..
                })),
            ) => {
                sender = Some((id_pat.clone(), id_ty.clone()));
                arg = Some((pat.clone(), ty.clone()));
                context = Some((ctx_pat.clone(), ctx_ty.clone()));
            }
            (
                Some(FnArg::Typed(PatType {
                    ty: id_ty,
                    pat: id_pat,
                    ..
                })),
                Some(FnArg::Typed(PatType { ty, pat, .. })),
                None,
            ) => {
                sender = Some((id_pat.clone(), id_ty.clone()));
                arg = Some((pat.clone(), ty.clone()));
                context = None;
            }
            (Some(FnArg::Typed(PatType { ty, pat, .. })), None, None) => {
                sender = None;
                arg = Some((pat.clone(), ty.clone()));
                context = None;
            }
            (None, None, None) => {
                sender = None;
                arg = None;
                context = None;
            }
            (_, _, _) => {
                return Err(syn::Error::new(
                    signature.span(),
                    "Must have 2, 3, or 4 arguments".to_string(),
                ));
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
                    ret_type = Some(ty.clone());
                }
            }
        }

        if ret_type.is_none() {
            return Err(syn::Error::new(
                signature.output.span(),
                "Must have a result return type.".to_string(),
            ));
        }

        let (meta_attrs, attributes): (Vec<syn::Attribute>, Vec<syn::Attribute>) = item
            .attrs
            .clone()
            .into_iter()
            .partition(|attr| attr.path.is_ident("many"));

        let metadata =
            meta_attrs
                .into_iter()
                .try_fold(EndpointManyAttribute::default(), |meta, attr| {
                    let list: Punctuated<EndpointManyAttribute, Token![,]> =
                        attr.parse_args_with(Punctuated::parse_terminated)?;

                    list.into_iter()
                        .try_fold(meta, EndpointManyAttribute::merge)
                })?;

        Ok(Self {
            metadata,
            attributes,
            name,
            func,
            span: signature.span(),
            context,
            is_async,
            is_mut,
            sender,
            arg,
            ret_type: ret_type.unwrap(),
            block: item.default.clone(),
        })
    }

    /// Returns the endpoint declaration.
    pub fn to_decl(&self) -> TokenStream {
        let Self {
            attributes,
            name: _,
            func,
            is_async,
            is_mut,
            sender,
            arg,
            context,
            ret_type,
            block,
            ..
        } = self;

        let s = if *is_mut {
            quote! { &mut self }
        } else {
            quote! { &self }
        };
        let a = if *is_async {
            quote! { async }
        } else {
            quote! {}
        };
        let sender = if let Some((name, ty)) = sender {
            quote! {, #name: #ty}
        } else {
            quote! {}
        };
        let attributes = attributes.iter();
        let block = if let Some(b) = block {
            quote! { #b }
        } else {
            quote! { ; }
        };

        let arg = if let Some((name, ty)) = arg {
            quote! {, #name: #ty}
        } else {
            quote! {}
        };

        let context = if let Some((name, ty)) = context {
            quote! {, #name: #ty}
        } else {
            quote! {}
        };

        quote! {
            #(#attributes)*
            #a fn #func(#s #sender #arg #context) -> #ret_type #block
        }
    }

    pub fn validate_endpoint_pat(&self, namespace: &Option<String>) -> TokenStream {
        let span = self.span;
        let name = self.name.as_str().to_camel_case();
        let ep = match namespace {
            Some(ref namespace) => format!("{namespace}.{name}"),
            None => name,
        };

        let check_anonymous = if self.metadata.deny_anonymous() {
            quote_spanned! { span =>
                if message.from.unwrap_or_default().is_anonymous() {
                    return Err(many_error::ManyError::sender_cannot_be_anonymous());
                }
            }
        } else {
            quote! { {} }
        };

        let check_webauthn = if self.metadata.check_webauthn() {
            quote_spanned! { span => {
                let protected = std::collections::BTreeMap::from_iter(envelope.protected.header.rest.clone().into_iter());
                if !protected.contains_key(&coset::Label::Text("webauthn".to_string())) {
                    return Err(many_error::ManyError::non_webauthn_request_denied(method))
                }
            }}
        } else {
            quote! { {} }
        };

        let check_ty = if let Some((_, ty)) = &self.arg {
            quote_spanned! { span =>
                minicbor::decode::<'_, #ty>(data)
                    .map_err(|e| many_error::ManyError::deserialization_error(e.to_string()))?;
            }
        } else {
            quote! { {} }
        };

        quote_spanned! { span =>
            #ep => {
                #check_anonymous
                #check_webauthn
                #check_ty
            }
        }
    }

    pub fn execute_endpoint_pat(&self, namespace: &Option<String>) -> TokenStream {
        let span = self.span;
        let name = self.name.as_str().to_camel_case();
        let ep = match namespace {
            Some(ref namespace) => format!("{namespace}.{name}"),
            None => name,
        };
        let ep_ident = &self.func;

        let backend_decl = if self.is_mut {
            quote! { let mut backend = self.backend.lock().unwrap(); }
        } else {
            quote! { let backend = self.backend.lock().unwrap(); }
        };

        let call = match (
            self.sender.is_some(),
            self.arg.is_some(),
            self.is_async,
            self.context.is_some(),
        ) {
            (false, true, false, false) => {
                quote_spanned! { span => encode( backend . #ep_ident ( decode( data )? ) ) }
            }
            (false, true, true, false) => {
                quote_spanned! { span => encode( backend . #ep_ident ( decode( data )? ).await ) }
            }
            (true, true, false, false) => {
                quote_spanned! { span => encode( backend . #ep_ident ( &message.from.unwrap_or_default(), decode( data )? ) ) }
            }
            (true, true, true, false) => {
                quote_spanned! { span => encode( backend . #ep_ident ( &message.from.unwrap_or_default(), decode( data )? ).await ) }
            }
            (false, false, false, false) => {
                quote_spanned! { span => encode( backend . #ep_ident ( ) ) }
            }
            (false, false, true, false) => {
                quote_spanned! { span => encode( backend . #ep_ident ( ).await ) }
            }
            (true, false, false, false) => {
                quote_spanned! { span => encode( backend . #ep_ident ( &message.from.unwrap_or_default() ) ) }
            }
            (true, false, true, false) => {
                quote_spanned! { span => encode( backend . #ep_ident ( &message.from.unwrap_or_default() ).await ) }
            }
            (false, true, false, true) => {
                quote_spanned! { span => encode( backend . #ep_ident ( decode( data )?, ctx ) ) }
            }
            (false, true, true, true) => {
                quote_spanned! { span => encode( backend . #ep_ident ( decode( data )?, ctx ).await ) }
            }
            (true, true, false, true) => {
                quote_spanned! { span => encode( backend . #ep_ident ( &message.from.unwrap_or_default(), decode( data )?, ctx ) ) }
            }
            (true, true, true, true) => {
                quote_spanned! { span => encode( backend . #ep_ident ( &message.from.unwrap_or_default(), decode( data )?, ctx ).await ) }
            }
            (false, false, false, true) => {
                quote_spanned! { span => encode( backend . #ep_ident ( ctx ) ) }
            }
            (false, false, true, true) => {
                quote_spanned! { span => encode( backend . #ep_ident ( ctx ).await ) }
            }
            (true, false, false, true) => {
                quote_spanned! { span => encode( backend . #ep_ident ( &message.from.unwrap_or_default(), ctx ) ) }
            }
            (true, false, true, true) => {
                quote_spanned! { span => encode( backend . #ep_ident ( &message.from.unwrap_or_default(), ctx ).await ) }
            }
        };

        quote_spanned! { span =>
            #ep => {
                #backend_decl
                #call
            }
        }
    }
}

impl quote::ToTokens for Endpoint {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(self.to_decl())
    }
}

#[allow(clippy::too_many_lines)]
fn many_module_impl(attr: &TokenStream, item: TokenStream) -> Result<TokenStream, syn::Error> {
    let attrs: ManyModuleAttributes = from_tokenstream(attr)?;
    let many_modules = Ident::new(
        attrs
            .many_modules_crate
            .as_ref()
            .map_or("many_modules", String::as_str),
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

    let vis = tr.vis.clone();
    let trait_ident = if attrs.name.is_none() {
        Ident::new(&format!("{struct_name}Backend"), tr.ident.span())
    } else {
        tr.ident.clone()
    };

    let attr_id = attrs.id.iter();
    let attr_name =
        inflections::Inflect::to_constant_case(format!("{struct_name}Attribute").as_str());
    let attr_ident = Ident::new(&attr_name, attr.span());

    let info_name = format!("{struct_name}Info");
    let info_ident = Ident::new(&info_name, attr.span());

    let endpoints: Vec<Endpoint> = tr
        .items
        .iter()
        .filter_map(|item| {
            if let TraitItem::Method(m) = item {
                Some(Endpoint::new(m))
            } else {
                None
            }
        })
        .collect::<syn::Result<_>>()?;
    let supertraits = tr.supertraits.iter();

    let trait_ = {
        let attributes = tr.attrs.iter();
        quote! {
            #(#attributes)*
            #vis trait #trait_ident: #(#supertraits +)* {
                #(#endpoints)*
            }
        }
    };

    let endpoint_strings: Vec<String> = endpoints
        .iter()
        .map(|e| {
            let name = e.name.as_str().to_camel_case();
            match &namespace {
                Some(ref namespace) => format!("{namespace}.{name}"),
                None => name,
            }
        })
        .collect();

    let validate_endpoint_pat = endpoints
        .iter()
        .map(|e| e.validate_endpoint_pat(&namespace));
    let validate = quote! {
        fn validate(
            &self,
            message: & many_protocol::RequestMessage,
            envelope: & coset::CoseSign1,
        ) -> Result<(), many_error::ManyError> {
            let method = message.method.as_str();
            let data = message.data.as_slice();
            match method {
                #(#validate_endpoint_pat)*

                _ => return Err( many_error::ManyError::invalid_method_name(method.to_string())),
            };
            Ok(())
        }
    };

    let execute_endpoint_pat = endpoints.iter().map(|e| e.execute_endpoint_pat(&namespace));

    let execute = quote! {
        async fn execute(
            &self,
            message: many_protocol::RequestMessage,
        ) -> Result<many_protocol::ResponseMessage, many_error::ManyError> {
            use {
                async_channel::unbounded,
                many_error::ManyError,
                many_protocol::context::{Context, ProofResult},
                many_types::PROOF
            };
            fn decode<'a, T: minicbor::Decode<'a, ()>>(data: &'a [u8]) -> Result<T, ManyError> {
                minicbor::decode(data).map_err(|e| ManyError::deserialization_error(e.to_string()))
            }
            fn encode<T: minicbor::Encode<()>>(result: Result<T, ManyError>) -> Result<Vec<u8>, ManyError> {
                minicbor::to_vec(result?).map_err(|e| ManyError::serialization_error(e.to_string()))
            }

            let data = message.data.as_slice();
            let (transmitter, receiver) = unbounded();
            let ctx = Context::new(message.clone(), transmitter);
            let result = match message.method.as_str() {
                #( #execute_endpoint_pat )*

                _ => Err(ManyError::internal_server_error()),
            }?;

            Ok(if message.attributes.contains(&PROOF) {
                many_protocol::ResponseMessage::from_request(
                    &message,
                    &message.to,
                    Ok(result),
                ).with_attributes(receiver.recv().await.map_err(|e| ManyError::unknown(e.to_string()))?.into_iter().collect::<Result<Vec<_>, _>>()?)
            } else {
                many_protocol::ResponseMessage::from_request(
                    &message,
                    &message.to,
                    Ok(result),
                )
            })
        }
    };

    let attribute = if attrs.id.is_some() {
        quote! { Some(#attr_ident) }
    } else {
        quote! { None }
    };

    Ok(quote! {
        #( #vis const #attr_ident: many_types::attributes::Attribute = many_types::attributes::Attribute::id(#attr_id); )*

        #vis struct #info_ident;
        impl std::ops::Deref for #info_ident {
            type Target =  #many_modules ::ManyModuleInfo;

            fn deref(&self) -> & #many_modules ::ManyModuleInfo {
                use #many_modules ::ManyModuleInfo;
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
        impl<T: #trait_ident>  #many_modules ::ManyModule for #struct_ident<T> {
            fn info(&self) -> & #many_modules ::ManyModuleInfo {
                & #info_ident
            }

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
