// The CMake engine supplies the remaining native globals and primitives.
// Cargo's standalone tests must not export adapters which require that host;
// a Cargo feature cannot express this distinction because of --all-features.
fn main() {
    println!("cargo:rustc-check-cfg=cfg(espeak_native_host)");
    println!("cargo:rerun-if-env-changed=ESPEAK_NATIVE_HOST");
    match std::env::var("ESPEAK_NATIVE_HOST") {
        Ok(value) if value == "1" => println!("cargo:rustc-cfg=espeak_native_host"),
        Err(std::env::VarError::NotPresent) => {}
        _ => panic!("ESPEAK_NATIVE_HOST must be unset or 1 (the CMake engine host)"),
    }
}
