#[macro_use]
mod cdsl;
mod srcgen;

pub mod error;
pub mod isa;

mod gen_inst;
mod gen_legalizer;
mod gen_registers;
mod gen_settings;
mod gen_types;

mod constant_hash;
mod shared;
mod unique_table;

pub fn isa_from_arch(arch: &str) -> Result<isa::Isa, String> {
    isa::Isa::from_arch(arch).ok_or_else(|| format!("no supported isa found for arch `{}`", arch))
}

/// Generates all the Rust source files used in Cranelift from the meta-language.
pub fn generate(isas: &Vec<isa::Isa>, out_dir: &str) -> Result<(), error::Error> {
    // Common definitions.
    let mut shared_defs = shared::define();

    gen_settings::generate(
        &shared_defs.settings,
        gen_settings::ParentGroup::None,
        "new_settings.rs",
        &out_dir,
    )?;
    gen_types::generate("types.rs", &out_dir)?;

    // Per ISA definitions.
    let isas = isa::define(isas, &mut shared_defs);

    let mut all_inst_groups = vec![&shared_defs.instructions];
    all_inst_groups.extend(isas.iter().map(|isa| &isa.instructions));

    gen_inst::generate(
        all_inst_groups,
        &shared_defs.format_registry,
        "opcodes.rs",
        "inst_builder.rs",
        &out_dir,
    )?;

    gen_legalizer::generate(
        &isas,
        &shared_defs.format_registry,
        &shared_defs.transform_groups,
        "new_legalize",
        &out_dir,
    )?;

    for isa in isas {
        gen_registers::generate(&isa, &format!("registers-{}.rs", isa.name), &out_dir)?;
        gen_settings::generate(
            &isa.settings,
            gen_settings::ParentGroup::Shared,
            &format!("new_settings-{}", isa.name),
            &out_dir,
        )?;
    }

    Ok(())
}
