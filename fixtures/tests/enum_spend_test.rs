use simplex::constants::DUMMY_SIGNATURE;
use simplex::transaction::{FinalTransaction, PartialInput, ProgramInput, RequiredSignature};

use simplex_fixtures::artifacts::enum_spend::EnumSpendProgram;
use simplex_fixtures::artifacts::enum_spend::derived_enum_spend::{Action, EnumSpendArguments, EnumSpendWitness};

fn spend_enum_variant(context: &simplex::TestContext, action: Action, sig_path: &[&str]) -> anyhow::Result<()> {
    let signer = context.get_default_signer();
    let provider = context.get_default_provider();

    let arguments = EnumSpendArguments {
        public_key: signer.get_schnorr_public_key().serialize(),
    };

    let program = EnumSpendProgram::new(&arguments);
    let script = program.get_script_pubkey(context.get_network());

    let tx_receipt = signer.send(script.clone(), 50_000)?;
    println!("Funded: {}", tx_receipt);

    let utxos = provider.fetch_scripthash_utxos(&script)?;

    let mut ft = FinalTransaction::new();

    ft.add_program_input(
        PartialInput::new(utxos[0].clone()),
        ProgramInput::new(
            Box::new(program.as_ref().clone()),
            Box::new(EnumSpendWitness { action }),
        ),
        RequiredSignature::witness_with_path("ACTION", sig_path),
    );

    let tx_receipt = signer.broadcast(&ft)?;
    println!("Broadcast: {}", tx_receipt);

    Ok(())
}

#[simplex::test]
fn test_inherit_spend(context: simplex::TestContext) -> anyhow::Result<()> {
    spend_enum_variant(&context, Action::Inherit(DUMMY_SIGNATURE), &["Inherit"])
}

#[simplex::test]
fn test_cold_spend(context: simplex::TestContext) -> anyhow::Result<()> {
    spend_enum_variant(&context, Action::Cold(DUMMY_SIGNATURE), &["Cold"])
}

#[simplex::test]
fn test_hot_spend(context: simplex::TestContext) -> anyhow::Result<()> {
    spend_enum_variant(&context, Action::Hot(DUMMY_SIGNATURE), &["Hot"])
}
