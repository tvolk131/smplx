use std::collections::HashMap;

use simplex::program::{ArgumentsTrait, WitnessTrait, collect_abi_types};
use simplex::simplicityhl::num::NonZeroPow2Usize;
use simplex::simplicityhl::str::{Identifier, WitnessName};
use simplex::simplicityhl::value::ValueConstructible;
use simplex::simplicityhl::{Arguments, Value, WitnessValues};

simplex::include_simf!("tests/ui_simfs/enums.simf");

use derived_enums::{EnumsArguments, EnumsWitness};

fn foreign_enum_value(name: &str, variants: &str, variant: &str, payload: Vec<Value>) -> Value {
    let source = format!("enum {name} {{ {variants} }} fn main() {{ let mode: {name} = witness::MODE; }}");
    let abi = collect_abi_types(&source);
    Value::enum_variant(&abi.enum_types[name], &Identifier::from_str_unchecked(variant), payload).unwrap()
}

fn other_off() -> Value {
    // Same variants and payload layout as Mode, but a different nominal type.
    foreign_enum_value("Other", "Off, Single(u32), Pair(u32, u64),", "Off", vec![])
}

fn witness_with(name: &str, value: Value) -> WitnessValues {
    let mut values: HashMap<_, _> = EnumsWitness::default()
        .build_witness()
        .iter()
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    values.insert(WitnessName::from_str_unchecked(name), value);
    WitnessValues::from(values)
}

fn arguments_with(name: &str, value: Value) -> Arguments {
    let mut values: HashMap<_, _> = EnumsArguments::default()
        .build_arguments()
        .iter()
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    values.insert(WitnessName::from_str_unchecked(name), value);
    Arguments::from(values)
}

fn assert_rejected_by_both(name: &str, value: Value) {
    assert!(
        EnumsWitness::from_witness(&witness_with(name, value.clone())).is_err(),
        "witness field {name} accepted an invalid enum value"
    );
    assert!(
        EnumsArguments::from_arguments(&arguments_with(name, value)).is_err(),
        "argument field {name} accepted an invalid enum value"
    );
}

#[test]
fn rejects_different_enum_with_matching_layout() {
    assert_rejected_by_both("MODE", other_off());
}

#[test]
fn rejects_shorter_payload_without_panicking() {
    let value = foreign_enum_value("Other", "Alpha, Beta,", "Beta", vec![]);
    assert_rejected_by_both("MODE", value);
}

#[test]
fn rejects_same_enum_name_with_different_definition() {
    let value = foreign_enum_value("Mode", "Off, Single(u64),", "Off", vec![]);
    assert_rejected_by_both("MODE", value);
}

#[test]
fn rejects_foreign_enum_inside_option() {
    assert_rejected_by_both("MAYBE", Value::some(other_off()));
}

#[test]
fn rejects_foreign_enum_type_in_empty_containers() {
    let other_type = other_off().ty().clone();
    for (name, value) in [
        ("MAYBE", Value::none(other_type.clone())),
        ("LIST", Value::list([], other_type, NonZeroPow2Usize::new(4).unwrap())),
    ] {
        assert_rejected_by_both(name, value);
    }
}

#[test]
fn valid_enum_values_still_round_trip() {
    let value = derived_enums::Mode::Single(7);
    let witness = EnumsWitness {
        mode: value.clone(),
        maybe: Some(value.clone()),
        ..Default::default()
    };
    assert_eq!(witness, EnumsWitness::from_witness(&witness.build_witness()).unwrap());

    let arguments = EnumsArguments {
        mode: value.clone(),
        maybe: Some(value),
        ..Default::default()
    };
    assert_eq!(
        arguments,
        EnumsArguments::from_arguments(&arguments.build_arguments()).unwrap()
    );
}
