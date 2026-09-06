use simplex::include_simf;
use simplex::program::{ArgumentsTrait, WitnessTrait};

include_simf!("../../../../crates/simplex/tests/ui_simfs/enums.simf");

use derived_enums::{Mode, Sealed, Wrapper};

fn main() -> Result<(), String> {
    // Defaults round-trip: first declared variant (unit for Mode/Wrapper,
    // payload-defaulted for Sealed), None, empty list.
    let default_witness = derived_enums::EnumsWitness::default();
    assert_eq!(
        default_witness,
        derived_enums::EnumsWitness::from_witness(&default_witness.build_witness())?
    );

    let default_arguments = derived_enums::EnumsArguments::default();
    assert_eq!(
        default_arguments,
        derived_enums::EnumsArguments::from_arguments(&default_arguments.build_arguments())?
    );

    for mode in [Mode::Off, Mode::Single(7), Mode::Pair(1, 2)] {
        for wrapper in [
            Wrapper::Plain,
            Wrapper::Wrapped(mode.clone()),
            Wrapper::Maybe(None),
            Wrapper::Maybe(Some(mode.clone())),
            Wrapper::Bag([mode.clone(), mode.clone()]),
        ] {
            let original_witness = derived_enums::EnumsWitness {
                mode: mode.clone(),
                wrapper: wrapper.clone(),
                maybe: Some(mode.clone()),
                either: simplex::either::Either::Left(mode.clone()),
                tuple: (mode.clone(), wrapper.clone()),
                array: [mode.clone(), mode.clone()],
                list: vec![mode.clone(), mode.clone()],
                sealed: Sealed::Only(42),
            };
            let witness_values = original_witness.build_witness();
            assert_eq!(
                original_witness,
                derived_enums::EnumsWitness::from_witness(&witness_values)?
            );

            let original_arguments = derived_enums::EnumsArguments {
                mode: mode.clone(),
                wrapper: wrapper.clone(),
                maybe: Some(mode.clone()),
                either: simplex::either::Either::Right(wrapper.clone()),
                tuple: (mode.clone(), wrapper.clone()),
                array: [mode.clone(), mode.clone()],
                list: vec![mode.clone()],
                sealed: Sealed::Only(42),
            };
            let arguments_values = original_arguments.build_arguments();
            assert_eq!(
                original_arguments,
                derived_enums::EnumsArguments::from_arguments(&arguments_values)?
            );
        }
    }

    Ok(())
}
