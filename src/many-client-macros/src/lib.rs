use proc_macro::TokenStream;
use proc_macro2::Punct;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::parse2;
use syn::FnArg;
use syn::ItemTrait;
use syn::TraitItemMethod;
use syn::Type;
use syn::{parse_macro_input, parse_quote, LitStr};

#[derive(Debug)]
struct MacroArguments {
    r#type: Type,
    namespace: Option<LitStr>,
}

impl Parse for MacroArguments {
    fn parse(stream: ParseStream) -> syn::Result<MacroArguments> {
        let r#type = stream.parse()?;
        let _: syn::Result<Punct> = stream.parse();
        let namespace = stream.parse().ok();
        let result = Ok(MacroArguments { r#type, namespace });
        if !stream.is_empty() {
            return Err(stream.error("Shouldn't have more than 2 arguments"));
        }
        result
    }
}

#[proc_macro_attribute]
pub fn many_client(attr: TokenStream, input: TokenStream) -> TokenStream {
    let MacroArguments { r#type, namespace } = parse_macro_input!(attr as MacroArguments);

    let input_trait = parse_macro_input!(input as ItemTrait);

    let methods_iter = input_trait.items.iter().map(|func| {
        let namespace = namespace.clone();
        let func = func.to_token_stream();
        let method: TraitItemMethod =
            parse2(func).expect("Should only contain function signatures");
        let mut method = method.sig;
        method.asyncness = parse_quote! { async };
        let mut args_iter = method.inputs.iter();
        let _self_arg = args_iter.next().expect("Should have a &self argument");
        let args_param = args_iter.next();
        let args_var = if let Some(FnArg::Typed(args)) = args_param {
            let args = &args.pat;
            quote! { #args }
        } else {
            quote! { () }
        };
        let server_method = if let Some(namespace) = namespace {
            format!("{}.{}", namespace.value(), method.ident)
        } else {
            format!("{}", method.ident)
        };
        let server_method: LitStr = parse_quote! { #server_method };
        let q = quote! {
            pub #method {
                let response = self.0.call_(#server_method, #args_var).await?;
                minicbor::decode(&response).map_err(many_protocol::ManyError::deserialization_error)
            }
        };
        q.into_token_stream()
    });

    let mut methods = TokenStream2::new();
    methods.extend(methods_iter);

    let q = quote! {
        impl #r#type {
            #methods

            pub fn new(client: crate::ManyClient) -> Self {
                Self(client)
            }
        }
    };
    q.into()
}
