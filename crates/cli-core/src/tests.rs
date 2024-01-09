#![cfg(test)]

use crate::command;
use proc_macro2::TokenStream;
use quote::quote;

fn assert_tokens_eq(expected: &TokenStream, actual: &TokenStream) {
    let expected = expected.to_string();
    let actual = actual.to_string();

    if expected != actual {
        println!(
            "{}",
            colored_diff::PrettyDifference {
                expected: &expected,
                actual: &actual,
            }
        );
        println!("expected: {}", &expected);
        println!("actual  : {}", &actual);

        let path = format!("test-failure.rs");
        std::fs::write(&path, actual).expect("Failed to write test output");
        std::process::Command::new("rustfmt")
            .arg(path)
            .output()
            .expect("Failed to rustfmt");

        panic!("expected != actual");
    }
}

#[test]
fn sandbox() {
    return;
    let before = quote! {
        fn onearg(x: usize) {
            println!("Hello, universe!");
        }
    };
    let after = command(quote!(), before);
    let expected = quote!(
        fn hello_world() {}
    );
    assert_tokens_eq(&expected, &after);
}

#[test]
fn noargs() {
    let before = quote! {
        fn noargs() {
            println!("Hello, universe!");
        }
    };
    let after = command(quote!(), before);
    let expected = quote!(
        pub fn noargs(cli: &mut ::cli::Cli) {
            fn impl_noargs() {
                println!("Hello, universe!");
            }
            let cmd = ::cli::clap::Command::new("noargs");
            cli.register_command(
                cmd,
                |ctx: &mut ::cli::Context, args: &::cli::clap::ArgMatches| -> ::cli::Result {
                    impl_noargs();
                    Ok(())
                },
            );
        }
    );
    assert_tokens_eq(&expected, &after);
}

#[test]
fn onearg() {
    let before = quote! {
        fn onearg(x: usize) {
            println!("Hello, universe!");
        }
    };
    let after = command(quote!(), before);
    let expected = quote!(
        pub fn onearg(cli: &mut ::cli::Cli) {
            fn impl_onearg(x: usize) {
                println!("Hello, universe!");
            }
            let cmd = ::cli::clap::Command::new("onearg")
                .arg(::cli::clap::Arg::new("x").value_parser(::cli::clap::value_parser!(usize)));
            cli.register_command(
                cmd,
                |ctx: &mut ::cli::Context, args: &::cli::clap::ArgMatches| -> ::cli::Result {
                    impl_onearg(*args.try_get_one::<usize>("x").unwrap().unwrap());
                    Ok(())
                },
            );
        }
    );
    assert_tokens_eq(&expected, &after);
}
