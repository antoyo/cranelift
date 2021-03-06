use crate::cdsl::cpu_modes::CpuMode;
use crate::cdsl::isa::TargetIsa;
use crate::cdsl::regs::{IsaRegs, IsaRegsBuilder, RegBankBuilder, RegClassBuilder};
use crate::cdsl::settings::{PredicateNode, SettingGroup, SettingGroupBuilder};

use crate::shared::types::Bool::B1;
use crate::shared::types::Float::{F32, F64};
use crate::shared::types::Int::{I16, I32, I64, I8};
use crate::shared::Definitions as SharedDefinitions;

mod instructions;
mod legalize;

fn define_settings(_shared: &SettingGroup) -> SettingGroup {
    let mut settings = SettingGroupBuilder::new("x86");

    // CPUID.01H:ECX
    let has_sse3 = settings.add_bool("has_sse3", "SSE3: CPUID.01H:ECX.SSE3[bit 0]", false);
    let has_ssse3 = settings.add_bool("has_ssse3", "SSSE3: CPUID.01H:ECX.SSSE3[bit 9]", false);
    let has_sse41 = settings.add_bool("has_sse41", "SSE4.1: CPUID.01H:ECX.SSE4_1[bit 19]", false);
    let has_sse42 = settings.add_bool("has_sse42", "SSE4.2: CPUID.01H:ECX.SSE4_2[bit 20]", false);
    let has_popcnt = settings.add_bool("has_popcnt", "POPCNT: CPUID.01H:ECX.POPCNT[bit 23]", false);
    settings.add_bool("has_avx", "AVX: CPUID.01H:ECX.AVX[bit 28]", false);

    // CPUID.(EAX=07H, ECX=0H):EBX
    let has_bmi1 = settings.add_bool(
        "has_bmi1",
        "BMI1: CPUID.(EAX=07H, ECX=0H):EBX.BMI1[bit 3]",
        false,
    );
    let has_bmi2 = settings.add_bool(
        "has_bmi2",
        "BMI2: CPUID.(EAX=07H, ECX=0H):EBX.BMI2[bit 8]",
        false,
    );

    // CPUID.EAX=80000001H:ECX
    let has_lzcnt = settings.add_bool(
        "has_lzcnt",
        "LZCNT: CPUID.EAX=80000001H:ECX.LZCNT[bit 5]",
        false,
    );

    settings.add_predicate("use_sse41", predicate!(has_sse41));
    settings.add_predicate("use_sse42", predicate!(has_sse41 && has_sse42));
    settings.add_predicate("use_popcnt", predicate!(has_popcnt && has_sse42));
    settings.add_predicate("use_bmi1", predicate!(has_bmi1));
    settings.add_predicate("use_lznct", predicate!(has_lzcnt));

    settings.add_preset("baseline", preset!());
    let nehalem = settings.add_preset(
        "nehalem",
        preset!(has_sse3 && has_ssse3 && has_sse41 && has_sse42 && has_popcnt),
    );
    let haswell = settings.add_preset(
        "haswell",
        preset!(nehalem && has_bmi1 && has_bmi2 && has_lzcnt),
    );
    let broadwell = settings.add_preset("broadwell", preset!(haswell));
    let skylake = settings.add_preset("skylake", preset!(broadwell));
    let cannonlake = settings.add_preset("cannonlake", preset!(skylake));
    settings.add_preset("icelake", preset!(cannonlake));
    settings.add_preset(
        "znver1",
        preset!(
            has_sse3
                && has_ssse3
                && has_sse41
                && has_sse42
                && has_popcnt
                && has_bmi1
                && has_bmi2
                && has_lzcnt
        ),
    );

    settings.finish()
}

fn define_registers() -> IsaRegs {
    let mut regs = IsaRegsBuilder::new();

    let builder = RegBankBuilder::new("IntRegs", "r")
        .units(16)
        .names(vec!["rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi"])
        .track_pressure(true);
    let int_regs = regs.add_bank(builder);

    let builder = RegBankBuilder::new("FloatRegs", "xmm")
        .units(16)
        .track_pressure(true);
    let float_regs = regs.add_bank(builder);

    let builder = RegBankBuilder::new("FlagRegs", "")
        .units(1)
        .names(vec!["rflags"])
        .track_pressure(false);
    let flag_reg = regs.add_bank(builder);

    let builder = RegClassBuilder::new_toplevel("GPR", int_regs);
    let gpr = regs.add_class(builder);

    let builder = RegClassBuilder::new_toplevel("FPR", float_regs);
    let fpr = regs.add_class(builder);

    let builder = RegClassBuilder::new_toplevel("FLAG", flag_reg);
    regs.add_class(builder);

    let builder = RegClassBuilder::subclass_of("GPR8", gpr, 0, 8);
    let gpr8 = regs.add_class(builder);

    let builder = RegClassBuilder::subclass_of("ABCD", gpr8, 0, 4);
    regs.add_class(builder);

    let builder = RegClassBuilder::subclass_of("FPR8", fpr, 0, 8);
    regs.add_class(builder);

    regs.finish()
}

pub fn define(shared_defs: &mut SharedDefinitions) -> TargetIsa {
    let settings = define_settings(&shared_defs.settings);
    let regs = define_registers();

    let inst_group = instructions::define(&shared_defs.format_registry);
    legalize::define(shared_defs, &inst_group);

    // CPU modes for 32-bit and 64-bit operations.
    let mut x86_64 = CpuMode::new("I64");
    let mut x86_32 = CpuMode::new("I32");

    let expand_flags = shared_defs.transform_groups.by_name("expand_flags");
    let narrow = shared_defs.transform_groups.by_name("narrow");
    let widen = shared_defs.transform_groups.by_name("widen");
    let x86_expand = shared_defs.transform_groups.by_name("x86_expand");

    x86_32.legalize_monomorphic(expand_flags);
    x86_32.legalize_default(narrow);
    x86_32.legalize_type(B1, expand_flags);
    x86_32.legalize_type(I8, widen);
    x86_32.legalize_type(I16, widen);
    x86_32.legalize_type(I32, x86_expand);
    x86_32.legalize_type(F32, x86_expand);
    x86_32.legalize_type(F64, x86_expand);

    x86_64.legalize_monomorphic(expand_flags);
    x86_64.legalize_default(narrow);
    x86_64.legalize_type(B1, expand_flags);
    x86_64.legalize_type(I8, widen);
    x86_64.legalize_type(I16, widen);
    x86_64.legalize_type(I32, x86_expand);
    x86_64.legalize_type(I64, x86_expand);
    x86_64.legalize_type(F32, x86_expand);
    x86_64.legalize_type(F64, x86_expand);

    let cpu_modes = vec![x86_64, x86_32];

    TargetIsa::new("x86", inst_group, settings, regs, cpu_modes)
}
