use proc_macro::TokenStream;
use quote::{format_ident, quote};
use serde_json::Value;
use syn::{LitStr, parse_macro_input};

#[proc_macro]
pub fn json_to_test_macro(input: TokenStream) -> TokenStream {
    let lit = parse_macro_input!(input as LitStr);
    let file_name = lit.value();

    let cases: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(file_name).unwrap()).unwrap();

    let tests = cases.into_iter().map(|case| {
        let fn_name = format_ident!(
            "{}_{}",
            case["section"]
                .to_string()
                .replace(' ', "_")
                .replace('\"', "")
                .to_lowercase(),
            case["example"].to_string()
        );
        let md_input = case["markdown"].to_string();
        let html_out = case["html"].to_string();

        quote! {
            #[test]
            fn #fn_name() {
                assert_eq!(markdown_to_html(#md_input), #html_out);
            }
        }
    });

    let expanded = quote! {
        #(#tests)*
    };

    expanded.into()
}
