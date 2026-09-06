use std::collections::BTreeSet;
use std::fmt::Display;

use quote::{format_ident, quote};

use simplicityhl::ResolvedType;

/// A variant of a generated Rust enum: its name and payload types.
#[derive(Debug, Clone)]
pub struct RustEnumVariant {
    pub name: String,
    pub payload: Vec<RustType>,
}

/// A nominal enum type mirrored from SimplicityHL.
#[derive(Debug, Clone)]
pub struct RustEnum {
    /// Original nominal name used by the SimplicityHL ABI.
    pub name: String,
    /// Rust spelling, resolved against all enum names in the contract ABI.
    pub rust_name: proc_macro2::Ident,
    pub variants: Vec<RustEnumVariant>,
}

impl RustEnum {
    fn variant_ident(&self, variant: &RustEnumVariant) -> proc_macro2::Ident {
        rust_identifier(&variant.name, |name| {
            self.variants.iter().any(|other| other.name == name)
        })
    }
}

fn rust_identifier(name: &str, is_taken: impl Fn(&str) -> bool) -> proc_macro2::Ident {
    let mut name = name.to_owned();
    // Path keywords and the single underscore cannot be raw identifiers. Keep
    // declared names intact and append underscores until a name is available.
    if matches!(name.as_str(), "self" | "Self" | "super" | "crate" | "_") {
        name.push('_');
        while is_taken(&name) {
            name.push('_');
        }
    }
    // Raw identifiers also cover Rust keywords added in later editions.
    proc_macro2::Ident::new_raw(&name, proc_macro2::Span::call_site())
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum RustType {
    Bool,
    U1,
    U2,
    U4,
    U8,
    U16,
    U32,
    U64,
    U128,
    U256Array,
    Array(Box<RustType>, usize),
    Tuple(Vec<RustType>),
    Either(Box<RustType>, Box<RustType>),
    Option(Box<RustType>),
    List(Box<RustType>, usize),
    Enum(RustEnum),
}

#[derive(Debug, Clone, Copy)]
enum RustTypeContext {
    Root,
    Array,
    Tuple,
    EitherLeft,
    EitherRight,
    Option,
    List,
    EnumVariant,
}

impl Display for RustTypeContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            RustTypeContext::Root => "root element".to_string(),
            RustTypeContext::Array => "array element".to_string(),
            RustTypeContext::Tuple => "tuple element".to_string(),
            RustTypeContext::EitherLeft => "left either branch".to_string(),
            RustTypeContext::EitherRight => "right either branch".to_string(),
            RustTypeContext::Option => "option element".to_string(),
            RustTypeContext::List => "list element".to_string(),
            RustTypeContext::EnumVariant => "enum variant payload".to_string(),
        };
        write!(f, "{str}")
    }
}

impl RustTypeContext {
    fn is_deref_needed(&self) -> bool {
        match self {
            RustTypeContext::Array | RustTypeContext::Tuple | RustTypeContext::Root => false,
            RustTypeContext::List
            | RustTypeContext::EitherLeft
            | RustTypeContext::EitherRight
            | RustTypeContext::Option
            | RustTypeContext::EnumVariant => true,
        }
    }
}

impl RustType {
    pub(super) fn resolve_enum_names<'a>(types: impl IntoIterator<Item = &'a mut RustType>) {
        let mut types: Vec<_> = types.into_iter().collect();
        let mut names = BTreeSet::new();
        for ty in &mut types {
            ty.visit_enums_mut(&mut |def| {
                names.insert(def.name.clone());
            });
        }
        for ty in &mut types {
            ty.visit_enums_mut(&mut |def| {
                def.rust_name = rust_identifier(&def.name, |name| names.contains(name));
            });
        }
    }

    fn visit_enums_mut(&mut self, visit: &mut impl FnMut(&mut RustEnum)) {
        match self {
            RustType::Enum(def) => {
                visit(def);
                for payload in def.variants.iter_mut().flat_map(|variant| &mut variant.payload) {
                    payload.visit_enums_mut(visit);
                }
            }
            RustType::Array(element, _) | RustType::Option(element) | RustType::List(element, _) => {
                element.visit_enums_mut(visit);
            }
            RustType::Either(left, right) => {
                left.visit_enums_mut(visit);
                right.visit_enums_mut(visit);
            }
            RustType::Tuple(elements) => {
                for element in elements {
                    element.visit_enums_mut(visit);
                }
            }
            _ => {}
        }
    }

    pub(super) fn contains_enum(&self) -> bool {
        match self {
            RustType::Enum(_) => true,
            RustType::Array(element, _) | RustType::Option(element) | RustType::List(element, _) => {
                element.contains_enum()
            }
            RustType::Either(left, right) => left.contains_enum() || right.contains_enum(),
            RustType::Tuple(elements) => elements.iter().any(RustType::contains_enum),
            _ => false,
        }
    }

    pub fn get_default_value(&self) -> proc_macro2::TokenStream {
        match self {
            RustType::Bool => quote! { Default::default() },
            RustType::U1 => quote! { Default::default() },
            RustType::U2 => quote! { Default::default() },
            RustType::U4 => quote! { Default::default() },
            RustType::U8 => quote! { Default::default() },
            RustType::U16 => quote! { Default::default() },
            RustType::U32 => quote! { Default::default() },
            RustType::U64 => quote! { Default::default() },
            RustType::U128 => quote! { Default::default() },
            RustType::U256Array => quote! { [Default::default(); 32] },
            RustType::Array(element, _size) => {
                let element_ty = element.get_default_value();
                quote! { ::std::array::from_fn(|_| #element_ty) }
            }
            RustType::Tuple(elements) => {
                let element_types: Vec<_> = elements.iter().map(RustType::get_default_value).collect();
                quote! { (#(#element_types),*) }
            }
            RustType::Either(left, _) => {
                let left_ty = left.get_default_value();
                quote! { ::simplex::either::Either::Left(#left_ty) }
            }
            RustType::Option(_inner) => {
                quote! { Default::default() }
            }
            RustType::List(_element, _size) => {
                quote! { Default::default() }
            }
            RustType::Enum(def) => {
                let first_variant = def.variants.first().expect("enums have at least one variant");
                let enum_ident = &def.rust_name;
                let variant_ident = def.variant_ident(first_variant);
                if first_variant.payload.is_empty() {
                    quote! { super::enums::#enum_ident::#variant_ident }
                } else {
                    let payload_defaults = first_variant.payload.iter().map(RustType::get_default_value);
                    quote! { super::enums::#enum_ident::#variant_ident(#(#payload_defaults),*) }
                }
            }
        }
    }

    pub fn from_resolved_type(ty: &ResolvedType) -> syn::Result<Self> {
        let mut ty = Self::from_resolved_type_inner(ty)?;
        Self::resolve_enum_names([&mut ty]);
        Ok(ty)
    }

    fn from_resolved_type_inner(ty: &ResolvedType) -> syn::Result<Self> {
        use simplicityhl::types::{TypeInner, UIntType};

        match ty.as_inner() {
            TypeInner::Boolean => Ok(RustType::Bool),
            TypeInner::UInt(uint_ty) => match uint_ty {
                UIntType::U1 => Ok(RustType::U1),
                UIntType::U2 => Ok(RustType::U2),
                UIntType::U4 => Ok(RustType::U4),
                UIntType::U8 => Ok(RustType::U8),
                UIntType::U16 => Ok(RustType::U16),
                UIntType::U32 => Ok(RustType::U32),
                UIntType::U64 => Ok(RustType::U64),
                UIntType::U128 => Ok(RustType::U128),
                UIntType::U256 => Ok(RustType::U256Array),
            },
            TypeInner::Either(left, right) => {
                let left_ty = Self::from_resolved_type_inner(left)?;
                let right_ty = Self::from_resolved_type_inner(right)?;
                Ok(RustType::Either(Box::new(left_ty), Box::new(right_ty)))
            }
            TypeInner::Option(inner) => {
                let inner_ty = Self::from_resolved_type_inner(inner)?;
                Ok(RustType::Option(Box::new(inner_ty)))
            }
            TypeInner::Tuple(elements) => {
                let element_types: syn::Result<Vec<_>> =
                    elements.iter().map(|e| Self::from_resolved_type_inner(e)).collect();
                Ok(RustType::Tuple(element_types?))
            }
            TypeInner::Array(element, size) => {
                let element_ty = Self::from_resolved_type_inner(element)?;
                Ok(RustType::Array(Box::new(element_ty), *size))
            }
            TypeInner::List(element, size) => {
                let element_ty = Self::from_resolved_type_inner(element)?;
                Ok(RustType::List(Box::new(element_ty), size.get()))
            }
            TypeInner::Enum(info) => {
                let variants = info
                    .variants()
                    .iter()
                    .map(|variant| {
                        let payload = variant
                            .payload()
                            .iter()
                            .map(Self::from_resolved_type_inner)
                            .collect::<syn::Result<Vec<_>>>()?;

                        Ok(RustEnumVariant {
                            name: variant.name().to_string(),
                            payload,
                        })
                    })
                    .collect::<syn::Result<Vec<_>>>()?;

                Ok(RustType::Enum(RustEnum {
                    name: info.name().to_string(),
                    rust_name: rust_identifier(info.name(), |_| false),
                    variants,
                }))
            }
            _ => Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "Unsupported type in macro conversions",
            )),
        }
    }

    /// Generate the Rust type as a `TokenStream` for struct field declarations
    pub fn to_type_token_stream(&self) -> proc_macro2::TokenStream {
        self.to_type_token_stream_with_enum_prefix(&quote! { super::enums:: })
    }

    // Structs live in helper modules, while enum payloads live in the enums
    // module. Explicit paths avoid collisions with bindings and helper imports.
    fn to_type_token_stream_with_enum_prefix(
        &self,
        enum_prefix: &proc_macro2::TokenStream,
    ) -> proc_macro2::TokenStream {
        match self {
            RustType::Bool => quote! { bool },
            RustType::U1 => quote! { u8 },
            RustType::U2 => quote! { u8 },
            RustType::U4 => quote! { u8 },
            RustType::U8 => quote! { u8 },
            RustType::U16 => quote! { u16 },
            RustType::U32 => quote! { u32 },
            RustType::U64 => quote! { u64 },
            RustType::U128 => quote! { u128 },
            RustType::U256Array => quote! { [u8; 32] },
            RustType::Array(element, size) => {
                let element_ty = element.to_type_token_stream_with_enum_prefix(enum_prefix);
                quote! { [#element_ty; #size] }
            }
            RustType::Tuple(elements) => {
                let element_types: Vec<_> = elements
                    .iter()
                    .map(|element| element.to_type_token_stream_with_enum_prefix(enum_prefix))
                    .collect();
                quote! { (#(#element_types),*) }
            }
            RustType::Either(left, right) => {
                let left_ty = left.to_type_token_stream_with_enum_prefix(enum_prefix);
                let right_ty = right.to_type_token_stream_with_enum_prefix(enum_prefix);
                quote! { ::simplex::either::Either<#left_ty, #right_ty> }
            }
            RustType::Option(inner) => {
                let inner_ty = inner.to_type_token_stream_with_enum_prefix(enum_prefix);
                quote! { ::std::option::Option<#inner_ty> }
            }
            RustType::List(element, _size) => {
                let element_ty = element.to_type_token_stream_with_enum_prefix(enum_prefix);
                quote! { ::std::vec::Vec<#element_ty> }
            }
            RustType::Enum(def) => {
                let enum_ident = &def.rust_name;
                quote! { #enum_prefix #enum_ident }
            }
        }
    }

    pub fn generate_to_simplicity_conversion(&self, value_expr: &proc_macro2::TokenStream) -> proc_macro2::TokenStream {
        self.generate_to_simplicity_conversion_inner(value_expr, None)
    }

    fn generate_to_simplicity_conversion_inner(
        &self,
        value_expr: &proc_macro2::TokenStream,
        prev_type: Option<RustTypeContext>,
    ) -> proc_macro2::TokenStream {
        let deref = {
            if let Some(type_context) = prev_type
                && type_context.is_deref_needed()
            {
                quote! { * }
            } else {
                quote! {}
            }
        };
        match self {
            RustType::Bool => {
                quote! { Value::from(#deref #value_expr) }
            }
            RustType::U1 => {
                quote! { Value::from(UIntValue::u1(#deref #value_expr).map_err(|_e| format!("Failed to create U1 type, got: '{}', [value size in bits: '{}']", #value_expr.checked_ilog2().unwrap_or_default(), #value_expr)).unwrap()) }
            }
            RustType::U2 => {
                quote! { Value::from(UIntValue::u2(#deref #value_expr).map_err(|_e| format!("Failed to create U2 type, got: '{}', [value size in bits: '{}']", #value_expr.checked_ilog2().unwrap_or_default(), #value_expr)).unwrap()) }
            }
            RustType::U4 => {
                quote! { Value::from(UIntValue::u4(#deref #value_expr).map_err(|_e| format!("Failed to create U4 type, got: '{}', [value size in bits: '{}']", #value_expr.checked_ilog2().unwrap_or_default(), #value_expr)).unwrap()) }
            }
            RustType::U8 => {
                quote! { Value::from(UIntValue::U8(#deref #value_expr)) }
            }
            RustType::U16 => {
                quote! { Value::from(UIntValue::U16(#deref #value_expr)) }
            }
            RustType::U32 => {
                quote! { Value::from(UIntValue::U32(#deref #value_expr)) }
            }
            RustType::U64 => {
                quote! { Value::from(UIntValue::U64(#deref #value_expr)) }
            }
            RustType::U128 => {
                quote! { Value::from(UIntValue::U128(#deref #value_expr)) }
            }
            RustType::U256Array => {
                quote! { Value::from(UIntValue::U256(U256::from_byte_array(#deref #value_expr))) }
            }
            RustType::Array(element, size) => {
                let indices: Vec<_> = (0..*size).map(syn::Index::from).collect();
                let element_conversions: Vec<_> = indices
                    .iter()
                    .map(|idx| {
                        let elem_expr = quote! { #value_expr[#idx] };
                        element.generate_to_simplicity_conversion_inner(&elem_expr, Some(RustTypeContext::Array))
                    })
                    .collect();

                let elem_ty_generation = element.generate_simplicity_type_construction();

                quote! {
                    {
                        let elements = [#(#element_conversions),*];
                        Value::array(elements, #elem_ty_generation)
                    }
                }
            }
            RustType::Tuple(elements) => {
                if elements.is_empty() {
                    quote! { Value::unit() }
                } else {
                    let tuple_conversions = elements.iter().enumerate().map(|(i, elem_ty)| {
                        let idx = syn::Index::from(i);
                        let elem_expr = quote! { #value_expr.#idx };

                        elem_ty.generate_to_simplicity_conversion_inner(&elem_expr, Some(RustTypeContext::Tuple))
                    });

                    quote! {
                        Value::tuple([#(#tuple_conversions),*])
                    }
                }
            }
            RustType::Either(left, right) => {
                let left_conv = left
                    .generate_to_simplicity_conversion_inner(&quote! { left_val }, Some(RustTypeContext::EitherLeft));
                let right_conv = right
                    .generate_to_simplicity_conversion_inner(&quote! { right_val }, Some(RustTypeContext::EitherRight));
                let left_ty = left.generate_simplicity_type_construction();
                let right_ty = right.generate_simplicity_type_construction();

                quote! {
                    match &#value_expr {
                        ::simplex::either::Either::Left(left_val) => {
                            Value::left(
                                #left_conv,
                                #right_ty
                            )
                        }
                        ::simplex::either::Either::Right(right_val) => {
                            Value::right(
                                #left_ty,
                                #right_conv
                            )
                        }
                    }
                }
            }
            RustType::Option(inner) => {
                let inner_conv =
                    inner.generate_to_simplicity_conversion_inner(&quote! { inner_val }, Some(RustTypeContext::Option));
                let inner_ty = inner.generate_simplicity_type_construction();

                quote! {
                    match &#value_expr {
                        None => {
                            Value::none(#inner_ty)
                        }
                        Some(inner_val) => {
                            Value::some(#inner_conv)
                        }
                    }
                }
            }
            RustType::List(element, size) => {
                let iter_tmp_var_name = quote! { x };
                let element_conversion = {
                    element.generate_to_simplicity_conversion_inner(&iter_tmp_var_name, Some(RustTypeContext::List))
                };
                let elem_ty_generation = element.generate_simplicity_type_construction();

                quote! {
                    {
                        let elements = #value_expr.iter().map(| #iter_tmp_var_name| #element_conversion).collect::<Vec<_>>();
                        let non_zero_pow2_size = NonZeroPow2Usize::new(#size).ok_or_else(|| format!("Failed to create non zero pow2 length, got size: '{}'", #size)).unwrap();

                        assert!(elements.len() < non_zero_pow2_size.get(), "There must be fewer list elements than the bound '{}'", non_zero_pow2_size.get());

                        Value::list(elements, #elem_ty_generation, non_zero_pow2_size)
                    }
                }
            }
            RustType::Enum(def) => {
                let ty_generation = self.generate_simplicity_type_construction();
                let enum_ident = &def.rust_name;
                let enum_name = &def.name;

                let variant_arms = def.variants.iter().map(|variant| {
                    let variant_ident = def.variant_ident(variant);
                    let variant_name = &variant.name;

                    if variant.payload.is_empty() {
                        quote! {
                            super::enums::#enum_ident::#variant_ident => Value::enum_variant(
                                &#ty_generation,
                                &Identifier::from_str_unchecked(#variant_name),
                                Vec::new(),
                            )
                            .unwrap_or_else(|| panic!("Failed to construct enum variant '{}::{}'", #enum_name, #variant_name))
                        }
                    } else {
                        let payload_bindings: Vec<_> = (0..variant.payload.len())
                            .map(|i| format_ident!("payload_{i}"))
                            .collect();
                        let payload_conversions: Vec<_> = variant
                            .payload
                            .iter()
                            .zip(&payload_bindings)
                            .map(|(payload, binding)| {
                                payload.generate_to_simplicity_conversion_inner(
                                    &quote! { #binding },
                                    Some(RustTypeContext::EnumVariant),
                                )
                            })
                            .collect();

                        quote! {
                            super::enums::#enum_ident::#variant_ident(#(#payload_bindings),*) => Value::enum_variant(
                                &#ty_generation,
                                &Identifier::from_str_unchecked(#variant_name),
                                vec![#(#payload_conversions),*],
                            )
                            .unwrap_or_else(|| panic!("Failed to construct enum variant '{}::{}'", #enum_name, #variant_name))
                        }
                    }
                });

                quote! {
                    match &#value_expr {
                        #(#variant_arms),*
                    }
                }
            }
        }
    }

    pub fn generate_simplicity_type_construction(&self) -> proc_macro2::TokenStream {
        match self {
            RustType::Bool => {
                quote! { ResolvedType::boolean() }
            }
            RustType::U1 => {
                quote! { ResolvedType::u1() }
            }
            RustType::U2 => {
                quote! { ResolvedType::u2() }
            }
            RustType::U4 => {
                quote! { ResolvedType::u4() }
            }
            RustType::U8 => {
                quote! { ResolvedType::u8() }
            }
            RustType::U16 => {
                quote! { ResolvedType::u16() }
            }
            RustType::U32 => {
                quote! { ResolvedType::u32() }
            }
            RustType::U64 => {
                quote! { ResolvedType::u64() }
            }
            RustType::U128 => {
                quote! { ResolvedType::u128() }
            }
            RustType::U256Array => {
                quote! { ResolvedType::u256() }
            }
            RustType::Array(element, size) => {
                let elem_ty = element.generate_simplicity_type_construction();
                quote! { ResolvedType::array(#elem_ty, #size) }
            }
            RustType::Tuple(elements) => {
                let elem_types: Vec<_> = elements
                    .iter()
                    .map(RustType::generate_simplicity_type_construction)
                    .collect();
                quote! { ResolvedType::tuple([#(#elem_types),*]) }
            }
            RustType::Either(left, right) => {
                let left_ty = left.generate_simplicity_type_construction();
                let right_ty = right.generate_simplicity_type_construction();
                quote! { ResolvedType::either(#left_ty, #right_ty) }
            }
            RustType::Option(inner) => {
                let inner_ty = inner.generate_simplicity_type_construction();
                quote! { ResolvedType::option(#inner_ty) }
            }
            RustType::List(element, size) => {
                let elem_ty = element.generate_simplicity_type_construction();
                quote! { ResolvedType::list(#elem_ty, NonZeroPow2Usize::new(#size).ok_or_else(|| format!("Failed to create non zero pow2 length, got size: '{}'", #size)).unwrap()) }
            }
            RustType::Enum(def) => {
                let enum_name = &def.name;
                quote! { enum_type(#enum_name) }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn generate_from_value_extraction(
        &self,
        args_expr: &proc_macro2::Ident,
        witness_name: &str,
    ) -> proc_macro2::TokenStream {
        let initial_arg_name = quote! { value };
        let get_witness_expr_tokens = quote! {
            let witness_name = WitnessName::from_str_unchecked(#witness_name);
            let #initial_arg_name = #args_expr
                .get(&witness_name)
                .ok_or_else(|| format!("Missing witness: {}", #witness_name))?;
        };
        let expand_value_extraction =
            self.generate_value_extraction_from_expr(&initial_arg_name, RustTypeContext::Root);
        // Check the complete field type before inspecting any variant indices or
        // payloads. This also validates enum types in None and empty containers.
        let type_validation = if self.contains_enum() {
            let expected_type = self.generate_simplicity_type_construction();
            quote! {
                let expected_type = #expected_type;
                if #initial_arg_name.ty() != &expected_type {
                    return Err(format!(
                        "Wrong type or enum definition for {}: expected {}, got {}",
                        #witness_name, expected_type, #initial_arg_name.ty()
                    ));
                }
            }
        } else {
            quote! {}
        };

        quote! {
            {
                #get_witness_expr_tokens
                #type_validation
                #expand_value_extraction
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn generate_value_extraction_from_expr(
        &self,
        value_expr: &proc_macro2::TokenStream,
        context: RustTypeContext,
    ) -> proc_macro2::TokenStream {
        let context = format!("{context:?}");
        match self {
            RustType::Bool => quote! {
                match #value_expr.inner() {
                    ::simplex::simplicityhl::value::ValueInner::Boolean(b) => *b,
                    _ => return Err(format!("Wrong type for {}: expected bool", #context)),
                }
            },
            RustType::U1 => quote! {
                match #value_expr.inner() {
                    ::simplex::simplicityhl::value::ValueInner::UInt(UIntValue::U1(v)) => *v,
                    _ => return Err(format!("Wrong type for {}: expected U1", #context)),
                }
            },
            RustType::U2 => quote! {
                match #value_expr.inner() {
                    ::simplex::simplicityhl::value::ValueInner::UInt(UIntValue::U2(v)) => *v,
                    _ => return Err(format!("Wrong type for {}: expected U2", #context)),
                }
            },
            RustType::U4 => quote! {
                match #value_expr.inner() {
                    ::simplex::simplicityhl::value::ValueInner::UInt(UIntValue::U4(v)) => *v,
                    _ => return Err(format!("Wrong type for {}: expected U4", #context)),
                }
            },
            RustType::U8 => quote! {
                match #value_expr.inner() {
                    ::simplex::simplicityhl::value::ValueInner::UInt(UIntValue::U8(v)) => *v,
                    _ => return Err(format!("Wrong type for {}: expected U8", #context)),
                }
            },
            RustType::U16 => quote! {
                match #value_expr.inner() {
                    ::simplex::simplicityhl::value::ValueInner::UInt(UIntValue::U16(v)) => *v,
                    _ => return Err(format!("Wrong type for {}: expected U16", #context)),
                }
            },
            RustType::U32 => quote! {
                match #value_expr.inner() {
                    ::simplex::simplicityhl::value::ValueInner::UInt(UIntValue::U32(v)) => *v,
                    _ => return Err(format!("Wrong type for {}: expected U32", #context)),
                }
            },
            RustType::U64 => quote! {
                match #value_expr.inner() {
                    ::simplex::simplicityhl::value::ValueInner::UInt(UIntValue::U64(v)) => *v,
                    _ => return Err(format!("Wrong type for {}: expected U64", #context)),
                }
            },
            RustType::U128 => quote! {
                match #value_expr.inner() {
                    ::simplex::simplicityhl::value::ValueInner::UInt(UIntValue::U128(v)) => *v,
                    _ => return Err(format!("Wrong type for {}: expected U128", #context)),
                }
            },
            RustType::U256Array => quote! {
                match #value_expr.inner() {
                    ::simplex::simplicityhl::value::ValueInner::UInt(UIntValue::U256(u256)) => u256.to_byte_array(),
                    _ => return Err(format!("Wrong type for {}: expected U256", #context)),
                }
            },
            RustType::Array(element, size) => {
                let elem_extractions: Vec<_> = (0..*size)
                    .map(|i| {
                        element.generate_value_extraction_from_expr(&quote! { arr_val[#i] }, RustTypeContext::Array)
                    })
                    .collect();

                quote! {
                    match #value_expr.inner() {
                        ::simplex::simplicityhl::value::ValueInner::Array(arr_val) => {
                            if arr_val.len() != #size {
                                return Err(format!("Wrong array length for {}: expected {}, got {}", #context, #size, arr_val.len()));
                            }

                            [#(#elem_extractions),*]
                        }
                        _ => return Err(format!("Wrong type for {}: expected Array", #context)),
                    }
                }
            }
            RustType::Tuple(elements) => {
                let tuple_len = elements.len();
                let elem_extractions: Vec<_> = elements
                    .iter()
                    .enumerate()
                    .map(|(i, elem_ty)| {
                        elem_ty.generate_value_extraction_from_expr(&quote! { tuple_val[#i] }, RustTypeContext::Tuple)
                    })
                    .collect();

                quote! {
                    match #value_expr.inner() {
                        ::simplex::simplicityhl::value::ValueInner::Tuple(tuple_val) => {
                            if tuple_val.len() != #tuple_len {
                                return Err(format!("Wrong tuple length for {}", #context));
                            }

                            (#(#elem_extractions),*)
                        }
                        _ => return Err(format!("Wrong type for {}: expected Tuple", #context)),
                    }
                }
            }
            RustType::Either(left, right) => {
                let left_extraction =
                    left.generate_value_extraction_from_expr(&quote! { left_val }, RustTypeContext::EitherLeft);
                let right_extraction =
                    right.generate_value_extraction_from_expr(&quote! { right_val }, RustTypeContext::EitherRight);

                quote! {
                    match #value_expr.inner() {
                        ::simplex::simplicityhl::value::ValueInner::Either(either_val) => {
                            match either_val {
                                ::simplex::either::Either::Left(left_val) => {
                                    ::simplex::either::Either::Left(#left_extraction)
                                }
                                ::simplex::either::Either::Right(right_val) => {
                                    ::simplex::either::Either::Right(#right_extraction)
                                }
                            }
                        }
                        _ => return Err(format!("Wrong type for {}: expected Either", #context)),
                    }
                }
            }
            RustType::Option(inner) => {
                let inner_extraction =
                    inner.generate_value_extraction_from_expr(&quote! { some_val }, RustTypeContext::Option);

                quote! {
                    match #value_expr.inner() {
                        ::simplex::simplicityhl::value::ValueInner::Option(opt_val) => {
                            match opt_val {
                                None => None,
                                Some(some_val) => Some(#inner_extraction),
                            }
                        }
                        _ => return Err(format!("Wrong type for {}: expected Option", #context)),
                    }
                }
            }
            RustType::List(element, _size) => {
                let iter_index = quote! { i };
                let list_name = quote! { list_value };
                let elem_extraction = element
                    .generate_value_extraction_from_expr(&quote! { #list_name[#iter_index] }, RustTypeContext::List);

                quote! {
                    match #value_expr.inner() {
                        ::simplex::simplicityhl::value::ValueInner::List(#list_name, non_zero_pow2_size) => {
                            let list_len = #list_name.len();

                            if list_len >= non_zero_pow2_size.get() {
                                return Err(format!("Wrong list length for {}: expected less than {}, got {}", #context, non_zero_pow2_size.get(), list_len));
                            }

                            let mut res = Vec::with_capacity(list_len);

                            for #iter_index in 0..list_len {
                                res.push(#elem_extraction);
                            }

                            res
                        }
                        _ => return Err(format!("Wrong type for {}: expected List", #context)),
                    }
                }
            }
            RustType::Enum(def) => {
                let enum_ident = &def.rust_name;
                let enum_name = &def.name;

                let variant_arms = def.variants.iter().enumerate().map(|(index, variant)| {
                    let variant_ident = def.variant_ident(variant);
                    let payload_extractions: Vec<_> = variant
                        .payload
                        .iter()
                        .enumerate()
                        .map(|(i, payload)| {
                            payload.generate_value_extraction_from_expr(
                                &quote! { enum_payload[#i] },
                                RustTypeContext::EnumVariant,
                            )
                        })
                        .collect();

                    if variant.payload.is_empty() {
                        quote! { #index => super::enums::#enum_ident::#variant_ident }
                    } else {
                        quote! { #index => super::enums::#enum_ident::#variant_ident(#(#payload_extractions),*) }
                    }
                });

                quote! {
                    match #value_expr.inner() {
                        ::simplex::simplicityhl::value::ValueInner::Enum(enum_variant_index, enum_payload) => {
                            match *enum_variant_index {
                                #(#variant_arms),*,
                                _ => return Err(format!("Unknown enum variant for {}: got index {}", #context, enum_variant_index)),
                            }
                        }
                        _ => return Err(format!("Wrong type for {}: expected enum '{}'", #context, #enum_name)),
                    }
                }
            }
        }
    }

    /// Collect the declarations of every enum type reachable from this type,
    /// deduplicated by enum name and in depth-first declaration order.
    pub fn collect_enum_declarations(&self, declarations: &mut Vec<(String, proc_macro2::TokenStream)>) {
        match self {
            RustType::Enum(def) => {
                for variant in &def.variants {
                    for payload in &variant.payload {
                        payload.collect_enum_declarations(declarations);
                    }
                }

                if !declarations.iter().any(|(name, _)| *name == def.name) {
                    declarations.push((def.name.clone(), self.enum_declaration_token_stream()));
                }
            }
            RustType::Array(element, _) | RustType::Option(element) | RustType::List(element, _) => {
                element.collect_enum_declarations(declarations);
            }
            RustType::Either(left, right) => {
                left.collect_enum_declarations(declarations);
                right.collect_enum_declarations(declarations);
            }
            RustType::Tuple(elements) => {
                for element in elements {
                    element.collect_enum_declarations(declarations);
                }
            }
            _ => {}
        }
    }

    fn enum_declaration_token_stream(&self) -> proc_macro2::TokenStream {
        let RustType::Enum(def) = self else {
            unreachable!("only enum types declare new types");
        };

        let enum_ident = &def.rust_name;
        let variant_tokens = def.variants.iter().map(|variant| {
            let variant_ident = def.variant_ident(variant);
            if variant.payload.is_empty() {
                quote! { #variant_ident }
            } else {
                let payload_types = variant
                    .payload
                    .iter()
                    .map(|payload| payload.to_type_token_stream_with_enum_prefix(&quote! { self:: }));
                quote! { #variant_ident(#(#payload_types),*) }
            }
        });

        quote! {
            #[derive(Debug, Clone, PartialEq, Eq)]
            #[allow(non_camel_case_types)]
            pub enum #enum_ident {
                #(#variant_tokens),*
            }
        }
    }
}
