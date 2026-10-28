//! Booting a program on the [`Host`]: register it, install its locales, configure tracing, sign-in, credentials and the
//! tick invoker from the environment, refuse a production boot that lacks what it needs.

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

/// Tracing is an operator's choice, and the trace holds program data (every emit's payload, never a credential):
/// `ORBITAL_TRACE_FILE=<path>` traces and appends each dispatch to that file, `ORBITAL_TRACE=memory` keeps the last
/// dispatches in memory, `ORBITAL_TRACE=off` traces nothing; with neither set, `trace_by_default` decides.
fn configure_trace(
    runtime: &Host,
    env: &orbital_core::runtime::auth::AuthEnv,
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
    env: &orbital_core::runtime::auth::AuthEnv,
    trace_by_default: bool,
    substrate: Option<SubstratePort>,
) -> anyhow::Result<Arc<Host>> {
    boot_file_with(schema_path, env, trace_by_default, HostConfig { substrate, ..HostConfig::default() })
}

/// [`boot_file`] with the whole host configuration given: a pinned dispatch clock (`HostConfig::clock`) as well as the
/// substrate port.
pub fn boot_file_with(
    schema_path: &Path,
    env: &orbital_core::runtime::auth::AuthEnv,
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
    port: SubstratePort,
    catalogs: Catalogs,
    env: &orbital_core::runtime::auth::AuthEnv,
    trace_by_default: bool,
) -> anyhow::Result<Arc<Host>> {
    if let Some(missing) = schema.locales.iter().flatten().find(|locale| !catalogs.contains_key(*locale)) {
        anyhow::bail!("the composed app declares locale \"{missing}\" but no catalog was given for it");
    }
    let path = format!("{}.orb", schema.name);
    let runtime = boot(
        schema,
        Path::new(&path),
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
    env: &orbital_core::runtime::auth::AuthEnv,
    catalogs: Catalogs,
) -> anyhow::Result<Arc<Host>> {
    let schema: orbital_core::OrbitalSchema =
        serde_json::from_str(program_json).map_err(|e| anyhow::anyhow!("parse the program: {e}"))?;
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

/// Message catalogs per locale, as `i18n/t` reads them.
pub type Catalogs = std::collections::HashMap<String, std::collections::HashMap<String, String>>;

#[allow(clippy::too_many_arguments)]
pub fn boot(
    schema: orbital_core::OrbitalSchema,
    schema_path: &Path,
    persistence: Arc<dyn PersistenceAdapter>,
    env: &orbital_core::runtime::auth::AuthEnv,
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
    configure_sign_in(&runtime, &schema, env)?;
    let production = env.get("NODE_ENV").is_some_and(|v| v == "production");
    if let Some(error) = runtime.credential_store_failure() {
        if production {
            anyhow::bail!("refusing to start in production: the credential store could not be opened: {error}");
        }
        tracing::warn!(target: orbital_core::logging::ns::SERVER_DATA, %error, "credential store unavailable; the environment answers");
    }
    let secrets = runtime.credentials();
    let invoker = orbital_core::runtime::auth::TickInvoker::from_env(
        env,
        &orbital_core::runtime::native_transport::feed_host,
        secrets.clone(),
        Arc::new(orbital_core::time_util::now_millis),
    )
    .map_err(anyhow::Error::msg)?;
    runtime.set_tick_invoker(invoker);
    let problems = boot_env_problems(
        env,
        production,
        &orbital_core::credential_admin::schema_invoked_services(&schema),
        &|| runtime.credential_catalog(),
        &|name| matches!(orbital_core::runtime::http_service::SecretSource::secret(secrets.as_ref(), name), Ok(Some(_))),
    );
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

/// What a deployment lacks beyond its sign-in (which [`configure_sign_in`] refuses itself): under `production` a
/// Firestore emulator host (it serves no real data), and, unless integrations are mocked
/// (`ALMADAR_INTEGRATIONS_MODE=mock` / `USE_MOCK_DATA=true`), every required credential of a service the program calls
/// that `present` (the credential store, then the environment) does not hold.
pub fn boot_env_problems(
    env: &orbital_core::runtime::auth::AuthEnv,
    production: bool,
    called: &std::collections::BTreeSet<String>,
    catalog: &dyn Fn() -> Result<orbital_core::credential_admin::Catalog, String>,
    present: &dyn Fn(&str) -> bool,
) -> Vec<String> {
    let mut problems = Vec::new();
    if production && env.get("FIRESTORE_EMULATOR_HOST").is_some_and(|v| !v.is_empty()) {
        problems.push("FIRESTORE_EMULATOR_HOST is set: the Firestore emulator is for local dev only".to_string());
    }
    let mocked = orbital_core::credential_admin::env_mock(&|k| env.get(k).cloned());
    if called.is_empty() || mocked {
        return problems;
    }
    match catalog() {
        Ok(catalog) => problems.extend(
            orbital_core::credential_admin::missing_required_credentials(&catalog, called, present)
                .into_iter()
                .map(|missing| format!("missing required integration credential {missing}")),
        ),
        Err(error) => problems.push(format!("the declared integration credentials could not be read: {error}")),
    }
    problems
}

/// Sign-in from the environment (`AUTH_PROVIDER`, `FIREBASE_PROJECT_ID`, `OIDC_ISSUER_URL`, `OIDC_AUDIENCE`, and
/// `ALMADAR_API_KEY_SALT` with `ALMADAR_API_KEY_ENTITY` for API keys). Under `NODE_ENV=production` the declaration is required
/// and any problem in it stops the boot; elsewhere a server with none declared stays open, and a declaration with problems
/// only warns.
pub fn configure_sign_in(
    runtime: &Arc<Host>,
    schema: &orbital_core::OrbitalSchema,
    env: &orbital_core::runtime::auth::AuthEnv,
) -> anyhow::Result<()> {
    use orbital_core::runtime::auth::{sign_in_from_env, Clock, RowFinder};
    let clock: Clock = Arc::new(orbital_core::time_util::now_millis);
    let persistence = runtime.persistence().clone();
    let find: RowFinder = Arc::new(move |entity: &str, id: &str| {
        persistence
            .find(entity, id)
            .map(|row| row.and_then(|row| serde_json::to_value(row).ok()))
    });
    let sign_in = sign_in_from_env(
        env,
        schema,
        &orbital_core::runtime::native_transport::feed_host,
        runtime.credentials(),
        clock,
        find,
    )
    .map_err(anyhow::Error::msg)?;
    if let Some(sign_in) = sign_in {
        runtime.set_sign_in(sign_in);
    }
    Ok(())
}

#[cfg(test)]
mod boot_env_tests {
    use super::boot_env_problems;
    use orbital_core::credential_admin::Catalog;
    use std::collections::{BTreeSet, HashMap};

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    fn catalog() -> Result<Catalog, String> {
        Catalog::from_registry(
            r#"{ "services": { "stripe": { "backend": "production", "credentials": [
                { "envVar": "STRIPE_SECRET_KEY", "required": true, "description": "API key" }] } } }"#,
        )
    }

    fn stripe() -> BTreeSet<String> {
        BTreeSet::from(["stripe".to_string()])
    }

    #[test]
    fn production_names_the_emulator_and_each_missing_required_credential() {
        let problems = boot_env_problems(&env(&[("FIRESTORE_EMULATOR_HOST", "127.0.0.1:8080")]), true, &stripe(), &catalog, &|_| false);
        assert_eq!(
            problems,
            [
                "FIRESTORE_EMULATOR_HOST is set: the Firestore emulator is for local dev only",
                "missing required integration credential stripe: STRIPE_SECRET_KEY (API key)",
            ]
        );
    }

    #[test]
    fn control_held_credentials_and_local_dev_emulators_are_no_problem() {
        assert!(boot_env_problems(&env(&[]), true, &stripe(), &catalog, &|_| true).is_empty());
        assert!(boot_env_problems(&env(&[("FIRESTORE_EMULATOR_HOST", "h:1")]), false, &BTreeSet::new(), &catalog, &|_| false).is_empty());
    }

    #[test]
    fn mocked_integrations_owe_no_credentials_and_an_unreadable_catalog_is_named() {
        for pairs in [vec![("ALMADAR_INTEGRATIONS_MODE", "mock")], vec![("USE_MOCK_DATA", "true")]] {
            assert!(boot_env_problems(&env(&pairs), true, &stripe(), &catalog, &|_| false).is_empty(), "{pairs:?}");
        }
        let unreadable = boot_env_problems(&env(&[]), true, &stripe(), &|| Err("no registry".into()), &|_| false);
        assert_eq!(unreadable, ["the declared integration credentials could not be read: no registry"]);
        assert!(
            boot_env_problems(&env(&[]), true, &BTreeSet::new(), &|| Err("no registry".into()), &|_| false).is_empty(),
            "control: a program calling no service never reads the catalog"
        );
    }
}

#[cfg(test)]
mod auth_boot_tests {
    use super::*;

    fn runtime() -> Arc<Host> {
        Arc::new(Host::new(Arc::new(DriverAdapter::memory())))
    }

    fn env(pairs: &[(&str, &str)]) -> orbital_core::runtime::auth::AuthEnv {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn empty() -> orbital_core::OrbitalSchema {
        serde_json::from_value(serde_json::json!({ "name": "app", "typeContractVersion": 1, "orbitals": [] })).unwrap()
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

    #[test]
    fn production_refuses_an_emulator_environment_and_a_missing_declaration() {
        let production = [
            ("NODE_ENV", "production"),
            ("FIREBASE_PROJECT_ID", "prod-app"),
        ];
        assert!(configure_sign_in(&runtime(), &empty(), &env(&production)).is_ok());
        let with_emulator = [
            ("NODE_ENV", "production"),
            ("FIREBASE_PROJECT_ID", "prod-app"),
            ("FIREBASE_AUTH_EMULATOR_HOST", "localhost:9099"),
        ];
        let error = configure_sign_in(&runtime(), &empty(), &env(&with_emulator))
            .unwrap_err()
            .to_string();
        assert!(error.contains("FIREBASE_AUTH_EMULATOR_HOST"), "{error}");
        let nothing = [("NODE_ENV", "production")];
        assert!(configure_sign_in(&runtime(), &empty(), &env(&nothing))
            .unwrap_err()
            .to_string()
            .contains("FIREBASE_PROJECT_ID"));
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
    fn booting_a_program_file_configures_sign_in_from_the_environment() {
        let path = program_file("auth");
        let error = boot_file(&path, &env(&[("NODE_ENV", "production")]), false, None)
            .err()
            .expect("production without a declaration refuses to boot");
        assert!(error.to_string().contains("FIREBASE_PROJECT_ID"), "{error}");
        let signed_in = boot_file(
            &path,
            &env(&[
                ("NODE_ENV", "production"),
                ("FIREBASE_PROJECT_ID", "prod-app"),
            ]),
            false,
        None,
        )
        .unwrap();
        assert!(
            signed_in.sign_in().is_some(),
            "a declared provider is configured by the boot"
        );
        let open = boot_file(&path, &env(&[]), false, None).unwrap();
        assert!(
            open.sign_in().is_none(),
            "control: nothing declared stays open"
        );
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

    #[test]
    fn control_a_dev_server_may_use_the_emulator_and_one_with_nothing_declared_stays_open() {
        let dev = [
            ("FIREBASE_PROJECT_ID", "demo-app"),
            ("FIREBASE_AUTH_EMULATOR_HOST", "localhost:9099"),
        ];
        let open = runtime();
        configure_sign_in(&open, &empty(), &env(&dev)).unwrap();
        assert!(open.sign_in().is_some());
        let none = runtime();
        configure_sign_in(&none, &empty(), &env(&[])).unwrap();
        assert!(none.sign_in().is_none());
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
        let error = boot_app(schema_of(app), port(), catalogs, &Default::default(), false)
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
        let host = boot_app(schema_of(persistent_program(ticks)), port(), Catalogs::new(), &Default::default(), true).unwrap();
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
}
