use proc_macro::TokenStream;
use quote::{format_ident, quote};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use syn::{LitStr, parse_macro_input};

#[derive(Serialize, Deserialize)]
struct TestCase {
    markdown: String,
    html: String,
    example: u32,
    section: String,
}

#[proc_macro]
pub fn json_to_test_macro(input: TokenStream) -> TokenStream {
    let lit = parse_macro_input!(input as LitStr);
    let file_name = lit.value();

    let cases: Vec<TestCase> =
        serde_json::from_str(&std::fs::read_to_string(file_name).unwrap()).unwrap();

    let tests = cases.into_iter().map(|case| {
        let fn_name = format_ident!(
            "{}_{}",
            &case.section.replace(' ', "_").to_lowercase(),
            &case.example
        );
        let md_input = case.markdown;
        let html_out = case.html;

        quote! {
            #[test]
            fn #fn_name() {
                println!("input:\n{}",#md_input);
                assert_eq!(markdown_to_html(#md_input), #html_out);
            }
        }
    });

    let expanded = quote! {
        #(#tests)*
    };

    expanded.into()
}
