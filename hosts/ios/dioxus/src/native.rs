use std::path::PathBuf;

fn bundle_file(name: &str) -> PathBuf {
    std::env::current_exe()
        .expect("iOS executable")
        .parent()
        .expect("iOS app bundle")
        .join(name)
}

#[no_mangle]
pub extern "C" fn orbital_native_ios_main() {
    let language = std::fs::read_to_string(bundle_file("language.json"))
        .expect("bundled language.json");
    orbital_core::language_data::install_language_bundle(&language)
        .expect("install language bundle");
    let program = std::fs::read_to_string(bundle_file("program.orb"))
        .expect("bundled program.orb");
    crate::native::start(&program, crate::styles::stylesheets(), "Native TODO")
        .expect("native TODO mount");
}
