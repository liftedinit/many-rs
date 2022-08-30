use proc_macro::TokenStream;
use proc_macro2::Punct;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::parse2;
use syn::spanned::Spanned;
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

    let methods_vec = input_trait.items.iter().map(|func| {
        let namespace = namespace.clone();
        let func = func.to_token_stream();
        let method: TraitItemMethod =
            parse2(func)?;
        let mut method = method.sig;
        method.asyncness = parse_quote! { async };
        let mut args_iter = method.inputs.iter();
        let _self_arg = args_iter.next().ok_or_else(|| syn::Error::new(method.span(), "Should have a &self argument"))?;
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
                minicbor::decode(&response).map_err(many_server::ManyError::deserialization_error)
            }
        };
        Ok(q.into_token_stream())
    }).try_fold(vec![], |mut acc, curr: syn::Result<TokenStream2>| {
        match curr {
            Ok(c) => acc.push(c),
            Err(e) => return Err(e)
        }
        Ok(acc)
    });
    let methods_vec = match methods_vec {
        Ok(v) => v,
        Err(e) => return e.to_compile_error().into(),
    };
    let methods_iter = methods_vec.into_iter();

    let methods = TokenStream2::from_iter(methods_iter);

    let q = quote! {
        impl<I: many_identity::Identity> #r#type<I> {
            #methods

            pub fn new(client: crate::ManyClient<I>) -> Self {
                Self(client)
            }
        }
    };
    q.into()
}
