#[cfg(test)]
mod tests {
    use x86_oracle::{BochsOracle, SailOracle, StepOutcome, X86Oracle, DEFAULT_CODE_ADDR, RAX, RBX};

    /// add rax, rbx
    const X86_ADD_RAX_RBX: &[u8] = &[0x48, 0x01, 0xD8];
    /// add x3, x1, x2
    const RV_ADD_X3_X1_X2: u32 = 0x0020_81B3;

    fn x86_add(cpu: &mut dyn X86Oracle) {
        cpu.set_rip(DEFAULT_CODE_ADDR);
        cpu.set_gpr(RAX, 5);
        cpu.set_gpr(RBX, 7);
        assert_eq!(cpu.step_bytes(X86_ADD_RAX_RBX), StepOutcome::Retired, "{}", cpu.backend_name());
        assert_eq!(cpu.get_gpr(RAX), 12, "{}", cpu.backend_name());
        assert_eq!(cpu.get_rip(), DEFAULT_CODE_ADDR + 3, "{}", cpu.backend_name());
    }

    #[test]
    fn x86_and_riscv_in_one_binary() {
        let mut sail = SailOracle::new();
        x86_add(&mut sail);
        let mut bochs = BochsOracle::new();
        x86_add(&mut bochs);

        let mut rv = sail_riscv_oracle::Oracle::new();
        rv.set_x(1, 3);
        rv.set_x(2, 4);
        assert!(rv.step(RV_ADD_X3_X1_X2).retired);
        assert_eq!(rv.get_x(3), 7);

        // Interleave once more: each model must still see only its own runtime state.
        x86_add(&mut sail);
        assert!(rv.step(RV_ADD_X3_X1_X2).retired);
        assert_eq!(rv.get_x(3), 7);
        assert_eq!(rv.get_pc(), sail_riscv_oracle::DEFAULT_RESET_PC + 8);
        x86_add(&mut bochs);
    }
}
