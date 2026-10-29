//! Booting a program on the [`Host`]: register it, install its locales, configure tracing and credentials from the
//! environment, refuse a production boot that lacks what it needs.

use super::store::{DriverAdapter, PersistenceAdapter};
use super::{Host, HostConfig, SubstratePort};
use orbital_core::schema::parse_schema_file;
use std::path::Path;
use std::sync::Arc;

/// Each declared locale's merged catalog, written beside the resolved `.orb` as
/// `<stem>.<locale>.json` by `orb resolve` / `orb dev`. JS twin: `registerFromFile`.
pub fn read_catalogs(
    schema_path: &Path,
    locales: &[String],
) -> anyhow::Result<std::collections::HashMap<String, std::collections::HashMap<String, String>>> {
    let mut catalogs = std::collections::HashMap::new();
    for locale in locales {
        let file = schema_path.with_extension(format!("{locale}.json"));
        let raw = std::fs::read_to_string(&file).map_err(|e| {
            anyhow::anyhow!(
                "{} declares locale \"{locale}\" but its catalog {} could not be read: {e}",
                schema_path.display(),
                file.display()
            )
        })?;
        let catalog: std::collections::HashMap<String, String> = serde_json::from_str(&raw)
            .map_err(|e| {
                anyhow::anyhow!(
                    "catalog {} is not a flat JSON object of strings: {e}",
                    file.display()
                )
            })?;
        catalogs.insert(locale.clone(), catalog);
    }
    Ok(catalogs)
}

#[cfg(test)]
mod catalog_tests {
    use super::read_catalogs;

    fn dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "orbital-host-catalogs-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("site.orb"), "{}").unwrap();
        std::fs::write(d.join("site.en.json"), r#"{"site:hero.title":"Hello"}"#).unwrap();
        d
    }

    #[test]
    fn reads_each_declared_locales_sidecar() {
        let d = dir("ok");
        std::fs::write(d.join("site.ar.json"), r#"{"site:hero.title":"مرحبا"}"#).unwrap();
        let catalogs = read_catalogs(&d.join("site.orb"), &["en".into(), "ar".into()]).unwrap();
        assert_eq!(catalogs["ar"]["site:hero.title"], "مرحبا");
    }

    #[test]
    fn edge_a_missing_sidecar_names_the_file() {
        let d = dir("missing");
        let err = read_catalogs(&d.join("site.orb"), &["en".into(), "ar".into()]).unwrap_err();
        assert!(err.to_string().contains("site.ar.json"), "{err}");
    }
}

/// A resolved program's lazy orbitals read from the files `orb resolve` wrote beside it (EXEC-LOAD-1), with their catalogs.
pub struct FileLazySource {
    dir: std::path::PathBuf,
}

impl FileLazySource {
    pub fn beside(program: &Path) -> Self {
        Self { dir: program.parent().unwrap_or(Path::new("")).to_path_buf() }
    }
}

impl orbital_core::runtime::LazySource for FileLazySource {
    fn read(&self, file: &str) -> Result<orbital_core::runtime::LazyProgram, String> {
        let path = self.dir.join(file);
        let schema = parse_schema_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let locales = schema.locales.clone().unwrap_or_default();
        let catalogs = read_catalogs(&path, &locales).map_err(|e| e.to_string())?;
        Ok(orbital_core::runtime::LazyProgram { schema, catalogs })
    }
}

/// Tracing is an operator's choice, and the trace holds program data (every emit's payload, never a credential):
/// `ORBITAL_TRACE_FILE=<path>` traces and appends each dispatch to that file, `ORBITAL_TRACE=memory` keeps the last
/// dispatches in memory, `ORBITAL_TRACE=off` traces nothing; with neither set, `trace_by_default` decides.
fn configure_trace(
    runtime: &Host,
    env: &BootEnv,
    trace_by_default: bool,
) -> anyhow::Result<()> {
    if let Some(path) = env.get("ORBITAL_TRACE_FILE").filter(|p| !p.is_empty()) {
        runtime
            .set_trace_file(Path::new(path))
            .map_err(|e| anyhow::anyhow!("ORBITAL_TRACE_FILE {path}: {e}"))?;
        return Ok(());
    }
    let on = match env.get("ORBITAL_TRACE").map(String::as_str) {
        Some("memory") => true,
        Some("off") => false,
        Some(other) => anyhow::bail!("ORBITAL_TRACE={other}: expected memory or off"),
        None => trace_by_default,
    };
    runtime.set_trace(on);
    Ok(())
}

/// Parse and register the program at `schema_path` on the in-memory store, with the host's
/// substrate handler installed when one is given (`orb` answers `program/*` and `behavior/*` itself).
pub fn boot_file(
    schema_path: &Path,
    env: &BootEnv,
    trace_by_default: bool,
    substrate: Option<SubstratePort>,
) -> anyhow::Result<Arc<Host>> {
    boot_file_with(schema_path, env, trace_by_default, HostConfig { substrate, ..HostConfig::default() })
}

/// [`boot_file`] with the whole host configuration given: a pinned dispatch clock (`HostConfig::clock`) as well as the
/// substrate port.
pub fn boot_file_with(
    schema_path: &Path,
    env: &BootEnv,
    trace_by_default: bool,
    config: HostConfig,
) -> anyhow::Result<Arc<Host>> {
    let schema = parse_schema_file(schema_path)?;
    boot(schema, schema_path, Arc::new(DriverAdapter::memory()), env, trace_by_default, config, None)
}

/// A composed app booted with the substrate `port` installed and its ticks driven (`orb serve`, and a cli program's
/// `orb-run: server` hand-off): the store is the kernel's in-memory store. `catalogs` must hold every locale the app declares.
pub fn boot_app(
    schema: orbital_core::OrbitalSchema,
    schema_path: &Path,
    port: SubstratePort,
    catalogs: Catalogs,
    env: &BootEnv,
    trace_by_default: bool,
) -> anyhow::Result<Arc<Host>> {
    if let Some(missing) = schema.locales.iter().flatten().find(|locale| !catalogs.contains_key(*locale)) {
        anyhow::bail!("the composed app declares locale \"{missing}\" but no catalog was given for it");
    }
    let runtime = boot(
        schema,
        schema_path,
        Arc::new(DriverAdapter::memory()),
        env,
        trace_by_default,
        HostConfig { substrate: Some(port), ..HostConfig::default() },
        Some(catalogs),
    )?;
    release_boot_memory();
    start_tick_driver(&runtime);
    Ok(runtime)
}

/// A program a host embeds (the Node and Python kernel add-ons): parsed from `program_json`, on the kernel's in-memory
/// store, its message catalogs given, its ticks driven.
pub fn boot_embedded(
    program_json: &str,
    env: &BootEnv,
    catalogs: Catalogs,
) -> anyhow::Result<Arc<Host>> {
    let schema: orbital_core::OrbitalSchema =
        serde_json::from_str(program_json).map_err(|e| anyhow::anyhow!("parse the program: {e}"))?;
    if schema.lazy_pages.as_ref().is_some_and(|pages| !pages.is_empty()) {
        anyhow::bail!("the embedded program `{}` lists lazy pages, and an embedded program has no folder their files sit beside", schema.name);
    }
    let runtime = boot(
        schema,
        Path::new("program.orb"),
        Arc::new(DriverAdapter::memory()),
        env,
        false,
        HostConfig::default(),
        Some(catalogs),
    )?;
    release_boot_memory();
    start_tick_driver(&runtime);
    Ok(runtime)
}

/// Boot held the program in several transient forms; give their freed pages back so the process holds what it serves.
fn release_boot_memory() {
    let released = orbital_core::runtime::memory_release::release_freed_memory();
    tracing::info!(target: orbital_core::logging::ns::SERVER_DATA, ?released, "released boot memory");
}

/// The process environment a host boots from.
pub type BootEnv = std::collections::HashMap<String, String>;

/// Message catalogs per locale, as `i18n/t` reads them.
pub type Catalogs = std::collections::HashMap<String, std::collections::HashMap<String, String>>;

#[allow(clippy::too_many_arguments)]
pub fn boot(
    schema: orbital_core::OrbitalSchema,
    schema_path: &Path,
    persistence: Arc<dyn PersistenceAdapter>,
    env: &BootEnv,
    trace_by_default: bool,
    config: HostConfig,
    catalogs: Option<Catalogs>,
) -> anyhow::Result<Arc<Host>> {
    tracing::info!(
        target: orbital_core::logging::ns::SERVER_DATA,
        schema = %schema.name,
        version = schema.version.as_deref().unwrap_or("1.0.0"),
        path = %schema_path.display(),
        "loaded schema",
    );

    let runtime = Host::with_persistence(persistence, config).shared();
    runtime.set_lazy_source(Arc::new(FileLazySource::beside(schema_path)));

    configure_trace(&runtime, env, trace_by_default)?;
    // Register schema (materializes any authored `instances`).
    let name = schema.name.clone();
    runtime.register_schema(schema)?;
    let schema = runtime
        .registered_schema(&name)
        .ok_or_else(|| anyhow::anyhow!("program '{name}' is not registered after registering it"))?;
    if let Some(locales) = schema.locales.clone() {
        let catalogs = match catalogs {
            Some(given) => given,
            None => read_catalogs(schema_path, &locales)?,
        };
        runtime.set_messages(locales, catalogs);
    }
    let production = env.get("NODE_ENV").is_some_and(|v| v == "production");
    if let Some(error) = runtime.credential_store_failure() {
        if production {
            anyhow::bail!("refusing to start in production: the credential store could not be opened: {error}");
        }
        tracing::warn!(target: orbital_core::logging::ns::SERVER_DATA, %error, "credential store unavailable; the environment answers");
    }
    let problems = boot_env_problems(env, production);
    if production && !problems.is_empty() {
        anyhow::bail!("refusing to start in production:\n- {}", problems.join("\n- "));
    }
    for problem in &problems {
        tracing::warn!(target: orbital_core::logging::ns::SERVER_DATA, %problem, "deployment config");
    }
    let orbitals = runtime.list_orbitals();
    for orbital in &orbitals {
        tracing::debug!(
            target: orbital_core::logging::ns::SERVER_DATA,
            orbital = %orbital.name,
            entity = %orbital.entity_name,
            traits = orbital.trait_count,
            "registered orbital",
        );
    }
    tracing::info!(
        target: orbital_core::logging::ns::SERVER_DATA,
        orbitals = orbitals.len(),
        "schema registered",
    );
    Ok(runtime)
}

/// Run a tick pass about 60 times a second for as long as the process lives, as the JS host's scheduler does.
pub fn start_tick_driver(runtime: &Arc<Host>) {
    let runtime = Arc::clone(runtime);
    std::thread::Builder::new()
        .name("orbital-ticks".to_string())
        .spawn(move || loop {
            runtime.tick_pass(runtime.monotonic_ms(), orbital_core::Calendar::local_now());
            std::thread::sleep(std::time::Duration::from_millis(16));
        })
        .map(|_| ())
        .unwrap_or_else(|error| tracing::error!(target: orbital_core::logging::ns::SERVER_DATA, %error, "the tick driver did not start"));
}

/// What a deployment lacks: under `production` a Firestore emulator host (it serves no real data). The credentials a
/// program needs are `std-credential-admin`'s boot check.
pub fn boot_env_problems(env: &BootEnv, production: bool) -> Vec<String> {
    let mut problems = Vec::new();
    if production && env.get("FIRESTORE_EMULATOR_HOST").is_some_and(|v| !v.is_empty()) {
        problems.push("FIRESTORE_EMULATOR_HOST is set: the Firestore emulator is for local dev only".to_string());
    }
    problems
}

#[cfg(test)]
mod boot_env_tests {
    use super::boot_env_problems;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn production_names_the_emulator_and_local_dev_does_not() {
        assert_eq!(
            boot_env_problems(&env(&[("FIRESTORE_EMULATOR_HOST", "127.0.0.1:8080")]), true),
            ["FIRESTORE_EMULATOR_HOST is set: the Firestore emulator is for local dev only"]
        );
        assert!(boot_env_problems(&env(&[("FIRESTORE_EMULATOR_HOST", "h:1")]), false).is_empty());
        assert!(boot_env_problems(&env(&[]), true).is_empty());
    }
}

#[cfg(test)]
mod boot_tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> BootEnv {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn a_hook_answered_after_a_keyed_completion_reaches_a_caller_that_awaits_it() {
        let program =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/late-answer.orb");
        let runtime = boot_file(&program, &env(&[]), false, None).unwrap();
        let awaited = runtime.deliver_inbound_awaiting(
            "cli-echo",
            [],
            "{}",
            Some(std::time::Duration::from_secs(10)),
        );
        assert_eq!(
            (awaited.status, awaited.body.as_str()),
            (200, "{\"ok\":true}")
        );
        let plain = runtime.deliver_inbound("cli-echo", [], "{}");
        assert_eq!(
            plain.status, 500,
            "control: the delivering dispatch alone does not answer: {}",
            plain.body
        );
        assert!(plain.body.contains("inbound-unanswered"), "{}", plain.body);
        let unknown = runtime.deliver_inbound_awaiting(
            "nope",
            [],
            "{}",
            Some(std::time::Duration::from_secs(1)),
        );
        assert_eq!(unknown.status, 404, "an undeclared hook is refused at once");
    }

    fn program_file(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("orbital-host-boot-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app.orb");
        std::fs::write(&path, r#"{ "name": "app", "typeContractVersion": 1, "orbitals": [] }"#).unwrap();
        path
    }

    #[test]
    fn tracing_follows_the_operator_and_otherwise_the_entry_points_default() {
        let path = program_file("trace");
        assert!(
            boot_file(&path, &env(&[]), true, None)
                .unwrap()
                .tracing(),
            "the dev server traces by default"
        );
        assert!(
            !boot_file(&path, &env(&[]), false, None)
                .unwrap()
                .tracing(),
            "orb serve does not"
        );
        assert!(
            boot_file(&path, &env(&[("ORBITAL_TRACE", "memory")]), false, None)
                .unwrap()
                .tracing()
        );
        assert!(
            !boot_file(&path, &env(&[("ORBITAL_TRACE", "off")]), true, None)
                .unwrap()
                .tracing()
        );
        let file =
            std::env::temp_dir().join(format!("orbital-boot-trace-{}.jsonl", std::process::id()));
        let traced = boot_file(
            &path,
            &env(&[("ORBITAL_TRACE_FILE", file.to_str().unwrap())]),
            false,
        None,
        )
        .unwrap();
        assert_eq!(traced.trace_file_path().as_deref(), Some(file.as_path()));
        assert!(
            boot_file(&path, &env(&[("ORBITAL_TRACE", "always")]), false, None).is_err(),
            "edge: an unknown mode refuses to boot"
        );
        let _ = std::fs::remove_file(&file);
    }
}

#[cfg(test)]
mod composed_tests {
    use super::*;
    use orbital_core::effects::executor::{SubstrateFailure, SubstrateHandler};

    /// A `program/compose` that appends every program's orbitals into one app, as a stand-in for `orb`'s composer.
    struct Appending;

    impl SubstrateHandler for Appending {
        fn run(&self, op: &str, args: &[orbital_core::Value]) -> Result<orbital_core::Value, SubstrateFailure> {
            let refuse = |message: &str| SubstrateFailure { message: message.to_string(), errors: Vec::new() };
            if op != "program/compose" {
                return Err(refuse("only program/compose"));
            }
            let Some(orbital_core::Value::Array(programs)) = args.first() else { return Err(refuse("no list")) };
            let name = orbital_core::json_value::value_to_json(&args[1])["appName"].clone();
            let mut orbitals = Vec::new();
            let mut locales = Vec::new();
            for program in programs {
                let json = orbital_core::json_value::value_to_json(program);
                orbitals.extend(json["orbitals"].as_array().cloned().unwrap_or_default());
                locales.extend(json["locales"].as_array().cloned().unwrap_or_default());
            }
            let mut app = serde_json::json!({ "name": name, "typeContractVersion": 1, "orbitals": orbitals });
            if !locales.is_empty() {
                app["locales"] = serde_json::Value::Array(locales);
            }
            serde_json::from_value(app).map_err(|e| refuse(&e.to_string()))
        }
    }

    fn program(name: &str, orbital: &str, extra: serde_json::Value) -> orbital_core::Value {
        let mut json = serde_json::json!({
            "name": name,
            "typeContractVersion": 1,
            "orbitals": [{
                "name": orbital,
                "entity": { "name": format!("{orbital}Row"), "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
                "pages": [{ "name": format!("{orbital}Boot"), "path": "", "protocol": "boot", "traits": [{ "ref": format!("{orbital}Trait") }] }],
                "traits": [{ "name": format!("{orbital}Trait"), "linkedEntity": format!("{orbital}Row"),
                    "stateMachine": { "states": [{ "name": "idle", "isInitial": true }, { "name": "ready" }],
                        "events": [{ "key": "INIT", "name": "Init" }],
                        "transitions": [{ "from": "idle", "to": "ready", "event": "INIT" }] } }]
            }]
        });
        if let (Some(target), Some(more)) = (json.as_object_mut(), extra.as_object()) {
            target.extend(more.clone());
        }
        serde_json::from_value(json).unwrap()
    }

    fn schema_of(program: orbital_core::Value) -> orbital_core::OrbitalSchema {
        serde_json::from_value(orbital_core::json_value::value_to_json(&program)).unwrap()
    }

    fn port() -> SubstratePort {
        SubstratePort(Arc::new(Appending))
    }

    #[test]
    fn edge_a_declared_locale_without_its_catalog_refuses_the_boot() {
        let app = program("app", "Notes", serde_json::json!({ "locales": ["en", "ar"] }));
        let mut catalogs = Catalogs::new();
        catalogs.insert("en".to_string(), std::collections::HashMap::new());
        let error = boot_app(schema_of(app), Path::new("app.orb"), port(), catalogs, &Default::default(), false)
            .err()
            .expect("a missing catalog refuses the boot");
        assert!(error.to_string().contains("\"ar\""), "{error}");
    }

    fn persistent_program(ticks: serde_json::Value) -> orbital_core::Value {
        serde_json::from_value(serde_json::json!({
            "name": "desk",
            "typeContractVersion": 1,
            "orbitals": [{
                "name": "Desk",
                "entity": { "name": "Memo", "persistence": "persistent", "fields": [{ "name": "id", "type": "string" }, { "name": "n", "type": "number" }] },
                "traits": [{ "name": "Memos", "linkedEntity": "Memo",
                    "stateMachine": { "states": [{ "name": "idle", "isInitial": true }], "events": [{ "key": "INIT", "name": "Init" }],
                        "transitions": [{ "from": "idle", "to": "idle", "event": "INIT" }] },
                    "ticks": ticks }]
            }]
        }))
        .unwrap()
    }

    fn memos(host: &Host) -> Vec<serde_json::Value> {
        host.persistence().list("Memo").unwrap().iter().map(|r| serde_json::to_value(r).unwrap()).collect()
    }

    #[test]
    fn the_composed_boot_drives_its_background_ticks() {
        let ticks = serde_json::json!([{ "name": "beat", "interval": 10, "runsInBackground": true, "effects": [["persist", "create", "Memo", { "n": 1 }]] }]);
        let host = boot_app(schema_of(persistent_program(ticks)), Path::new("app.orb"), port(), Catalogs::new(), &Default::default(), true).unwrap();
        let first: orbital_core::Value = serde_json::from_value(serde_json::json!({ "id": "m0", "n": 0 })).unwrap();
        host.persistence().create("Memo", &first).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while memos(&host).len() < 2 {
            assert!(
                std::time::Instant::now() < deadline,
                "no tick ran: nothing drove the background tick; trace {}",
                serde_json::to_string(&host.trace()).unwrap()
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    fn lazy_files(tag: &str, with_lazy_file: bool) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("orbital-host-lazy-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("lazy")).unwrap();
        let orbital = |name: &str, entity: &str, trait_def: serde_json::Value| serde_json::json!({
            "name": name,
            "entity": { "name": entity, "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
            "traits": [trait_def]
        });
        let emitter = serde_json::json!({ "name": "Emitter", "linkedEntity": "Note",
            "stateMachine": { "states": [{ "name": "idle", "isInitial": true }],
                "transitions": [{ "from": "idle", "to": "idle", "event": "GO", "effects": [["emit", "PING", {}]] }] },
            "emits": [{ "event": "PING" }] });
        let hear = serde_json::json!({ "name": "Hear", "linkedEntity": "Memo",
            "stateMachine": { "states": [{ "name": "idle", "isInitial": true }, { "name": "heard" }],
                "transitions": [{ "from": "idle", "to": "heard", "event": "HEARD" }] },
            "listens": [{ "event": "PING", "triggers": "HEARD" }] });
        let main = serde_json::json!({
            "name": "app", "typeContractVersion": 1,
            "orbitals": [orbital("Main", "Note", emitter)],
            "lazyPages": [{ "path": "/late", "orbital": "Late", "orbRef": "lazy/Late.orb",
                "traits": [{ "name": "Hear", "dispatchMode": "persistedAwaited", "listens": [{ "event": "PING", "triggers": "HEARD" }] }] }]
        });
        std::fs::write(dir.join("app.orb"), main.to_string()).unwrap();
        if with_lazy_file {
            let late = serde_json::json!({
                "name": "Late", "typeContractVersion": 1,
                "orbitals": [orbital("Late", "Memo", hear)]
            });
            std::fs::write(dir.join("lazy/Late.orb"), late.to_string()).unwrap();
        }
        dir
    }

    #[test]
    fn a_booted_program_loads_its_lazy_orbital_from_the_file_beside_it_on_the_first_event() {
        let dir = lazy_files("file", true);
        let host = super::boot_file_with(&dir.join("app.orb"), &Default::default(), false, HostConfig::default()).unwrap();
        assert_eq!(host.kernel().lazy_pending(), ["Late"], "boot registers a stub, not the orbital");
        host.kernel().dispatch("Emitter", None, "GO", orbital_core::Value::Null).unwrap();
        assert_eq!(host.kernel().get_state("Hear", None).unwrap().current_state, "heard");
        assert!(host.kernel().lazy_pending().is_empty());
    }

    #[test]
    fn the_composed_boot_loads_its_lazy_orbital_beside_the_app_file() {
        let dir = lazy_files("app", true);
        let app = dir.join("app.orb");
        let schema = orbital_core::schema::parse_schema_file(&app).unwrap();
        let host = boot_app(schema, &app, port(), Catalogs::new(), &Default::default(), false).unwrap();
        host.kernel().dispatch("Hear", None, "HEARD", orbital_core::Value::Null).unwrap();
        assert_eq!(host.kernel().get_state("Hear", None).unwrap().current_state, "heard");
    }

    #[test]
    fn edge_a_missing_lazy_file_fails_the_event_that_reaches_it_and_names_the_file() {
        let dir = lazy_files("missing", false);
        let host = super::boot_file_with(&dir.join("app.orb"), &Default::default(), false, HostConfig::default()).unwrap();
        let error = host.kernel().dispatch("Emitter", None, "GO", orbital_core::Value::Null).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("Late") && message.contains("Late.orb"), "{message}");
    }

    #[test]
    fn edge_an_embedded_program_that_lists_lazy_pages_is_refused() {
        let dir = lazy_files("embedded", true);
        let json = std::fs::read_to_string(dir.join("app.orb")).unwrap();
        let error = boot_embedded(&json, &Default::default(), Catalogs::new()).err().expect("refused");
        assert!(error.to_string().contains("lazy pages"), "{error}");
    }
}
