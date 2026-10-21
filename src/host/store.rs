//! Kernel store drivers as the server's persistence: a driver is an `EntitySource` over raw ports (sockets, HTTP),
//! so no host process stands between the kernel and the data. [`DriverAdapter`] presents one as a
//! [`PersistenceAdapter`], and [`persistence_from_env`] picks the driver the way `@almadar/db`'s
//! `persistenceSpecFromEnv` does: `DATA_BACKEND` (`mock` | `firebase` | `postgres` | `couchdb`, default `firebase`),
//! the `USE_MOCK_DATA` flag, `DATABASE_URL`, `PGPOOL_MAX`, `COUCHDB_URL` and the Firebase variables.
//!
//! [`PersistenceAdapter`] is the interface every store backend presents to the host.

use orbital_core::runtime::couchdb::CouchRows;
use orbital_core::runtime::entity_source::{EntitySource, InMemoryEntityCache};
use orbital_core::runtime::firestore::FirestoreRows;
use orbital_core::runtime::http_service::Transport;
use orbital_core::runtime::pg_rows::PostgresRows;
use orbital_core::runtime::row_query::{
    QueryCapabilities, RowIdPage, RowPage, RowQuery, RowWriteBatch, RowWriteOp, RowWriteOutcome,
    WriteCapabilities,
};
use orbital_core::runtime::store_config::driver_store;
use orbital_core::{PersistenceHandler, Value};
use std::collections::HashMap;
use std::sync::Arc;


/// Server storage operations; deliberately not a `FetchHandler` — reads go through the kernel's `@read`-enforcing owner.
pub trait PersistenceAdapter: PersistenceHandler + Send + Sync {
    /// List entities with filtering
    fn list_filtered(
        &self,
        entity_type: &str,
        filter: &HashMap<String, Value>,
    ) -> Result<Vec<Value>, String>;

    /// Count entities of a given type
    fn count(&self, entity_type: &str) -> Result<usize, String>;

    /// Count entities with filtering
    fn count_filtered(
        &self,
        entity_type: &str,
        filter: &HashMap<String, Value>,
    ) -> Result<usize, String>;

    /// Check if an entity exists
    fn exists(&self, entity_type: &str, id: &str) -> Result<bool, String>;

    /// Get multiple entities by IDs
    fn get_many(&self, entity_type: &str, ids: &[&str]) -> Result<Vec<Value>, String>;

    /// Delete all entities of a given type
    fn delete_all(&self, entity_type: &str) -> Result<usize, String>;

    /// Get entity types (collections)
    fn entity_types(&self) -> Vec<String>;

    /// Whether rows outlive the process (a database, not process memory): where the encrypted credential rows are kept.
    fn durable(&self) -> bool {
        false
    }

    /// Bind `entity_type` to its declared `[persistent: tbl]` collection; a store that keys its rows by the
    /// entity name alone ignores it.
    fn bind_collection(&self, _entity_type: &str, _collection: &str) {}

    /// What this store's `query_page` / `query_ids` honour; `None` = not declared, the kernel lists and
    /// queries itself.
    fn query_capabilities(&self) -> Option<QueryCapabilities> {
        None
    }

    /// What this store's `write_where` / `write_batch` (from [`PersistenceHandler`]) honour; `None` = not declared.
    fn write_capabilities(&self) -> Option<WriteCapabilities> {
        None
    }

    /// The page of rows matching `query`, run by the store. `None` = this store cannot.
    fn query_page(&self, _entity_type: &str, _query: &RowQuery) -> Option<Result<RowPage, String>> {
        None
    }

    /// The ids of the page `query_page` would return. `None` = this store cannot.
    fn query_ids(
        &self,
        _entity_type: &str,
        _query: &RowQuery,
    ) -> Option<Result<RowIdPage, String>> {
        None
    }
}

/// A kernel store driver as a persistence adapter.
pub struct DriverAdapter {
    source: Arc<dyn EntitySource>,
    query: QueryCapabilities,
    write: WriteCapabilities,
    durable: bool,
}

impl DriverAdapter {
    pub fn new(
        source: Arc<dyn EntitySource>,
        query: QueryCapabilities,
        write: WriteCapabilities,
    ) -> Self {
        Self {
            source,
            query,
            write,
            durable: true,
        }
    }

    /// The kernel's in-memory store (development and tests): process memory, refusing a create on a taken id.
    pub fn memory() -> Self {
        Self {
            source: Arc::new(InMemoryEntityCache::refusing_duplicate_ids()),
            query: QueryCapabilities::full(),
            write: WriteCapabilities::full(),
            durable: false,
        }
    }

    /// The Postgres row store, declaring what its SQL and transactions honour.
    pub fn postgres(rows: PostgresRows) -> Self {
        let (query, write) = (rows.query_capabilities(), rows.write_capabilities());
        Self::new(Arc::new(rows), query, write)
    }

    /// The CouchDB row store, declaring what Mango and `_rev`-guarded writes honour.
    pub fn couchdb(rows: CouchRows) -> Self {
        let (query, write) = (rows.query_capabilities(), rows.write_capabilities());
        Self::new(Arc::new(rows), query, write)
    }

    /// The Firestore driver, declaring what its plan and transactions honour.
    pub fn firestore(rows: FirestoreRows) -> Self {
        let (query, write) = (rows.query_capabilities(), rows.write_capabilities());
        Self::new(Arc::new(rows), query, write)
    }
}

fn row_matches(row: &Value, filter: &HashMap<String, Value>) -> bool {
    row.as_object().is_some_and(|obj| {
        filter
            .iter()
            .all(|(key, expected)| obj.get(key) == Some(expected))
    })
}

impl PersistenceHandler for DriverAdapter {
    fn create(&self, entity_type: &str, data: &Value) -> Result<Value, String> {
        self.source.create(entity_type, data)
    }

    fn read(&self, entity_type: &str, id: &str) -> Result<Value, String> {
        self.source
            .find(entity_type, id)?
            .map(|row| (*row).clone())
            .ok_or_else(|| format!("Entity not found: {entity_type}:{id}"))
    }

    fn find(&self, entity_type: &str, id: &str) -> Result<Option<Value>, String> {
        Ok(self.source.find(entity_type, id)?.map(|row| (*row).clone()))
    }

    fn update(&self, entity_type: &str, id: &str, data: &Value) -> Result<Value, String> {
        if self.source.find(entity_type, id)?.is_none() {
            return Err(format!("Entity not found: {entity_type}:{id}"));
        }
        self.source.write(entity_type, id, data)
    }

    fn delete(&self, entity_type: &str, id: &str) -> Result<(), String> {
        self.source.delete(entity_type, id)
    }

    fn list(&self, entity_type: &str) -> Result<Vec<Value>, String> {
        Ok(self
            .source
            .list(entity_type)?
            .iter()
            .map(|row| (**row).clone())
            .collect())
    }

    fn write_where(&self, op: &RowWriteOp) -> Option<Result<RowWriteOutcome, String>> {
        self.source.write_where(op)
    }

    fn write_batch(&self, batch: &RowWriteBatch) -> Option<Result<Vec<RowWriteOutcome>, String>> {
        self.source.write_batch(batch)
    }
}

impl PersistenceAdapter for DriverAdapter {
    fn durable(&self) -> bool {
        self.durable
    }

    fn bind_collection(&self, entity_type: &str, collection: &str) {
        self.source.bind_collection(entity_type, collection);
    }

    fn list_filtered(
        &self,
        entity_type: &str,
        filter: &HashMap<String, Value>,
    ) -> Result<Vec<Value>, String> {
        Ok(self
            .list(entity_type)?
            .into_iter()
            .filter(|row| row_matches(row, filter))
            .collect())
    }

    fn count(&self, entity_type: &str) -> Result<usize, String> {
        Ok(self.list(entity_type)?.len())
    }

    fn count_filtered(
        &self,
        entity_type: &str,
        filter: &HashMap<String, Value>,
    ) -> Result<usize, String> {
        Ok(self.list_filtered(entity_type, filter)?.len())
    }

    fn exists(&self, entity_type: &str, id: &str) -> Result<bool, String> {
        Ok(self.source.find(entity_type, id)?.is_some())
    }

    fn get_many(&self, entity_type: &str, ids: &[&str]) -> Result<Vec<Value>, String> {
        let mut rows = Vec::new();
        for id in ids {
            if let Some(row) = self.find(entity_type, id)? {
                rows.push(row);
            }
        }
        Ok(rows)
    }

    fn delete_all(&self, entity_type: &str) -> Result<usize, String> {
        let rows = self.list(entity_type)?;
        for row in &rows {
            self.delete(entity_type, &orbital_core::runtime::row_query::row_id(row))?;
        }
        Ok(rows.len())
    }

    fn entity_types(&self) -> Vec<String> {
        self.source.entity_types()
    }

    fn query_capabilities(&self) -> Option<QueryCapabilities> {
        Some(self.query.clone())
    }

    fn write_capabilities(&self) -> Option<WriteCapabilities> {
        Some(self.write.clone())
    }

    fn query_page(&self, entity_type: &str, query: &RowQuery) -> Option<Result<RowPage, String>> {
        self.source.query_page(entity_type, query)
    }

    fn query_ids(&self, entity_type: &str, query: &RowQuery) -> Option<Result<RowIdPage, String>> {
        self.source.query_ids(entity_type, query)
    }
}

pub use orbital_core::runtime::store_config::{persistence_spec_from_env, PersistenceSpec};

/// The persistence the environment selects, its drivers speaking over `transport`. A backend whose driver has not
/// landed in the kernel yet is an error naming it.
pub fn persistence_from_env(
    get: &dyn Fn(&str) -> Option<String>,
    read_file: &dyn Fn(&str) -> Result<String, String>,
    transport: Arc<dyn Transport>,
) -> Result<Arc<dyn PersistenceAdapter>, String> {
    let spec = persistence_spec_from_env(get)?;
    Ok(match driver_store(&spec, get, read_file, transport)? {
        None => Arc::new(DriverAdapter::memory()),
        Some(store) => Arc::new(DriverAdapter::new(
            store.source,
            store
                .query
                .ok_or("the store driver declares no query capabilities")?,
            store
                .write
                .ok_or("the store driver declares no write capabilities")?,
        )),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbital_core::runtime::http_service::{HttpRequest, HttpResponse, TransportError};

    fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |name| {
            pairs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| v.to_string())
        }
    }

    struct Dead;

    impl Transport for Dead {
        fn send(&self, _: &HttpRequest) -> Result<HttpResponse, TransportError> {
            Err(TransportError::Failed("no network in this test".into()))
        }
    }

    #[test]
    fn the_backend_comes_from_data_backend_and_defaults_to_firebase() {
        assert_eq!(
            persistence_spec_from_env(&env(&[])),
            Ok(PersistenceSpec::Firestore)
        );
        assert_eq!(
            persistence_spec_from_env(&env(&[
                ("DATA_BACKEND", "postgres"),
                ("DATABASE_URL", "postgres://x"),
                ("PGPOOL_MAX", "7")
            ])),
            Ok(PersistenceSpec::Postgres {
                connection_string: "postgres://x".into(),
                pool_max: Some(7)
            })
        );
        assert_eq!(
            persistence_spec_from_env(&env(&[
                ("DATA_BACKEND", "couchdb"),
                ("COUCHDB_URL", "http://c:5984")
            ])),
            Ok(PersistenceSpec::CouchDb {
                url: "http://c:5984".into()
            })
        );
        assert!(
            persistence_spec_from_env(&env(&[("DATA_BACKEND", "postgres")]))
                .unwrap_err()
                .contains("requires DATABASE_URL")
        );
        assert!(
            persistence_spec_from_env(&env(&[("DATA_BACKEND", "oracle")]))
                .unwrap_err()
                .contains("must be one of")
        );
    }

    #[test]
    fn mock_is_refused_in_production_and_an_unset_node_env_counts_as_production() {
        for pairs in [
            &[("DATA_BACKEND", "mock")][..],
            &[("USE_MOCK_DATA", "true")][..],
            &[("DATA_BACKEND", "mock"), ("NODE_ENV", "production")][..],
        ] {
            let leaked: &'static [(&'static str, &'static str)] =
                Box::leak(pairs.to_vec().into_boxed_slice());
            assert!(
                persistence_spec_from_env(&env(leaked))
                    .unwrap_err()
                    .contains("not permitted"),
                "{pairs:?}"
            );
        }
        assert_eq!(
            persistence_spec_from_env(&env(&[
                ("DATA_BACKEND", "mock"),
                ("NODE_ENV", "development")
            ])),
            Ok(PersistenceSpec::Mock),
            "control: development allows mock"
        );
        assert!(persistence_spec_from_env(&env(&[("USE_MOCK_DATA", "yes")]))
            .unwrap_err()
            .contains("'true' or 'false'"));
    }

    #[test]
    fn the_environment_builds_the_firestore_driver_and_a_missing_driver_names_itself() {
        let no_file = |_: &str| -> Result<String, String> { Err("no file".into()) };
        let built = persistence_from_env(
            &env(&[("FIRESTORE_EMULATOR_HOST", "127.0.0.1:8080")]),
            &no_file,
            Arc::new(Dead),
        )
        .unwrap();
        let caps = built
            .query_capabilities()
            .expect("the driver declares its push-down");
        assert!(caps.window && caps.exact_total);
        assert!(built.write_capabilities().is_some_and(|w| w.atomic_batch));
        assert!(
            persistence_from_env(&env(&[]), &no_file, Arc::new(Dead)).is_err(),
            "control: no credentials builds nothing"
        );
        let postgres = persistence_from_env(
            &env(&[
                ("DATA_BACKEND", "postgres"),
                ("DATABASE_URL", "postgres://u:p@127.0.0.1:1/db"),
            ]),
            &no_file,
            Arc::new(Dead),
        )
        .unwrap();
        assert!(
            postgres
                .write_capabilities()
                .is_some_and(|w| w.atomic_batch),
            "the Postgres driver declares its transactions"
        );
        assert!(
            postgres
                .query_capabilities()
                .is_some_and(|q| q.nearest.is_none()),
            "and leaves nearest to the kernel"
        );
        let couch = persistence_from_env(
            &env(&[
                ("DATA_BACKEND", "couchdb"),
                ("COUCHDB_URL", "http://c:5984"),
            ]),
            &no_file,
            Arc::new(Dead),
        )
        .unwrap();
        assert!(
            couch.write_capabilities().is_some_and(|w| !w.atomic_batch),
            "CouchDB cannot make a batch atomic"
        );
        for (backend, key, value) in [
            ("postgres", "DATABASE_URL", "mysql://x"),
            ("couchdb", "COUCHDB_URL", "ftp://c"),
        ] {
            let leaked: &'static [(&'static str, &'static str)] =
                Box::leak(vec![("DATA_BACKEND", backend), (key, value)].into_boxed_slice());
            assert!(
                persistence_from_env(&env(leaked), &no_file, Arc::new(Dead)).is_err(),
                "control: a {backend} url of another scheme builds nothing"
            );
        }
        let mock = persistence_from_env(
            &env(&[("DATA_BACKEND", "mock"), ("NODE_ENV", "development")]),
            &no_file,
            Arc::new(Dead),
        )
        .unwrap();
        assert!(
            !mock.durable() && mock.entity_types().is_empty(),
            "control: development builds the in-memory adapter"
        );
    }

    fn row(pairs: &[(&str, &str)]) -> Value {
        let mut m = orbital_core::ValueMap::new();
        for (k, v) in pairs {
            m.insert((*k).to_string(), Value::String((*v).to_string()));
        }
        Value::Object(m)
    }

    #[test]
    fn the_memory_store_answers_the_whole_adapter_contract() {
        let store = DriverAdapter::memory();
        assert!(!store.durable(), "process memory is not durable");
        assert!(store.query_capabilities().is_some() && store.write_capabilities().is_some());
        for (id, status) in [("u0", "active"), ("u1", "pending"), ("u2", "active")] {
            store
                .create("User", &row(&[("id", id), ("status", status)]))
                .unwrap();
        }
        store.create("Post", &row(&[("id", "p1")])).unwrap();
        assert!(store
            .create("User", &row(&[("id", "u0")]))
            .unwrap_err()
            .contains("already exists"));
        let minted = store.create("User", &row(&[("name", "n")])).unwrap();
        assert!(store
            .exists("User", &orbital_core::runtime::row_query::row_id(&minted))
            .unwrap());
        let active = HashMap::from([("status".to_string(), Value::String("active".into()))]);
        assert_eq!(store.list_filtered("User", &active).unwrap().len(), 2);
        assert_eq!(store.count_filtered("User", &active).unwrap(), 2);
        assert_eq!(store.count("User").unwrap(), 4);
        assert_eq!(
            store.get_many("User", &["u1", "u2", "u9"]).unwrap().len(),
            2
        );
        let updated = store
            .update("User", "u1", &row(&[("status", "active")]))
            .unwrap();
        assert_eq!(
            updated.as_object().unwrap()["status"],
            Value::String("active".into())
        );
        assert!(
            store.update("User", "u9", &row(&[])).is_err(),
            "control: no row, no update"
        );
        assert!(store
            .read("User", "u9")
            .unwrap_err()
            .contains("Entity not found"));
        let mut types = store.entity_types();
        types.sort();
        assert_eq!(types, ["Post", "User"]);
        assert_eq!(store.delete_all("User").unwrap(), 4);
        assert_eq!(store.count("User").unwrap(), 0);
        assert_eq!(
            store.count("Post").unwrap(),
            1,
            "control: another table is untouched"
        );
    }
}
