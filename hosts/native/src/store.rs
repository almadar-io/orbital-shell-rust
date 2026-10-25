//! Kernel store drivers as the server's persistence: a driver is an `EntitySource` over raw ports (sockets, HTTP),
//! so no host process stands between the kernel and the data. [`DriverAdapter`] presents one as a
//! [`PersistenceAdapter`].
//!
//! [`PersistenceAdapter`] is the interface every store backend presents to the host.

use orbital_core::runtime::entity_source::{
    EntitySource, InMemoryEntityCache, QueryAnswer, NO_QUERY_PUSHDOWN,
};
use orbital_core::runtime::row_query::{
    QueryCapabilities, RowIdPage, RowPage, RowQuery, RowWriteBatch, RowWriteOp, RowWriteOutcome,
    WriteCapabilities,
};
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

    /// The page of rows matching `query`, run by the store. A store that cannot declines with its reason.
    fn query_page(&self, _entity_type: &str, _query: &RowQuery) -> QueryAnswer<RowPage> {
        QueryAnswer::Declined(NO_QUERY_PUSHDOWN.to_string())
    }

    /// The ids of the page `query_page` would return. A store that cannot declines with its reason.
    fn query_ids(&self, _entity_type: &str, _query: &RowQuery) -> QueryAnswer<RowIdPage> {
        QueryAnswer::Declined(NO_QUERY_PUSHDOWN.to_string())
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

    fn query_page(&self, entity_type: &str, query: &RowQuery) -> QueryAnswer<RowPage> {
        self.source.query_page(entity_type, query)
    }

    fn query_ids(&self, entity_type: &str, query: &RowQuery) -> QueryAnswer<RowIdPage> {
        self.source.query_ids(entity_type, query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
