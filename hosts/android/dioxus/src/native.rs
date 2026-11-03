use std::ffi::CString;
use std::io::Read;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case", deny_unknown_fields)]
enum NativeHost {
    Embedded,
    Server { #[serde(rename = "backendUrl")] backend_url: String },
}

fn read_asset(app: &android_activity::AndroidApp, name: &str) -> String {
    let name = CString::new(name).expect("asset name");
    let mut asset = app.asset_manager().open(&name).expect("native GPU asset");
    let mut value = String::new();
    asset.read_to_string(&mut value).expect("UTF-8 native GPU asset");
    value
}

#[cfg(feature = "native-a11y")]
#[no_mangle]
pub extern "C" fn orbital_native_gpu_accessibility_v1() -> u32 {
    1
}

#[no_mangle]
pub extern "system" fn Java_com_google_androidgamesdk_OrbitalTextInput_nativeIsEditableAt(
    _env: *mut std::ffi::c_void,
    _class: *mut std::ffi::c_void,
    x: f32,
    y: f32,
) -> u8 {
    blitz_shell::android_is_editable_at(x, y) as u8
}

#[no_mangle]
pub fn android_main(app: android_activity::AndroidApp) {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init();
    let language = read_asset(&app, "language.json");
    orbital_core::language_data::install_language_bundle(&language)
        .expect("install language bundle");
    let schema = read_asset(&app, "program.orb");
    let host: NativeHost = serde_json::from_str(&read_asset(&app, "native-host.json"))
        .expect("native-host.json contract");
    blitz_shell::set_android_app(app);
    match host {
        NativeHost::Embedded => crate::native::start(&schema, crate::styles::stylesheets(), "Native App"),
        NativeHost::Server { backend_url } => crate::native::start_server(&schema, &backend_url, crate::styles::stylesheets(), "Native App"),
    }
    .expect("native GPU mount");
}
