//! Built-in composition belongs to the application, not the extension APIs.
pub fn registry() -> whale_toolchain::Registry {
    let mut registry = whale_toolchain::Registry::new();
    registry
        .register_target(ir::Target::X86_64WhaleLinux.specification())
        .expect("valid built-in target");
    registry.register_assembler(whale_assembler_amd64::AMD64).expect("unique built-in assembler");
    registry
}
