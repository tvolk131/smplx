use simplex::either::Either;
use simplex::program::{ArgumentsTrait, WitnessTrait};

simplex::include_simf!("../../../../crates/simplex/tests/ui_simfs/enum_names.simf");

use derived_enum_names::{Arguments, EnumNamesArguments, EnumNamesWitness, Result as EnumResult, Value, WitnessValues};
use derived_enum_names::{core as EnumCore, simplex as EnumSimplex, std as EnumStd, str as EnumStr};

fn main() -> Result<(), String> {
    // Enum names may collide with helper imports, crate paths, and Rust types.
    for action in [
        EnumSimplex::Choice(Either::Left(EnumStd::Values([7, 42]))),
        EnumSimplex::Choice(Either::Right(99)),
    ] {
        let args = Arguments::Wrapped(Value::Single(7));
        let values = WitnessValues::Wrapped(args.clone());
        let result = EnumResult::Wrapped(derived_enum_names::Vec::Items(vec![values.clone()]));
        let wrapped = EnumCore::Wrapped(Some(action.clone()));
        let witness = EnumNamesWitness {
            value: Value::Single(42),
            argument: args.clone(),
            values: values.clone(),
            result: result.clone(),
            items: vec![Value::Off, Value::Single(9)],
            action: action.clone(),
            wrapped: wrapped.clone(),
            label: EnumStr::Empty,
        };
        assert_eq!(witness, EnumNamesWitness::from_witness(&witness.build_witness())?);
        let arguments = EnumNamesArguments {
            value: Value::Single(42),
            argument: args,
            values,
            result,
            items: vec![Value::Off, Value::Single(9)],
            action,
            wrapped,
            label: EnumStr::Empty,
        };
        assert_eq!(
            arguments,
            EnumNamesArguments::from_arguments(&arguments.build_arguments())?
        );
    }
    let default_witness = EnumNamesWitness::default();
    assert_eq!(
        default_witness.action,
        EnumSimplex::Choice(Either::Left(EnumStd::Values([0, 0])))
    );
    assert_eq!(
        default_witness,
        EnumNamesWitness::from_witness(&default_witness.build_witness())?
    );
    let default_arguments = EnumNamesArguments::default();
    assert_eq!(
        default_arguments,
        EnumNamesArguments::from_arguments(&default_arguments.build_arguments())?
    );
    Ok(())
}
