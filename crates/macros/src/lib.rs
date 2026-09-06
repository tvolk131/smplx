#![warn(clippy::all, clippy::pedantic)]

use proc_macro::TokenStream;

/// Generate a contract module with typed arguments, witnesses, and ABI enums.
///
/// Enums are available under `derived_<contract>::enums` and re-exported at the
/// module root unless their names collide with generated bindings. Witness and
/// Arguments structs always keep their established root names.
///
/// Rust keyword enum and variant names use raw identifiers, such as `r#move`.
/// For `self`, `Self`, `super`, and `_`, which Rust cannot escape, underscores are
/// appended until the name is distinct from all enum names in the contract ABI,
/// or from all variants in the containing enum. For example, `Self` becomes
/// `Self_`, or `Self__` if an enum named `Self_` is also part of the ABI.
/// Likewise, `_` becomes `__`, or `___` if `__` is already declared in that scope.
/// These Rust spellings do not change enum or variant names in `SimplicityHL` values.
#[proc_macro]
pub fn include_simf(tokenstream: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(tokenstream as smplx_build::macros::parse::SynFilePath);

    match smplx_build::macros::expand(&input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro_attribute]
pub fn test(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::ItemFn);

    match smplx_test::macros::expand(args.into(), input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}
