use simplex::program::{ArgumentsTrait, WitnessTrait};
use simplex::simplicityhl::str::WitnessName;

simplex::include_simf!("../../../../crates/simplex/tests/ui_simfs/enum_keywords.simf");

use derived_enum_keywords::{Action, EnumKeywordsArguments, EnumKeywordsWitness, Wrapper};

fn main() -> Result<(), String> {
    for action in [
        Action::____(42),
        Action::__,
        Action::___(99),
        Action::r#move(42),
        Action::r#async,
        Action::r#dyn(7, 99),
        Action::r#gen,
        Action::self___,
        Action::self_,
        Action::self__,
        Action::Self_,
        Action::super_,
    ] {
        let wrapped = Wrapper::r#yield(Some(action.clone()));
        let witness = EnumKeywordsWitness {
            action: action.clone(),
            wrapped: wrapped.clone(),
        };
        assert_eq!(witness, EnumKeywordsWitness::from_witness(&witness.build_witness())?);
        let arguments = EnumKeywordsArguments { action, wrapped };
        assert_eq!(
            arguments,
            EnumKeywordsArguments::from_arguments(&arguments.build_arguments())?
        );
    }

    let witness = EnumKeywordsWitness::default();
    assert_eq!(witness.action, Action::____(0));
    assert_eq!(
        witness
            .build_witness()
            .get(&WitnessName::from_str_unchecked("ACTION"))
            .unwrap()
            .to_string(),
        "Action::_(0)"
    );
    assert_eq!(witness.wrapped, Wrapper::r#yield(None));
    assert_eq!(witness, EnumKeywordsWitness::from_witness(&witness.build_witness())?);
    let arguments = EnumKeywordsArguments::default();
    assert_eq!(arguments.action, Action::____(0));
    assert_eq!(
        arguments,
        EnumKeywordsArguments::from_arguments(&arguments.build_arguments())?
    );
    Ok(())
}
