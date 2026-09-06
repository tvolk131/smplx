use simplex::program::{ArgumentsTrait, WitnessTrait};
use simplex::simplicityhl::str::WitnessName;

simplex::include_simf!("../../../../crates/simplex/tests/ui_simfs/enum_type_names.simf");

use derived_enum_type_names::{__ as Double, ___ as Triple, ____ as Underscore};
use derived_enum_type_names::{
    EnumTypeNamesArguments, EnumTypeNamesWitness, Self_, Self__, Self___, r#move, self_, super_,
};

fn main() -> Result<(), String> {
    let action = r#move::Wrapped(Self___::Number(42));
    let other = Self_::Flag(true);
    let lower = self_::Maybe(Some(action.clone()));
    let parent = super_::Items(vec![Self__::Number(99)]);
    let witness = EnumTypeNamesWitness {
        action: action.clone(),
        other: other.clone(),
        lower: lower.clone(),
        parent: parent.clone(),
        underscore: Underscore::Number(23),
        double: Double::Flag(true),
        triple: Triple::Number(99),
    };
    let values = witness.build_witness();
    assert_eq!(witness, EnumTypeNamesWitness::from_witness(&values)?);
    // Escaped Rust names must leave the nominal ABI types unchanged.
    for (name, expected) in [
        ("ACTION", "move"),
        ("OTHER", "Self_"),
        ("LOWER", "self"),
        ("PARENT", "super"),
        ("UNDERSCORE", "_"),
        ("DOUBLE", "__"),
        ("TRIPLE", "___"),
    ] {
        assert_eq!(
            values
                .get(&WitnessName::from_str_unchecked(name))
                .unwrap()
                .ty()
                .to_string(),
            expected
        );
    }
    let arguments = EnumTypeNamesArguments {
        action,
        other,
        lower,
        parent,
        underscore: Underscore::Number(23),
        double: Double::Flag(true),
        triple: Triple::Number(99),
    };
    assert_eq!(
        arguments,
        EnumTypeNamesArguments::from_arguments(&arguments.build_arguments())?
    );

    let witness = EnumTypeNamesWitness::default();
    assert_eq!(witness.action, r#move::Wrapped(Self___::Number(0)));
    assert_eq!(witness.underscore, Underscore::Number(0));
    assert_eq!(witness, EnumTypeNamesWitness::from_witness(&witness.build_witness())?);
    let arguments = EnumTypeNamesArguments::default();
    assert_eq!(arguments.underscore, Underscore::Number(0));
    assert_eq!(
        arguments,
        EnumTypeNamesArguments::from_arguments(&arguments.build_arguments())?
    );
    Ok(())
}
