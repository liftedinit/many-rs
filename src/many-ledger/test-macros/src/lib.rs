use proc_macro::{self, TokenStream};
use quote::quote;

#[proc_macro_derive(LedgerWorld)]
pub fn derive_many_world(input: TokenStream) -> TokenStream {
    // Construct a representation of Rust code as a syntax tree
    // that we can manipulate
    let ast = syn::parse(input).expect("LedgerWorld syn parsing failed");

    // Build the trait implementation
    impl_many_world_macro(&ast)
}

fn impl_many_world_macro(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let gen = quote! {
        impl LedgerWorld for #name {
            fn setup_id(&self) -> Address {
                self.setup.id
            }

            fn module_impl(&self) -> &LedgerModuleImpl {
                &self.setup.module_impl
            }

            fn module_impl_mut(&mut self) -> &mut LedgerModuleImpl {
                &mut self.setup.module_impl
            }

            fn error(&self) -> &Option<ManyError> {
                &self.error
            }
        }
    };
    gen.into()
}

#[proc_macro_derive(TokenWorld)]
pub fn derive_token_world(input: TokenStream) -> TokenStream {
    // Construct a representation of Rust code as a syntax tree
    // that we can manipulate
    let ast = syn::parse(input).expect("TokenWorld syn parsing failed");

    // Build the trait implementation
    impl_token_world_macro(&ast)
}

fn impl_token_world_macro(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let gen = quote! {
        impl TokenWorld for #name {
            fn info(&self) -> &TokenInfo {
                &self.info
            }

            fn info_mut(&mut self) -> &mut TokenInfo {
                &mut self.info
            }

            fn ext_info_mut(&mut self) -> &mut TokenExtendedInfo {
                &mut self.ext_info
            }
        }
    };
    gen.into()
}

#[proc_macro_derive(AccountWorld)]
pub fn derive_account_world(input: TokenStream) -> TokenStream {
    // Construct a representation of Rust code as a syntax tree
    // that we can manipulate
    let ast = syn::parse(input).expect("AccountWorld syn parsing failed");

    // Build the trait implementation
    impl_account_world_macro(&ast)
}

fn impl_account_world_macro(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let gen = quote! {
        impl AccountWorld for #name {
            fn account(&self) -> Address {
                self.account
            }

            fn account_mut(&mut self) -> &mut Address {
                &mut self.account
            }
        }
    };
    gen.into()
}
