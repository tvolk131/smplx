use simplex::program::{ArgumentsTrait, WitnessTrait};

simplex::include_simf!("../../../../crates/simplex/tests/ui_simfs/enum_collisions.simf");

use derived_enum_collisions::{EnumCollisionsArguments, EnumCollisionsWitness};

fn main() -> Result<(), String> {
    // The established root names must continue to refer to the generated structs.
    let default_witness = EnumCollisionsWitness::default();
    assert_eq!(
        default_witness,
        EnumCollisionsWitness::from_witness(&default_witness.build_witness())?
    );
    let default_arguments = EnumCollisionsArguments::default();
    assert_eq!(
        default_arguments,
        EnumCollisionsArguments::from_arguments(&default_arguments.build_arguments())?
    );

    // Every enum remains accessible, including those sharing a struct's name.
    use derived_enum_collisions::enums::{EnumCollisionsArguments as Wrapped, EnumCollisionsWitness as Action};
    let action = Action::Single(42);
    let wrapped = Wrapped::Wrapped(action.clone());
    let witness = EnumCollisionsWitness {
        action: action.clone(),
        wrapped: wrapped.clone(),
    };
    assert_eq!(witness, EnumCollisionsWitness::from_witness(&witness.build_witness())?);
    let arguments = EnumCollisionsArguments { action, wrapped };
    assert_eq!(
        arguments,
        EnumCollisionsArguments::from_arguments(&arguments.build_arguments())?
    );
    Ok(())
}
