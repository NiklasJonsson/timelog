mod tests;

use proc_macro2::{Ident, Span, TokenStream};
use proc_macro_error::abort;
use quote::quote;
use syn::{
    parse2, parse_quote,
    punctuated::Punctuated,
    spanned::Spanned,
    token::{Comma, Token},
    FnArg, ItemFn, Pat, Signature,
};

struct Arg {
    ident: Ident,
    ident_str: String,
    ty: Box<syn::Type>,
}

fn extract_args(fn_inputs: &Punctuated<FnArg, Comma>) -> Vec<Arg> {
    let mut out = vec![];
    for input in fn_inputs {
        match input {
            FnArg::Receiver(r) => abort!(r, "Functions taking self are not supported"),
            FnArg::Typed(pat_type) => match &*pat_type.pat {
                Pat::Ident(pat_ident) => {
                    out.push(Arg {
                        ident: pat_ident.ident.clone(),
                        ident_str: pat_ident.ident.to_string(),
                        ty: pat_type.ty.clone(),
                    });
                }
                _ => abort!(pat_type.span(), "Unsupported"),
            },
        }
    }
    out
}

pub fn command(args: TokenStream, input: TokenStream) -> TokenStream {
    if !args.is_empty() {
        abort!(args, "anyinput does not take any arguments.")
    }

    // proc_marco2 version of "parse_macro_input!(input as ItemFn)"
    let old_item_fn = match parse2::<ItemFn>(input) {
        Ok(syntax_tree) => syntax_tree,
        Err(error) => return error.to_compile_error(),
    };

    transform_fn(old_item_fn)
}

fn transform_fn(item_fn: ItemFn) -> TokenStream {
    println!("input code  : {}", quote!(#item_fn));
    println!("input syntax: {:?}", item_fn);
    let ctx_arg = Ident::new("ctx", Span::call_site());
    let name = &item_fn.sig.ident;
    let name_str = name.to_string();
    let impl_name = Ident::new(&format!("impl_{name}"), Span::call_site());

    let fn_footer = match &item_fn.sig.output {
        syn::ReturnType::Default => quote! { Ok(())},
        syn::ReturnType::Type(..) => TokenStream::new(),
    };

    let impl_fn = ItemFn {
        sig: Signature {
            ident: impl_name.clone(),
            ..item_fn.sig
        },
        ..item_fn
    };

    let fn_args = extract_args(&impl_fn.sig.inputs);
    let mut clap_builder: TokenStream = quote! {
        ::cli::clap::Command::new(#name_str)
    };

    for Arg { ident_str, ty, .. } in &fn_args {
        clap_builder = parse_quote! {
            #clap_builder
            .arg(::cli::clap::Arg::new(#ident_str).value_parser(::cli::clap::value_parser!(#ty)))
        };
    }

    let mut invoc_args: Punctuated<syn::Expr, Comma> = Punctuated::new();
    for Arg { ident_str, ty, .. } in &fn_args {
        let ty = ty.clone();
        invoc_args.push(parse_quote! {
            // TODO: Cleanup deref
            *args.try_get_one::<#ty>(#ident_str).unwrap().unwrap()
        });
    }

    // TODO: Should we return (clap::Command, impl Fn) instead?
    quote! {
        pub fn #name(cli: &mut ::cli::Cli) {
            #impl_fn

            let cmd = #clap_builder;
            cli.register_command(cmd, |#ctx_arg: &mut ::cli::Globals, args: &::cli::clap::ArgMatches| -> ::cli::Result {
                #impl_name(#invoc_args);

                #fn_footer
            },
        );
        }
    }
}
