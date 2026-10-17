# Extending Whale

Whale separates reusable contracts from target implementations. Extensions are
ordinary Rust crates, linked explicitly by the application. There is no global
registry, automatic discovery, or stable dynamic-library ABI.

## Crate boundaries

| Crate | Responsibility | Depends on target implementations? |
| --- | --- | --- |
| `whale-diagnostic` (`diagnostic/`) | Structured diagnostics | No |
| `whale-target` (`target-spec/`) | Open target descriptions, storage profiles, output and memory models | No |
| `assembler` | Assembly output, limits, diagnostics, source-level extension trait; optional shared syntax utilities | No |
| `whale-assembler-amd64` (`assembler-amd64/`) | AMD64 parser, instruction tables and encoder | Its own ISA only |
| `ir` | Typed IR, verification, layout and reference interpreter | No assembler/backend dependency |
| `whale-backend-api` (`backend-api/`) | Requirement classification, checked input and artifact emission | No |
| `whale-toolchain` (`toolchain/`) | Explicit registry and compile/assemble dispatch | No |
| `whale` | CLI and built-in composition in `src/builtins.rs` | Yes |
| `whale-example-target` (`examples/external-target/`) | Independent toy assembler and executable bytecode backend | No built-in targets |

The existing `object`, `linker`, and `reverse` crates remain separate native tools.
Their ELF/AMD64 implementation contracts are not generalized by this change.
Registering a new target does not claim that those tools, the AST frontend, or the
reference interpreter support it.

## Add an assembler

Implement `assembler::extension::Assembler` in your own crate:

```rust,ignore
impl assembler::extension::Assembler for VendorAssembler {
    fn id(&self) -> &str { "vendor.cpu" }
    fn assemble(
        &self,
        source_name: &str,
        source: &str,
        limits: &assembler::AssemblyLimits,
    ) -> Result<assembler::AssemblerOutput, assembler::AsmError> {
        // Parse your syntax and encode your instructions.
        // Check output limits BEFORE allocation; preserve source diagnostics.
        todo!()
    }
}
```

The implementation owns its grammar, tokens and AST. `assembler::tokens`,
`assembler::ast` and `traits::ISA` are optional shared-syntax utilities, not a
mandatory representation for external ISAs. `RelocKind::TargetSpecific` carries a
namespace and code understood by the target's own object/link adapter. The built-in
AMD64 ELF adapter explicitly rejects such relocations rather than guessing.

Applications compose extensions explicitly:

```rust,ignore
let mut registry = whale_toolchain::Registry::new();
registry.register_assembler(VendorAssembler)?;
let output = registry.assemble("vendor.cpu", "input.asm", source, &limits)?;
```

Assembly registration is independent of IR backend registration. An extension can
provide only an assembler, only a backend, or both. Duplicate IDs fail without
replacing the existing implementation. Registries are independent and deterministic.
Implementations are trusted in-process Rust code, not a sandbox.

## Add an IR target/backend

1. Construct a `whale_target::TargetSpec` with a namespaced ID, `OutputKind`,
   `MemoryModel`, and `StorageLayout`.
2. Register the specification with `Registry::register_target`.
3. Implement `whale_backend_api::Backend` and register it with `register_backend`.
4. Build IR using the same target ID and data layout.
5. Call `Registry::compile`; receive named binary artifacts in memory.

The compile path is:

- Validate the backend ID and supported IR format/semantics versions.
- Verify the entire module against the supplied target contract.
- Query support for types, declarations, instructions, terminators and globals.
- Collect required runtime identities and report unsupported requirements with
  function/block/instruction context.
- Run backend-specific validation/legalization availability checks.
- Give the backend a `CheckedModule` and emit artifacts.
- Reject empty artifact sets, duplicate names, absolute/traversal paths and
  nonportable path spellings. File publication belongs to the application.

`Support::{Direct, Expanded, Runtime, Unsupported}` describes how each operation is
implemented. A backend must inspect complete type and instruction semantics,
including nested types, calling conventions, overflow, pointer provenance,
initialization, lifetime and traps. A runtime declaration is a dependency identity,
not an automatic installer: validate availability, emit the helper, or declare a
host-provided dependency. `Backend::validate` is the place to enforce that policy.
The API does not magically make an unsupported instruction legal.

`StorageLayout` describes the current natural-layout family: 32/64-bit pointers,
scalar alignments, naturally padded aggregates and an optional large-array
placement rule. It does not describe every possible ABI (for example packed or
scalable types). Extend the layout contract explicitly when another layout family
is needed; do not inherit the host or AMD64 layout silently. External verification
uses `verify_module_for_target`; `verify_module` retains strict built-in lookup.
`Target::SUPPORTED` is the built-in native catalog, not the extension registry.

The IR text and operation semantics versions remain 7 and 1: this changes host
APIs and target selection, not the serialized grammar or arithmetic contracts.

## Executable example

`examples/external-target` has no dependency on the AMD64 implementation. Its own
`value 42` assembler grammar emits a constant and return. Its backend translates a
single parameterless i64 function containing constants/addition/return into actual
binary instructions. Tests independently execute the result, including wrapping
`i64::MAX + 1`, and reject unsupported instructions and mismatched layouts.
This toy encoding is NOT the future official Whale bytecode format or VM.

```sh
cargo test -p whale-example-target --locked
cargo test -p ir --test extension_layout --locked
cargo test -p whale-assembler-amd64 --locked
cargo test --workspace --all-features --locked
```

To make an out-of-tree extension, use path/git dependencies on the API crates and
copy the example's registration pattern. Replace `rust-version.workspace = true`
with an explicit Rust version when using the example outside this workspace.
No edit to Whale's target enum or core registry is needed. A custom application
links the extension and calls its registration function. The stock CLI composes
only built-ins; it does not discover external crates at runtime.

## Breaking import migration

- `assembler::isa::AMD64` becomes `whale_assembler_amd64::AMD64`.
- `assembler::isa::amd64::{parser,encoder,encoding,tables}` moves to
  `whale_assembler_amd64::{parser,encoder,encoding,tables}`.
- Depend on `whale-assembler-amd64` wherever an AMD64 implementation is needed.
- Assembly fixtures/tests now belong to `assembler-amd64/`.

## Further modules

The same boundary can later separate object writers, relocation appliers,
linkers, VM execution engines and frontend imports. JVM, CIL, native IR codegen
and the official Whale bytecode VM are not implemented in this refactor.
A dynamic plugin layer can wrap these contracts using a versioned C ABI or
process protocol; do not pass Rust trait objects across an unstable dylib ABI.
