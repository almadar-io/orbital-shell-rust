//! Coverage REPORT for the Dioxus client against the installed `@almadar/core`
//! registry (`docs/Almadar_Behavior_Packages.md` §14, 5d). Not a gate: a
//! pattern this client lacks renders the declared unsupported-pattern error,
//! so a new pattern in core never reds this crate. The report is printed so a
//! run shows exactly what is declared unsupported.

use orbital_client_dioxus::registry::{default_registry, RENDERER_ID, STUBBED_PATTERNS};
use orbital_core::renderer_coverage::unsupported_patterns;

fn installed_patterns() -> Vec<String> {
    let registry: serde_json::Value = serde_json::from_str(
        &orbital_core::language_data::language_data()
            .unwrap_or_else(|e| panic!("{}: {e}", e.code()))
            .patterns,
    )
    .unwrap();
    registry["patterns"]
        .as_object()
        .expect("patterns object")
        .keys()
        .cloned()
        .collect()
}

#[test]
fn coverage_report_against_the_installed_registry() {
    let registry = default_registry();
    let patterns = installed_patterns();
    assert!(!patterns.is_empty(), "installed registry has no patterns");
    let unsupported = unsupported_patterns(patterns.iter().map(String::as_str), |p| {
        registry.supports(p)
    });
    eprintln!(
        "{RENDERER_ID} {}: {} of {} registry patterns declared unsupported: {unsupported:?}",
        env!("CARGO_PKG_VERSION"),
        unsupported.len(),
        patterns.len()
    );
    for p in &unsupported {
        assert_eq!(registry.get_renderer(p).pattern_name(), "*", "{p}");
    }
}

#[test]
fn the_coverage_list_is_what_the_registry_answers() {
    let registry = default_registry();
    let coverage = registry.coverage();
    assert!(!coverage.is_empty());
    assert!(coverage.windows(2).all(|w| w[0] < w[1]), "sorted, unique");
    assert!(coverage.iter().all(|p| registry.supports(p)));
    assert!(!registry.supports("zz-not-a-pattern"));
    assert_eq!(
        registry.get_renderer("zz-not-a-pattern").pattern_name(),
        "*"
    );
}

/// The native desktop registry (the game overrides register only there) — the one
/// `orb-desktop` probes with.
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
#[test]
fn the_declared_stubs_are_exactly_the_unsupported_patterns() {
    let registry = default_registry();
    let patterns = installed_patterns();
    let mut unsupported = unsupported_patterns(patterns.iter().map(String::as_str), |p| registry.supports(p));
    unsupported.sort();
    let mut stubs: Vec<String> = STUBBED_PATTERNS.iter().map(|s| s.to_string()).collect();
    stubs.sort();
    assert_eq!(unsupported, stubs, "declare a new gap in STUBBED_PATTERNS, or drop an implemented one");
    assert!(stubs.iter().all(|p| registry.is_stub(p)));
    assert!(!registry.is_stub("typography"), "control: a supported pattern is no stub");
}
