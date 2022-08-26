use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};
use syn::parse;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::MetaList;
use syn::MetaNameValue;
use syn::Type;
use syn::{
    parse_macro_input, parse_quote, AttributeArgs, FieldsNamed, ItemStruct, LitStr, NestedMeta,
};

fn build_request_methods<'a>(
    namespace: Option<&'a LitStr>,
    methods: &'a Punctuated<NestedMeta, Comma>,
) -> impl IntoIterator<Item = TokenStream2> + 'a {
    methods.iter().map(move |method| {
        let list = parse::<MetaList>(method.to_token_stream().into())
            .expect("A method should be a MetaList");
        let name = list.path.get_ident().unwrap();
        let mut params = None;
        let mut returns = None;
        for element in list.nested.iter() {
            let name_value = parse::<MetaNameValue>(element.to_token_stream().into())
                .expect("Elements should be MetaNameValue");
            let name = name_value.path.get_ident().unwrap();
            if name == "params" {
                let str = parse::<LitStr>(name_value.lit.to_token_stream().into())
                    .expect("Params should be a string or undeclared");
                params = Some(str.parse().unwrap());
            } else if name == "returns" {
                let str = parse::<LitStr>(name_value.lit.to_token_stream().into())
                    .expect("Returns should be a string or undeclared");
                returns = Some(str.parse().unwrap());
            } else {
                panic!("Unknown field {:?}", element);
            }
        }
        let returns = returns
            .map(|r: Type| quote! { #r })
            .unwrap_or(quote! { () });
        let params: TokenStream2 = params
            .map(|params: Type| quote! { params: #params })
            .unwrap_or_else(TokenStream2::new);
        let method: LitStr = if let Some(namespace) = namespace {
            let namespace = format!("{}.{}", namespace.value(), name);
            parse_quote! { #namespace }
        } else {
            let namespace = format!("{}", name);
            parse_quote! { #namespace }
        };
        let params_call = if params.is_empty() {
            quote! { () }
        } else {
            quote! { params }
        };
        let q = quote! {
            pub async fn #name(&self, #params) -> Result<#returns, many_protocol::ManyError> {
                let response = self.client.call_(#method, #params_call).await?;
                minicbor::decode(&response).map_err(many_protocol::ManyError::deserialization_error)
            }
        };
        q.into_token_stream()
    })
}

#[proc_macro_attribute]
pub fn many_client(attr: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as AttributeArgs);

    let mut namespace = None;
    let mut methods = None;

    for arg in args.iter() {
        if let Ok(name_value) = parse::<MetaNameValue>(arg.to_token_stream().into()) {
            if name_value.path.get_ident().unwrap() == "namespace" {
                let namespace_lit = name_value.lit;
                let namespace_lit: LitStr = parse_quote! { #namespace_lit };
                namespace = Some(namespace_lit);
            }
        } else if let Ok(list) = parse::<MetaList>(arg.to_token_stream().into()) {
            if list.path.get_ident().unwrap() == "methods" {
                methods = Some(list.nested);
            }
        } else {
            panic!("Unknown argument {:?}", arg);
        }
    }

    let mut mapped_methods = proc_macro2::TokenStream::new();
    if let Some(methods) = methods.as_ref() {
        mapped_methods.extend(build_request_methods(namespace.as_ref(), methods));
    }

    let mut input_struct = parse_macro_input!(input as ItemStruct);

    assert!(
        input_struct.fields.is_empty(),
        "The base struct should be a unit struct"
    );

    let fields_named: FieldsNamed = parse_quote! { { client: crate::ManyClient } };
    input_struct.fields = fields_named.into();

    let struct_name = &input_struct.ident;

    let gen = quote! {
        #[derive(Clone, Debug)]
        #input_struct

        impl #struct_name {
            pub fn new(client: crate::ManyClient) -> Self {
                Self { client }
            }

            #mapped_methods
        }
    };
    gen.into()
}
