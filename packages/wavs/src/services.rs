use std::collections::BTreeMap;
use std::ops::Bound;

use thiserror::Error;
use tracing::instrument;
use utils::storage::db::{DBError, WavsDb};
use wavs_types::{Service, ServiceId, ServiceStatus, Workflow, WorkflowId};

type Result<T> = std::result::Result<T, ServicesError>;

#[derive(Clone)]
pub struct Services {
    db_storage: WavsDb,
}

impl Services {
    pub fn new(db_storage: WavsDb) -> Self {
        Self { db_storage }
    }

    #[instrument(skip(self), fields(subsys = "Services"))]
    pub fn try_get(&self, id: &ServiceId) -> Result<Option<Service>> {
        Ok(self.db_storage.services.get_cloned(id))
    }

    #[instrument(skip(self), fields(subsys = "Services"))]
    pub fn get(&self, service_id: &ServiceId) -> Result<Service> {
        match self.try_get(service_id)? {
            Some(service) => Ok(service),
            None => Err(ServicesError::UnknownService(service_id.clone())),
        }
    }

    #[instrument(skip(self), fields(subsys = "Services"))]
    pub fn get_workflow(
        &self,
        service_id: &ServiceId,
        workflow_id: &WorkflowId,
    ) -> Result<Workflow> {
        let service = self.get(service_id)?;
        service
            .workflows
            .get(workflow_id)
            .cloned()
            .ok_or_else(|| ServicesError::UnknownWorkflow {
                service_name: service.name,
                service_id: service_id.clone(),
                workflow_id: workflow_id.clone(),
            })
    }

    #[instrument(skip(self), fields(subsys = "Services"))]
    pub fn exists(&self, service_id: &ServiceId) -> Result<bool> {
        Ok(self.db_storage.services.contains_key(service_id))
    }

    pub fn is_active(&self, service_id: &ServiceId) -> bool {
        self.get(service_id)
            .map(|service| match service.status {
                ServiceStatus::Active => true,
                ServiceStatus::Paused => false,
            })
            .unwrap_or(false)
    }

    #[instrument(skip(self), fields(subsys = "Services"))]
    pub fn remove(&self, service_id: &ServiceId) -> Result<()> {
        self.db_storage.services.remove(service_id);
        Ok(())
    }

    #[instrument(skip(self), fields(subsys = "Services"))]
    pub fn save(&self, service: &Service) -> Result<()> {
        self.db_storage
            .services
            .insert(service.id(), service.clone())
            .map_err(|e| e.into())
    }

    #[instrument(skip(self), fields(subsys = "Services"))]
    pub fn list(
        &self,
        bounds_start: Bound<&ServiceId>,
        bounds_end: Bound<&ServiceId>,
    ) -> Result<Vec<Service>> {
        let mut services = BTreeMap::new();

        for entry in self.db_storage.services.iter() {
            let (key, value) = entry.pair();
            services.insert(key.clone(), value.clone());
        }

        let convert_bound = |bound: Bound<&ServiceId>| match bound {
            Bound::Unbounded => Bound::Unbounded,
            Bound::Included(id) => Bound::Included(id.clone()),
            Bound::Excluded(id) => Bound::Excluded(id.clone()),
        };

        let start = convert_bound(bounds_start);
        let end = convert_bound(bounds_end);

        let services = services
            .range((start, end))
            .map(|(_, service)| service.clone())
            .collect();

        Ok(services)
    }
}

#[derive(Error, Debug)]
pub enum ServicesError {
    #[error("Unknown Service {0}")]
    UnknownService(ServiceId),

    #[error("Unknown Workflow {workflow_id} for Service {service_name} (id: {service_id})")]
    UnknownWorkflow {
        service_name: String,
        service_id: ServiceId,
        workflow_id: WorkflowId,
    },

    #[error("Database error: {0}")]
    DBError(#[from] DBError),
}

#[macro_export]
macro_rules! tracing_service_info {
    ($services:expr, $service_id:expr, $($msg:tt)*) => {
        if tracing::enabled!(tracing::Level::INFO) {
            match $services.get(&$service_id).ok() {
                Some(service) => {
                    tracing::info!(service.name = %service.name, service.manager = ?service.manager, "Service {} [{:?}]: {}", service.name, service.manager, format_args!($($msg)*));
                },
                None => {
                    tracing::info!(service.id = %$service_id, "Service [id: {}]: {}", $service_id, format_args!($($msg)*));
                }
            }
        }
    };
}

#[macro_export]
macro_rules! tracing_service_debug {
    ($services:expr, $service_id:expr, $($msg:tt)*) => {
        if tracing::enabled!(tracing::Level::DEBUG) {
            match $services.get(&$service_id).ok() {
                Some(service) => {
                    tracing::debug!(service.name = %service.name, service.manager = ?service.manager, "Service {} [{:?}]: {}", service.name, service.manager, format_args!($($msg)*));
                },
                None => {
                    tracing::debug!(service.id = %$service_id, "Service [id: {}]: {}", $service_id, format_args!($($msg)*));
                }
            }
        }
    };
}

#[macro_export]
macro_rules! tracing_service_trace {
    ($services:expr, $service_id:expr, $($msg:tt)*) => {
        if tracing::enabled!(tracing::Level::TRACE) {
            match $services.get(&$service_id).ok() {
                Some(service) => {
                    tracing::trace!(service.name = %service.name, service.manager = ?service.manager, "Service {} [{:?}]: {}", service.name, service.manager, format_args!($($msg)*));
                },
                None => {
                    tracing::trace!(service.id = %$service_id, "Service [id: {}]: {}", $service_id, format_args!($($msg)*));
                }
            }
        }
    };
}
#[macro_export]
macro_rules! tracing_service_warn {
    ($services:expr, $service_id:expr, $($msg:tt)*) => {
        if tracing::enabled!(tracing::Level::WARN) {
            match $services.get(&$service_id).ok() {
                Some(service) => {
                    tracing::warn!(service.name = %service.name, service.manager = ?service.manager, "Service {} [{:?}]: {}", service.name, service.manager, format_args!($($msg)*));
                },
                None => {
                    tracing::warn!(service.id = %$service_id, "Service [id: {}]: {}", $service_id, format_args!($($msg)*));
                }
            }
        }
    };
}

#[macro_export]
macro_rules! tracing_service_error {
    ($services:expr, $service_id:expr, $($msg:tt)*) => {
        if tracing::enabled!(tracing::Level::ERROR) {
            match $services.get(&$service_id).ok() {
                Some(service) => {
                    tracing::error!(service.name = %service.name, service.manager = ?service.manager, "Service {} [{:?}]: {}", service.name, service.manager, format_args!($($msg)*));
                },
                None => {
                    tracing::error!(service.id = %$service_id, "Service [id: {}]: {}", $service_id, format_args!($($msg)*));
                }
            }
        }
    };
}
