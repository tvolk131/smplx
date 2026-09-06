use std::collections::HashMap;
use std::sync::Arc;

use simplicityhl::ast::ElementsJetHinter;
use simplicityhl::types::TypeInner;
use simplicityhl::{Parameters, ResolvedType, TemplateProgram, UnstableFeatures, WitnessTypes};

/// The full ABI of a contract: its parameter and witness types, plus every
/// nominal enum type reachable from them, keyed by declared enum name.
#[derive(Debug, Clone)]
pub struct AbiTypes {
    /// Types of the contract's parameters.
    pub parameters: Parameters,
    /// Types of the contract's witnesses.
    pub witness_types: WitnessTypes,
    /// Every enum type reachable from the ABI, keyed by declared name.
    pub enum_types: HashMap<String, ResolvedType>,
}

/// Compile `source` and collect its ABI types.
///
/// Enum types cannot be reconstructed from scratch outside the `SimplicityHL`
/// crate, so generated code uses this to recover them from the program itself.
/// The source is only parsed and analyzed, never fully compiled, and callers
/// are expected to cache the result.
///
/// # Panics
/// Panics if the source fails to parse or analyze. Callers only pass sources
/// that were already validated at macro-expansion time.
#[must_use]
pub fn collect_abi_types(source: &str) -> AbiTypes {
    let template =
        TemplateProgram::new_with_unstable(Arc::from(source), &UnstableFeatures::all(), Box::new(ElementsJetHinter))
            .expect("source was validated at macro-expansion time");

    let parameters = template.parameters().shallow_clone();
    let witness_types = template.witness_types().shallow_clone();

    let mut enum_types = HashMap::new();
    for (_, ty) in template.parameters().iter() {
        collect_from_type(ty, &mut enum_types);
    }
    for (_, ty) in template.witness_types().iter() {
        collect_from_type(ty, &mut enum_types);
    }

    AbiTypes {
        parameters,
        witness_types,
        enum_types,
    }
}

fn collect_from_type(ty: &ResolvedType, enum_types: &mut HashMap<String, ResolvedType>) {
    match ty.as_inner() {
        TypeInner::Enum(info) => {
            enum_types.insert(info.name().to_string(), ty.clone());
            for variant in info.variants() {
                for payload_ty in variant.payload() {
                    collect_from_type(payload_ty, enum_types);
                }
            }
        }
        TypeInner::Either(left, right) => {
            collect_from_type(left, enum_types);
            collect_from_type(right, enum_types);
        }
        TypeInner::Option(inner) => collect_from_type(inner, enum_types),
        TypeInner::Tuple(elements) => {
            for element in elements.iter() {
                collect_from_type(element, enum_types);
            }
        }
        TypeInner::Array(element, _) | TypeInner::List(element, _) => collect_from_type(element, enum_types),
        _ => {}
    }
}

#[cfg(test)]
mod test {
    use simplicityhl::str::WitnessName;
    use simplicityhl::value::ValueConstructible;
    use simplicityhl::{Arguments, UnresolvedValues, Value, WitnessValues};

    use super::*;

    const ENUM_SOURCE: &str = "
enum Mode {
    Off,
    Single(u32),
    Pair(u32, u64),
}

fn main() {
    let argument: Mode = param::MODE;
    let witness: Mode = witness::MODE;
}
";

    /// The deserialization path used by generated code for enum-containing
    /// contracts: bare value strings resolve against the program's declared
    /// types, which is the only way to recover nominal enum values.
    #[test]
    fn unresolved_values_resolve_enum_witnesses() {
        let abi = collect_abi_types(ENUM_SOURCE);

        let witness_name = WitnessName::from_str_unchecked("MODE");
        assert!(abi.witness_types.get(&witness_name).is_some());

        let unresolved: UnresolvedValues =
            serde_json::from_str(r#"{ "MODE": "Mode::Single(7)" }"#).expect("bare form deserializes");
        let resolved: WitnessValues = unresolved.resolve(&abi.witness_types).expect("resolves");

        let expected_mode_ty = &abi.enum_types["Mode"];
        let expected = Value::enum_variant(
            expected_mode_ty,
            &simplicityhl::str::Identifier::from_str_unchecked("Single"),
            vec![Value::u32(7)],
        )
        .unwrap();

        assert_eq!(resolved.get(&witness_name), Some(&expected));
    }

    #[test]
    fn unresolved_values_resolve_enum_parameters() {
        let abi = collect_abi_types(ENUM_SOURCE);

        let unresolved: UnresolvedValues = serde_json::from_str(r#"{ "MODE": "Mode::Pair(1, 2)" }"#).unwrap();
        let resolved: Arguments = unresolved.resolve(&abi.parameters).unwrap();

        let expected_mode_ty = &abi.enum_types["Mode"];
        let expected = Value::enum_variant(
            expected_mode_ty,
            &simplicityhl::str::Identifier::from_str_unchecked("Pair"),
            vec![Value::u32(1), Value::u64(2)],
        )
        .unwrap();

        let param_name = WitnessName::from_str_unchecked("MODE");
        assert_eq!(resolved.get(&param_name), Some(&expected));
    }

    #[test]
    fn unresolved_values_reject_unknown_variant() {
        let abi = collect_abi_types(ENUM_SOURCE);

        let unresolved: UnresolvedValues = serde_json::from_str(r#"{ "MODE": "Mode::Nope(1)" }"#).unwrap();
        let err = unresolved.resolve::<WitnessValues, _>(&abi.witness_types).unwrap_err();

        assert!(err.contains("MODE"), "error names the witness: {err}");
    }
}
