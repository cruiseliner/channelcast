fn main() {
    // Procedural UniFFI macros embed metadata in the library; no UDL scaffolding step.
    // Keep build.rs so we can add scaffolding later if needed.
    println!("cargo:rerun-if-changed=src/lib.rs");
}
