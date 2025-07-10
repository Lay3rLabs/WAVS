use std::ops::Bound;

use redb::ReadableTable;
use thiserror::Error;
use tracing::instrument;
use utils::storage::db::{DBError, RedbStorage, Table, JSON};
use wavs_types::{Service, ServiceID, ServiceStatus};

const SERVICE_TABLE: Table<&str, JSON<Service>> = Table::new("services");

type Result<T> = std::result::Result<T, ServicesError>;

#[derive(Clone)]
pub struct Services {
    db_storage: RedbStorage,
}

impl Services {
    pub fn new(db_storage: RedbStorage) -> Self {
        Self { db_storage }
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Services"))]
    pub fn try_get(&self, id: &ServiceID) -> Result<Option<Service>> {
        match self.db_storage.get(SERVICE_TABLE, id.as_ref()) {
            Ok(Some(service)) => Ok(Some(service.value())),
            Ok(None) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Services"))]
    pub fn get(&self, service_id: &ServiceID) -> Result<Service> {
        match self.try_get(service_id)? {
            Some(service) => Ok(service),
            None => Err(ServicesError::UnknownService(service_id.clone())),
        }
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Services"))]
    pub fn exists(&self, service_id: &ServiceID) -> Result<bool> {
        match self.db_storage.get(SERVICE_TABLE, service_id.as_ref())? {
            Some(_) => Ok(true),
            None => Ok(false),
        }
    }

    pub fn is_active(&self, service_id: &ServiceID) -> bool {
        self.get(service_id)
            .map(|service| match service.status {
                ServiceStatus::Active => true,
                ServiceStatus::Paused => false,
            })
            .unwrap_or(false)
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Services"))]
    pub fn remove(&self, service_id: &ServiceID) -> Result<()> {
        self.db_storage
            .remove(SERVICE_TABLE, service_id.as_ref())
            .map_err(|e| e.into())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Services"))]
    pub fn save(&self, service: &Service) -> Result<()> {
        self.db_storage
            .set(SERVICE_TABLE, service.id.as_ref(), service)
            .map_err(|e| e.into())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Services"))]
    pub fn list(&self, bounds_start: Bound<&str>, bounds_end: Bound<&str>) -> Result<Vec<Service>> {
        let res = self
            .db_storage
            .map_table_read(SERVICE_TABLE, |table| match table {
                // TODO: try to refactor. There's a couple areas of improvement:
                //
                // 1. just taking in a RangeBounds<&str> instead of two Bound<&str>
                // 2. just calling `.range()` on the range once
                Some(table) => match (bounds_start, bounds_end) {
                    (Bound::Unbounded, Bound::Unbounded) => {
                        let res = table
                            .iter()?
                            .map(|i| i.map(|(_, value)| value.value()))
                            .collect::<std::result::Result<Vec<_>, redb::StorageError>>()?;
                        Ok(res)
                    }
                    (Bound::Unbounded, Bound::Included(y)) => {
                        let res = table
                            .range(..=y)?
                            .map(|i| i.map(|(_, value)| value.value()))
                            .collect::<std::result::Result<Vec<_>, redb::StorageError>>()?;

                        Ok(res)
                    }
                    (Bound::Unbounded, Bound::Excluded(y)) => {
                        let res = table
                            .range(..y)?
                            .map(|i| i.map(|(_, value)| value.value()))
                            .collect::<std::result::Result<Vec<_>, redb::StorageError>>()?;

                        Ok(res)
                    }
                    (Bound::Included(x), Bound::Unbounded) => {
                        let res = table
                            .range(x..)?
                            .map(|i| i.map(|(_, value)| value.value()))
                            .collect::<std::result::Result<Vec<_>, redb::StorageError>>()?;

                        Ok(res)
                    }
                    (Bound::Excluded(x), Bound::Unbounded) => {
                        let res = table
                            .range(x..)?
                            .skip(1)
                            .map(|i| i.map(|(_, value)| value.value()))
                            .collect::<std::result::Result<Vec<_>, redb::StorageError>>()?;

                        Ok(res)
                    }
                    (Bound::Included(x), Bound::Included(y)) => {
                        let res = table
                            .range(x..=y)?
                            .map(|i| i.map(|(_, value)| value.value()))
                            .collect::<std::result::Result<Vec<_>, redb::StorageError>>()?;

                        Ok(res)
                    }
                    (Bound::Included(x), Bound::Excluded(y)) => {
                        let res = table
                            .range(x..y)?
                            .map(|i| i.map(|(_, value)| value.value()))
                            .collect::<std::result::Result<Vec<_>, redb::StorageError>>()?;

                        Ok(res)
                    }
                    (Bound::Excluded(x), Bound::Included(y)) => {
                        let res = table
                            .range(x..=y)?
                            .skip(1)
                            .map(|i| i.map(|(_, value)| value.value()))
                            .collect::<std::result::Result<Vec<_>, redb::StorageError>>()?;
                        Ok(res)
                    }
                    (Bound::Excluded(x), Bound::Excluded(y)) => {
                        let res = table
                            .range(x..y)?
                            .skip(1)
                            .map(|i| i.map(|(_, value)| value.value()))
                            .collect::<std::result::Result<Vec<_>, redb::StorageError>>()?;
                        Ok(res)
                    }
                },
                None => Ok(Vec::new()),
            })?;

        Ok(res)
    }
}

#[derive(Error, Debug)]
pub enum ServicesError {
    #[error("Unknown Service {0}")]
    UnknownService(ServiceID),

    #[error("Database error: {0}")]
    DBError(#[from] DBError),
}
