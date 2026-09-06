use std::error::Error;

use proc_macro2::Span;
use quote::quote;

use simplicityhl::ast::ElementsJetHinter;
use simplicityhl::{AbiMeta, TemplateProgram, UnstableFeatures};

use super::codegen::{
    GeneratedArgumentTokens, GeneratedWitnessTokens, SimfContractMeta, convert_contract_name_to_contract_module,
};
use super::parse::{SimfContent, SynFilePath};

pub fn expand(input: &SynFilePath) -> syn::Result<proc_macro2::TokenStream> {
    let simf_content = SimfContent::new(input)?;
    let abi_meta = compile_simf(&simf_content).map_err(|e| syn::Error::new(Span::call_site(), e))?;
    let generated = expand_inner(simf_content, abi_meta).map_err(|e| syn::Error::new(Span::call_site(), e))?;

    Ok(generated)
}

fn expand_inner(simf_content: SimfContent, meta: AbiMeta) -> Result<proc_macro2::TokenStream, Box<dyn Error>> {
    let mod_ident = convert_contract_name_to_contract_module(&simf_content.contract_name);

    let derived_meta = SimfContractMeta::try_from(simf_content, meta)?;

    let program_helpers = construct_program_helpers(&derived_meta);
    let witness_helpers = construct_witness_helpers(&derived_meta)?;
    let arguments_helpers = construct_argument_helpers(&derived_meta)?;
    let enum_declarations = derived_meta.enum_declarations();
    let enum_helpers = if enum_declarations.is_empty() {
        quote! {}
    } else {
        let enum_type_provider = construct_enum_type_provider(&derived_meta);
        quote! {
            #enum_type_provider

            /// Enum types declared in the contract ABI. Use this module when an
            /// enum shares a name with the generated Witness or Arguments struct.
            pub mod enums {
                #(#enum_declarations)*
            }

            // Preserve the convenient root paths for names that do not collide
            // with the explicitly re-exported bindings below.
            #[allow(unused_imports)]
            pub use self::enums::*;
        }
    };

    Ok(quote! {
        pub mod #mod_ident{
            #enum_helpers

            #program_helpers

            #witness_helpers

            #arguments_helpers
        }
    })
}

/// Builds the lazily-initialized lookup that resolves declared enum names to
/// their `ResolvedType`s. Enum types cannot be reconstructed outside
/// SimplicityHL, so they are recovered from the contract source itself.
fn construct_enum_type_provider(derived_meta: &SimfContractMeta) -> proc_macro2::TokenStream {
    let contract_source_const = &derived_meta.contract_source_const_name;

    quote! {
        fn abi_types() -> &'static ::simplex::program::AbiTypes {
            static ABI_TYPES: ::std::sync::OnceLock<::simplex::program::AbiTypes> = ::std::sync::OnceLock::new();

            ABI_TYPES.get_or_init(|| ::simplex::program::collect_abi_types(#contract_source_const))
        }

        fn enum_type(name: &::std::primitive::str) -> ::simplex::simplicityhl::ResolvedType {
            abi_types()
                .enum_types
                .get(name)
                .cloned()
                .unwrap_or_else(|| panic!("enum type '{name}' is not part of the contract ABI"))
        }
    }
}

fn construct_program_helpers(derived_meta: &SimfContractMeta) -> proc_macro2::TokenStream {
    let contract_content = &derived_meta.simf_content.content;
    let contract_source_name = &derived_meta.contract_source_const_name;

    quote! {
        pub const #contract_source_name: &::std::primitive::str = #contract_content;
    }
}

fn construct_witness_helpers(derived_meta: &SimfContractMeta) -> syn::Result<proc_macro2::TokenStream> {
    let struct_name = &derived_meta.witness_struct.struct_name;
    let GeneratedWitnessTokens {
        imports,
        struct_token_stream,
        struct_impl,
    } = derived_meta.witness_struct.generate_witness_impl()?;

    Ok(quote! {
        pub use self::build_witness::#struct_name;
        mod build_witness {
            #imports

            #struct_token_stream

            #struct_impl
        }
    })
}

fn construct_argument_helpers(derived_meta: &SimfContractMeta) -> syn::Result<proc_macro2::TokenStream> {
    let struct_name = &derived_meta.args_struct.struct_name;
    let GeneratedArgumentTokens {
        imports,
        struct_token_stream,
        struct_impl,
    } = derived_meta.args_struct.generate_arguments_impl()?;

    Ok(quote! {
        pub use self::build_arguments::#struct_name;
        mod build_arguments {
            #imports

            #struct_token_stream

            #struct_impl
        }
    })
}

fn compile_simf(content: &SimfContent) -> Result<AbiMeta, Box<dyn Error>> {
    let program = content.content.as_str();

    Ok(
        TemplateProgram::new_with_unstable(program, &UnstableFeatures::all(), Box::new(ElementsJetHinter))?
            .generate_abi_meta()?,
    )
}
