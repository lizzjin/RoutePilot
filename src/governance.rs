use std::{
    collections::{BTreeMap, VecDeque},
    env,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore};
use uuid::Uuid;

use crate::{config::ResolvedProvider, domain::TenantScope, error::AppError, storage::JsonStore};

const DEFAULT_LOCAL_EXECUTING_PER_USER: usize = 1;
const DEFAULT_LOCAL_QUEUED_PER_USER: usize = 2;
const DEFAULT_LOCAL_QUEUE_GLOBAL: usize = 16;
const DEFAULT_BATCH_QUEUE_GLOBAL: usize = 16;
const DEFAULT_OVERFLOW_AFTER: Duration = Duration::from_secs(5);
const DEFAULT_STRICT_WAIT: Duration = Duration::from_secs(60);
const DEFAULT_SERVICE_TIME: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HybridMode {
    #[default]
    LocalStrict,
    LocalFirst,
    Balanced,
    CloudFirst,
}

impl HybridMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::LocalStrict => "local_strict",
            Self::LocalFirst => "local_first",
            Self::Balanced => "balanced",
            Self::CloudFirst => "cloud_first",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "local_strict" => Some(Self::LocalStrict),
            "local_first" => Some(Self::LocalFirst),
            "balanced" => Some(Self::Balanced),
            "cloud_first" => Some(Self::CloudFirst),
            _ => None,
        }
    }

    fn egress_rank(self) -> u8 {
        match self {
            Self::LocalStrict => 0,
            Self::LocalFirst => 1,
            Self::Balanced => 2,
            Self::CloudFirst => 3,
        }
    }

    pub(crate) fn restrict_to(self, maximum: Self) -> Result<Self, AppError> {
        if self.egress_rank() <= maximum.egress_rank() {
            Ok(self)
        } else {
            Err(AppError::Forbidden(format!(
                "requested routing mode {} exceeds project maximum {}",
                self.as_str(),
                maximum.as_str()
            )))
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DataClassification {
    #[default]
    Unknown,
    Sensitive,
    Internal,
    Public,
}

impl DataClassification {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "unknown" => Some(Self::Unknown),
            "sensitive" => Some(Self::Sensitive),
            "internal" => Some(Self::Internal),
            "public" => Some(Self::Public),
            _ => None,
        }
    }

    pub(crate) fn forces_local(self) -> bool {
        matches!(self, Self::Unknown | Self::Sensitive)
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WorkloadClass {
    #[default]
    Interactive,
    Batch,
}

impl WorkloadClass {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "interactive" | "business" | "diagnostic" | "synthetic" => Some(Self::Interactive),
            "batch" => Some(Self::Batch),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProviderBoundary {
    Local,
    Cloud,
}

impl ProviderBoundary {
    pub(crate) fn for_resolved(provider: &ResolvedProvider) -> Self {
        if provider.provider_id.starts_with("local_")
            || provider.provider_id.starts_with("cpa_")
            || provider.provider_id == "ollama"
            || reqwest::Url::parse(&provider.provider.base_url)
                .ok()
                .and_then(|url| url.host_str().map(str::to_owned))
                .is_some_and(|host| {
                    host.eq_ignore_ascii_case("localhost")
                        || host.parse::<std::net::IpAddr>().is_ok_and(is_local_address)
                })
        {
            Self::Local
        } else {
            Self::Cloud
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectPolicy {
    pub organization_id: String,
    pub project_id: String,
    pub environment_id: String,
    #[serde(default)]
    pub maximum_mode: HybridMode,
    #[serde(default)]
    pub default_classification: DataClassification,
    #[serde(default)]
    pub allowed_providers: Vec<String>,
    #[serde(default)]
    pub allowed_models: Vec<String>,
    #[serde(default)]
    pub allowed_regions: Vec<String>,
    #[serde(default)]
    pub allowed_api_versions: Vec<String>,
    #[serde(default)]
    pub cloud_enabled: bool,
    #[serde(default)]
    pub updated_by: String,
    #[serde(default)]
    pub updated_at_ms: u64,
}

impl ProjectPolicy {
    fn fail_closed(tenant: &TenantScope) -> Self {
        Self {
            organization_id: tenant.organization_id.to_string(),
            project_id: tenant.project_id.to_string(),
            environment_id: tenant.environment_id.to_string(),
            maximum_mode: HybridMode::LocalStrict,
            default_classification: DataClassification::Unknown,
            allowed_providers: Vec::new(),
            allowed_models: Vec::new(),
            allowed_regions: vec!["local".to_owned()],
            allowed_api_versions: Vec::new(),
            cloud_enabled: false,
            updated_by: "builtin-fail-closed".to_owned(),
            updated_at_ms: 0,
        }
    }

    fn key(&self) -> String {
        policy_key(
            &self.organization_id,
            &self.project_id,
            &self.environment_id,
        )
    }

    pub(crate) fn effective_mode(
        &self,
        requested: Option<HybridMode>,
        classification: DataClassification,
    ) -> Result<HybridMode, AppError> {
        if classification.forces_local() {
            return Ok(HybridMode::LocalStrict);
        }
        requested
            .unwrap_or(self.maximum_mode)
            .restrict_to(self.maximum_mode)
    }

    pub(crate) fn enforce_attempt(&self, provider: &ResolvedProvider) -> Result<(), AppError> {
        let boundary = ProviderBoundary::for_resolved(provider);
        if boundary == ProviderBoundary::Cloud && !self.cloud_enabled {
            return Err(AppError::Forbidden(
                "project policy does not enable cloud routing".to_owned(),
            ));
        }
        if !self.allowed_providers.is_empty()
            && !matches_policy(&self.allowed_providers, &provider.provider_id)
        {
            return Err(AppError::Forbidden(format!(
                "project policy does not allow provider {}",
                provider.provider_id
            )));
        }
        if !self.allowed_models.is_empty() && !matches_policy(&self.allowed_models, &provider.model)
        {
            return Err(AppError::Forbidden(format!(
                "project policy does not allow model {}",
                provider.model
            )));
        }
        let (region, api_version) = provider_governance_metadata(provider);
        if !self.allowed_regions.is_empty() && !matches_policy(&self.allowed_regions, &region) {
            return Err(AppError::Forbidden(format!(
                "project policy does not allow provider region {region}"
            )));
        }
        if !self.allowed_api_versions.is_empty()
            && !matches_policy(&self.allowed_api_versions, &api_version)
        {
            return Err(AppError::Forbidden(format!(
                "project policy does not allow provider API version {api_version}"
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChangeRequestInput {
    pub action: String,
    pub target: String,
    #[serde(default)]
    pub payload: Value,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChangeApproval {
    pub actor_id: String,
    pub actor_name: String,
    pub approved_at_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChangeRequest {
    pub id: String,
    pub action: String,
    pub target: String,
    pub payload: Value,
    pub payload_sha256: String,
    pub reason: String,
    pub risk: String,
    pub status: String,
    pub requested_by: String,
    pub requested_by_name: String,
    pub approvals: Vec<ChangeApproval>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub applied_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GovernanceDocument {
    #[serde(default)]
    project_policies: BTreeMap<String, ProjectPolicy>,
    #[serde(default)]
    change_requests: BTreeMap<String, ChangeRequest>,
}

pub(crate) struct GovernanceStore {
    store: Option<JsonStore>,
    inner: Mutex<GovernanceDocument>,
    revision: AtomicU64,
    persistence_degraded: AtomicBool,
}

impl GovernanceStore {
    pub(crate) fn load() -> Result<Self, AppError> {
        let store = JsonStore::open("governance")?;
        let (document, revision) = store.read_versioned_or_default(json!({
            "projectPolicies": {},
            "changeRequests": {},
        }))?;
        Ok(Self {
            store: Some(store),
            inner: Mutex::new(document),
            revision: AtomicU64::new(revision),
            persistence_degraded: AtomicBool::new(false),
        })
    }

    #[cfg(test)]
    pub(crate) fn for_tests() -> Self {
        Self {
            store: None,
            inner: Mutex::new(GovernanceDocument::default()),
            revision: AtomicU64::new(0),
            persistence_degraded: AtomicBool::new(false),
        }
    }

    pub(crate) fn is_ready(&self) -> bool {
        !self.persistence_degraded.load(Ordering::Acquire)
    }

    pub(crate) fn effective_policy(&self, tenant: &TenantScope) -> ProjectPolicy {
        self.inner
            .lock()
            .expect("governance lock poisoned")
            .project_policies
            .get(&policy_key(
                tenant.organization_id.as_str(),
                tenant.project_id.as_str(),
                tenant.environment_id.as_str(),
            ))
            .cloned()
            .unwrap_or_else(|| ProjectPolicy::fail_closed(tenant))
    }

    pub(crate) fn list_policies(&self) -> Vec<ProjectPolicy> {
        self.inner
            .lock()
            .expect("governance lock poisoned")
            .project_policies
            .values()
            .cloned()
            .collect()
    }

    pub(crate) fn list_change_requests(&self) -> Vec<ChangeRequest> {
        self.inner
            .lock()
            .expect("governance lock poisoned")
            .change_requests
            .values()
            .rev()
            .cloned()
            .collect()
    }

    pub(crate) fn create_change_request(
        &self,
        actor_id: &str,
        actor_name: &str,
        input: ChangeRequestInput,
    ) -> Result<ChangeRequest, AppError> {
        validate_change_input(&input)?;
        let now = now_millis();
        let request = ChangeRequest {
            id: format!("chg_{}", Uuid::new_v4().simple()),
            action: input.action,
            target: input.target,
            payload_sha256: payload_sha256(&input.payload)?,
            payload: input.payload,
            reason: input.reason.trim().to_owned(),
            risk: "high".to_owned(),
            status: "pending_second_approval".to_owned(),
            requested_by: actor_id.to_owned(),
            requested_by_name: actor_name.to_owned(),
            approvals: vec![ChangeApproval {
                actor_id: actor_id.to_owned(),
                actor_name: actor_name.to_owned(),
                approved_at_ms: now,
            }],
            created_at_ms: now,
            updated_at_ms: now,
            applied_at_ms: None,
        };
        let mut inner = self.inner.lock().expect("governance lock poisoned");
        let previous = inner.clone();
        inner
            .change_requests
            .insert(request.id.clone(), request.clone());
        self.save_or_restore(&mut inner, previous)?;
        Ok(request)
    }

    pub(crate) fn approve_change_request(
        &self,
        request_id: &str,
        actor_id: &str,
        actor_name: &str,
    ) -> Result<ChangeRequest, AppError> {
        let mut inner = self.inner.lock().expect("governance lock poisoned");
        let previous = inner.clone();
        let request = inner
            .change_requests
            .get_mut(request_id)
            .ok_or_else(|| AppError::NotFound("change request not found".to_owned()))?;
        if request.status == "applied" {
            return Err(AppError::StateConflict(
                "change request has already been applied".to_owned(),
            ));
        }
        if request
            .approvals
            .iter()
            .any(|approval| approval.actor_id == actor_id)
        {
            return Err(AppError::InvalidRequest(
                "the same administrator cannot provide both approvals".to_owned(),
            ));
        }
        request.approvals.push(ChangeApproval {
            actor_id: actor_id.to_owned(),
            actor_name: actor_name.to_owned(),
            approved_at_ms: now_millis(),
        });
        request.status = "approved".to_owned();
        request.updated_at_ms = now_millis();
        let output = request.clone();
        self.save_or_restore(&mut inner, previous)?;
        Ok(output)
    }

    pub(crate) fn approved_change(&self, request_id: &str) -> Result<ChangeRequest, AppError> {
        let inner = self.inner.lock().expect("governance lock poisoned");
        let request = inner
            .change_requests
            .get(request_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound("change request not found".to_owned()))?;
        if request.status != "approved" || request.approvals.len() < 2 {
            return Err(AppError::Forbidden(
                "high-risk change requires two distinct administrator approvals".to_owned(),
            ));
        }
        Ok(request)
    }

    pub(crate) fn verify_approved_change(
        &self,
        request_id: &str,
        action: &str,
        target: &str,
        payload: &Value,
    ) -> Result<ChangeRequest, AppError> {
        let request = self.approved_change(request_id)?;
        if request.action != action
            || request.target != target
            || request.payload_sha256 != payload_sha256(payload)?
        {
            return Err(AppError::Forbidden(
                "approved change request does not match this action, target, and payload"
                    .to_owned(),
            ));
        }
        Ok(request)
    }

    pub(crate) fn mark_change_applied(&self, request_id: &str) -> Result<(), AppError> {
        let mut inner = self.inner.lock().expect("governance lock poisoned");
        let previous = inner.clone();
        let request = inner
            .change_requests
            .get_mut(request_id)
            .ok_or_else(|| AppError::NotFound("change request not found".to_owned()))?;
        if request.status != "approved" || request.approvals.len() < 2 {
            return Err(AppError::Forbidden(
                "high-risk change requires two distinct administrator approvals".to_owned(),
            ));
        }
        request.status = "applied".to_owned();
        request.applied_at_ms = Some(now_millis());
        request.updated_at_ms = now_millis();
        self.save_or_restore(&mut inner, previous)
    }

    pub(crate) fn apply_project_policy(
        &self,
        request_id: &str,
        actor_id: &str,
    ) -> Result<ProjectPolicy, AppError> {
        let request = self.approved_change(request_id)?;
        if request.action != "project_policy.upsert" {
            return Err(AppError::InvalidRequest(
                "change request is not a project policy update".to_owned(),
            ));
        }
        let mut policy: ProjectPolicy = serde_json::from_value(request.payload.clone())?;
        validate_project_policy(&policy)?;
        policy.updated_by = actor_id.to_owned();
        policy.updated_at_ms = now_millis();
        let mut inner = self.inner.lock().expect("governance lock poisoned");
        let previous = inner.clone();
        inner.project_policies.insert(policy.key(), policy.clone());
        let change = inner
            .change_requests
            .get_mut(request_id)
            .ok_or_else(|| AppError::NotFound("change request not found".to_owned()))?;
        change.status = "applied".to_owned();
        change.applied_at_ms = Some(now_millis());
        change.updated_at_ms = now_millis();
        self.save_or_restore(&mut inner, previous)?;
        Ok(policy)
    }

    fn save_or_restore(
        &self,
        inner: &mut GovernanceDocument,
        previous: GovernanceDocument,
    ) -> Result<(), AppError> {
        let result = if let Some(store) = &self.store {
            let expected = self.revision.load(Ordering::Acquire);
            store
                .compare_and_swap_json(expected, inner)
                .map(|revision| self.revision.store(revision, Ordering::Release))
        } else {
            Ok(())
        };
        self.persistence_degraded
            .store(result.is_err(), Ordering::Release);
        if let Err(error) = result {
            *inner = previous;
            return Err(error);
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LocalSchedulerConfig {
    pub executing_per_user: usize,
    pub queued_per_user: usize,
    pub global_interactive_queue: usize,
    pub global_batch_queue: usize,
    pub overflow_after: Duration,
    pub strict_wait: Duration,
}

impl LocalSchedulerConfig {
    pub(crate) fn from_env() -> Result<Self, AppError> {
        let config = Self {
            executing_per_user: env_usize(
                "MODELPORT_LOCAL_EXECUTING_PER_USER",
                DEFAULT_LOCAL_EXECUTING_PER_USER,
            ),
            queued_per_user: env_usize(
                "MODELPORT_LOCAL_QUEUED_PER_USER",
                DEFAULT_LOCAL_QUEUED_PER_USER,
            ),
            global_interactive_queue: env_usize(
                "MODELPORT_LOCAL_QUEUE_GLOBAL",
                DEFAULT_LOCAL_QUEUE_GLOBAL,
            ),
            global_batch_queue: env_usize(
                "MODELPORT_BATCH_QUEUE_GLOBAL",
                DEFAULT_BATCH_QUEUE_GLOBAL,
            ),
            overflow_after: Duration::from_secs(env_u64(
                "MODELPORT_LOCAL_OVERFLOW_AFTER_SECONDS",
                DEFAULT_OVERFLOW_AFTER.as_secs(),
            )),
            strict_wait: Duration::from_secs(env_u64(
                "MODELPORT_LOCAL_STRICT_WAIT_SECONDS",
                DEFAULT_STRICT_WAIT.as_secs(),
            )),
        };
        if config.executing_per_user != 1
            || config.queued_per_user != 2
            || config.global_interactive_queue != 16
            || config.overflow_after != Duration::from_secs(5)
            || config.strict_wait != Duration::from_secs(60)
        {
            return Err(AppError::Config(
                "the 40-user baseline requires local limits 1 executing / 2 queued per user, global queue 16, overflow 5s, strict wait 60s"
                    .to_owned(),
            ));
        }
        Ok(config)
    }

    #[cfg(test)]
    pub(crate) fn for_tests() -> Self {
        Self {
            executing_per_user: DEFAULT_LOCAL_EXECUTING_PER_USER,
            queued_per_user: DEFAULT_LOCAL_QUEUED_PER_USER,
            global_interactive_queue: DEFAULT_LOCAL_QUEUE_GLOBAL,
            global_batch_queue: DEFAULT_BATCH_QUEUE_GLOBAL,
            overflow_after: DEFAULT_OVERFLOW_AFTER,
            strict_wait: DEFAULT_STRICT_WAIT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalAdmission {
    Acquired,
    OverflowToCloud,
}

#[derive(Debug)]
struct QueuedRequest {
    id: u64,
    enqueued: Instant,
    notify: Arc<Notify>,
}

#[derive(Debug, Default)]
struct SchedulerState {
    running_by_user: BTreeMap<String, usize>,
    queued_by_user: BTreeMap<String, usize>,
    interactive: VecDeque<QueuedRequest>,
    batch: VecDeque<QueuedRequest>,
    next_id: u64,
    service_time_ms: u64,
}

#[derive(Debug)]
pub(crate) struct LocalScheduler {
    config: LocalSchedulerConfig,
    execution: Arc<Semaphore>,
    state: Arc<Mutex<SchedulerState>>,
}

#[derive(Debug)]
pub(crate) struct LocalLease {
    user_id: String,
    started: Instant,
    permit: Option<OwnedSemaphorePermit>,
    scheduler: Arc<LocalScheduler>,
}

impl LocalScheduler {
    pub(crate) fn new(config: LocalSchedulerConfig) -> Arc<Self> {
        Arc::new(Self {
            config,
            execution: Arc::new(Semaphore::new(1)),
            state: Arc::new(Mutex::new(SchedulerState {
                service_time_ms: DEFAULT_SERVICE_TIME.as_millis() as u64,
                ..SchedulerState::default()
            })),
        })
    }

    pub(crate) fn snapshot(&self) -> Value {
        let state = self.state.lock().expect("local scheduler lock poisoned");
        json!({
            "running": state.running_by_user.values().sum::<usize>(),
            "interactiveQueued": state.interactive.len(),
            "batchQueued": state.batch.len(),
            "usersQueued": state.queued_by_user.len(),
            "estimatedServiceMs": state.service_time_ms,
            "oldestInteractiveWaitMs": state.interactive.front()
                .map(|request| request.enqueued.elapsed().as_millis() as u64)
                .unwrap_or(0),
            "oldestBatchWaitMs": state.batch.front()
                .map(|request| request.enqueued.elapsed().as_millis() as u64)
                .unwrap_or(0),
            "limits": {
                "executingPerUser": self.config.executing_per_user,
                "queuedPerUser": self.config.queued_per_user,
                "globalInteractiveQueue": self.config.global_interactive_queue,
                "globalBatchQueue": self.config.global_batch_queue,
                "overflowAfterSeconds": self.config.overflow_after.as_secs(),
                "strictWaitSeconds": self.config.strict_wait.as_secs(),
            }
        })
    }

    pub(crate) async fn acquire(
        self: &Arc<Self>,
        user_id: &str,
        mode: HybridMode,
        class: WorkloadClass,
        cloud_available: bool,
    ) -> Result<(LocalAdmission, Option<LocalLease>), AppError> {
        if let Ok(permit) = self.execution.clone().try_acquire_owned() {
            let mut state = self.state.lock().expect("local scheduler lock poisoned");
            let running = state.running_by_user.entry(user_id.to_owned()).or_default();
            if *running < self.config.executing_per_user {
                *running += 1;
                drop(state);
                return Ok((
                    LocalAdmission::Acquired,
                    Some(LocalLease {
                        user_id: user_id.to_owned(),
                        started: Instant::now(),
                        permit: Some(permit),
                        scheduler: self.clone(),
                    }),
                ));
            }
            drop(permit);
        }

        let estimated_wait = self.estimated_wait(class);
        if matches!(mode, HybridMode::LocalFirst | HybridMode::Balanced)
            && cloud_available
            && estimated_wait > self.config.overflow_after
        {
            return Ok((LocalAdmission::OverflowToCloud, None));
        }

        let notify = Arc::new(Notify::new());
        let request_id = {
            let mut state = self.state.lock().expect("local scheduler lock poisoned");
            let queued = state.queued_by_user.get(user_id).copied().unwrap_or(0);
            if queued >= self.config.queued_per_user {
                return Err(AppError::RateLimited {
                    message: "per-user local queue limit exceeded (2 queued)".to_owned(),
                    retry_after_secs: retry_after_seconds(state.service_time_ms),
                });
            }
            let queue = match class {
                WorkloadClass::Interactive => &state.interactive,
                WorkloadClass::Batch => &state.batch,
            };
            let limit = match class {
                WorkloadClass::Interactive => self.config.global_interactive_queue,
                WorkloadClass::Batch => self.config.global_batch_queue,
            };
            if queue.len() >= limit {
                if matches!(mode, HybridMode::LocalFirst | HybridMode::Balanced) && cloud_available
                {
                    return Ok((LocalAdmission::OverflowToCloud, None));
                }
                return Err(AppError::RateLimited {
                    message: format!("global {:?} local queue limit exceeded", class),
                    retry_after_secs: retry_after_seconds(state.service_time_ms),
                });
            }
            state.next_id = state.next_id.saturating_add(1);
            let id = state.next_id;
            *state.queued_by_user.entry(user_id.to_owned()).or_default() += 1;
            let request = QueuedRequest {
                id,
                enqueued: Instant::now(),
                notify: notify.clone(),
            };
            match class {
                WorkloadClass::Interactive => state.interactive.push_back(request),
                WorkloadClass::Batch => state.batch.push_back(request),
            }
            id
        };

        self.notify_next();
        let wait = if mode == HybridMode::LocalStrict {
            self.config.strict_wait
        } else {
            self.config.overflow_after
        };
        let deadline = Instant::now() + wait;
        loop {
            if self.is_front(request_id, class, user_id)
                && let Ok(permit) = self.execution.clone().try_acquire_owned()
            {
                self.remove_queued(request_id, class, user_id);
                let mut state = self.state.lock().expect("local scheduler lock poisoned");
                *state.running_by_user.entry(user_id.to_owned()).or_default() += 1;
                drop(state);
                return Ok((
                    LocalAdmission::Acquired,
                    Some(LocalLease {
                        user_id: user_id.to_owned(),
                        started: Instant::now(),
                        permit: Some(permit),
                        scheduler: self.clone(),
                    }),
                ));
            }

            let now = Instant::now();
            if now >= deadline {
                self.remove_queued(request_id, class, user_id);
                if mode != HybridMode::LocalStrict && cloud_available {
                    return Ok((LocalAdmission::OverflowToCloud, None));
                }
                return Err(AppError::RateLimited {
                    message: "local_strict request exceeded the 60 second local wait limit"
                        .to_owned(),
                    retry_after_secs: self.retry_after(),
                });
            }
            let remaining = deadline.saturating_duration_since(now);
            let _ = tokio::time::timeout(remaining, notify.notified()).await;
        }
    }

    fn estimated_wait(&self, class: WorkloadClass) -> Duration {
        let state = self.state.lock().expect("local scheduler lock poisoned");
        let ahead = state.interactive.len()
            + usize::from(class == WorkloadClass::Batch) * state.batch.len()
            + state.running_by_user.values().sum::<usize>();
        Duration::from_millis(state.service_time_ms.saturating_mul(ahead as u64))
    }

    fn retry_after(&self) -> u64 {
        let state = self.state.lock().expect("local scheduler lock poisoned");
        retry_after_seconds(state.service_time_ms)
    }

    fn is_front(&self, id: u64, class: WorkloadClass, user_id: &str) -> bool {
        let state = self.state.lock().expect("local scheduler lock poisoned");
        if state.running_by_user.get(user_id).copied().unwrap_or(0)
            >= self.config.executing_per_user
        {
            return false;
        }
        match class {
            WorkloadClass::Interactive => {
                state.interactive.front().is_some_and(|item| item.id == id)
            }
            WorkloadClass::Batch => {
                state.interactive.is_empty()
                    && state.batch.front().is_some_and(|item| item.id == id)
            }
        }
    }

    fn remove_queued(&self, id: u64, class: WorkloadClass, user_id: &str) {
        let mut state = self.state.lock().expect("local scheduler lock poisoned");
        let queue = match class {
            WorkloadClass::Interactive => &mut state.interactive,
            WorkloadClass::Batch => &mut state.batch,
        };
        if let Some(index) = queue.iter().position(|item| item.id == id) {
            queue.remove(index);
        }
        if let Some(count) = state.queued_by_user.get_mut(user_id) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                state.queued_by_user.remove(user_id);
            }
        }
        drop(state);
        self.notify_next();
    }

    fn notify_next(&self) {
        let notify = {
            let state = self.state.lock().expect("local scheduler lock poisoned");
            state
                .interactive
                .front()
                .or_else(|| state.batch.front())
                .map(|request| request.notify.clone())
        };
        if let Some(notify) = notify {
            notify.notify_one();
        }
    }
}

impl Drop for LocalLease {
    fn drop(&mut self) {
        let elapsed_ms = self.started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        let mut state = self
            .scheduler
            .state
            .lock()
            .expect("local scheduler lock poisoned");
        if let Some(running) = state.running_by_user.get_mut(&self.user_id) {
            *running = running.saturating_sub(1);
            if *running == 0 {
                state.running_by_user.remove(&self.user_id);
            }
        }
        state.service_time_ms = if state.service_time_ms == 0 {
            elapsed_ms.max(1)
        } else {
            state
                .service_time_ms
                .saturating_mul(4)
                .saturating_add(elapsed_ms.max(1))
                / 5
        };
        drop(state);
        self.permit.take();
        self.scheduler.notify_next();
    }
}

pub(crate) fn order_attempts(
    attempts: Vec<ResolvedProvider>,
    mode: HybridMode,
) -> Vec<ResolvedProvider> {
    let mut local = Vec::new();
    let mut cloud = Vec::new();
    for attempt in attempts {
        match ProviderBoundary::for_resolved(&attempt) {
            ProviderBoundary::Local => local.push(attempt),
            ProviderBoundary::Cloud => cloud.push(attempt),
        }
    }
    match mode {
        HybridMode::LocalStrict => local,
        HybridMode::LocalFirst => local.into_iter().chain(cloud).collect(),
        HybridMode::Balanced => local.into_iter().chain(cloud).collect(),
        HybridMode::CloudFirst => cloud.into_iter().chain(local).collect(),
    }
}

pub(crate) fn provider_governance_metadata(provider: &ResolvedProvider) -> (String, String) {
    if ProviderBoundary::for_resolved(provider) == ProviderBoundary::Local {
        return ("local".to_owned(), "openai-compatible-v1".to_owned());
    }
    let host = reqwest::Url::parse(&provider.provider.base_url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .unwrap_or_default();
    let region = if host.ends_with(".cn") || host.contains("cn-") {
        "cn"
    } else if host.contains("eu-") || host.ends_with(".eu") {
        "eu"
    } else if host.contains("us-") || host.ends_with(".us") {
        "us"
    } else {
        "global"
    };
    let api_version = match provider.provider.protocol {
        crate::config::ProviderProtocol::Anthropic => "anthropic-v1",
        crate::config::ProviderProtocol::OpenaiCompat => "openai-compatible-v1",
    };
    (region.to_owned(), api_version.to_owned())
}

fn validate_change_input(input: &ChangeRequestInput) -> Result<(), AppError> {
    let supported = [
        "project_policy.upsert",
        "provider.allowlist_change",
        "routing.cloud_first",
        "budget.hard_limit",
        "identity.permission",
        "model.production_promotion",
        "data_egress.change",
        "database.major_migration",
        "secret.rotation",
    ];
    if !supported.contains(&input.action.as_str()) {
        return Err(AppError::InvalidRequest(
            "unsupported high-risk change action".to_owned(),
        ));
    }
    if input.target.trim().is_empty() || input.target.len() > 240 {
        return Err(AppError::InvalidRequest(
            "change request target must contain 1-240 characters".to_owned(),
        ));
    }
    if input.reason.trim().len() < 8 || input.reason.len() > 1_000 {
        return Err(AppError::InvalidRequest(
            "change request reason must contain 8-1000 characters".to_owned(),
        ));
    }
    let bytes = serde_json::to_vec(&input.payload)?;
    if bytes.len() > 64 * 1024 {
        return Err(AppError::InvalidRequest(
            "change request payload exceeds 64 KiB".to_owned(),
        ));
    }
    Ok(())
}

fn validate_project_policy(policy: &ProjectPolicy) -> Result<(), AppError> {
    for value in [
        &policy.organization_id,
        &policy.project_id,
        &policy.environment_id,
    ] {
        if !crate::domain::valid_tenant_identifier(value) {
            return Err(AppError::InvalidRequest(
                "project policy contains an invalid tenant identifier".to_owned(),
            ));
        }
    }
    if !policy.cloud_enabled && policy.maximum_mode != HybridMode::LocalStrict {
        return Err(AppError::InvalidRequest(
            "cloud-disabled project policy must use local_strict maximum mode".to_owned(),
        ));
    }
    for list in [
        &policy.allowed_providers,
        &policy.allowed_models,
        &policy.allowed_regions,
        &policy.allowed_api_versions,
    ] {
        if list.len() > 256
            || list
                .iter()
                .any(|value| value.trim().is_empty() || value.len() > 160)
        {
            return Err(AppError::InvalidRequest(
                "project policy allowlists contain invalid entries".to_owned(),
            ));
        }
    }
    Ok(())
}

fn policy_key(organization_id: &str, project_id: &str, environment_id: &str) -> String {
    format!("{organization_id}/{project_id}/{environment_id}")
}

fn is_local_address(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ip) => {
            ip.is_loopback() || ip.is_private() || ip.is_link_local() || ip.is_unspecified()
        }
        std::net::IpAddr::V6(ip) => {
            ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local()
        }
    }
}

fn matches_policy(rules: &[String], value: &str) -> bool {
    rules.iter().any(|rule| {
        rule == "*"
            || rule == value
            || rule
                .strip_suffix('*')
                .is_some_and(|prefix| value.starts_with(prefix))
    })
}

fn payload_sha256(payload: &Value) -> Result<String, AppError> {
    let canonical = canonical_json(payload);
    let bytes = serde_json::to_vec(&canonical)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn canonical_json(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let sorted = map
                .iter()
                .map(|(key, value)| (key.clone(), canonical_json(value)))
                .collect::<BTreeMap<_, _>>();
            serde_json::to_value(sorted).expect("canonical JSON object is serializable")
        }
        Value::Array(values) => Value::Array(values.iter().map(canonical_json).collect()),
        other => other.clone(),
    }
}

fn retry_after_seconds(service_time_ms: u64) -> u64 {
    service_time_ms.div_ceil(1_000).clamp(1, 60)
}

fn env_usize(name: &str, default: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(default)
}

fn env_u64(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tenant() -> TenantScope {
        TenantScope::from_strings("org_local", "prj_default", "env_default")
    }

    #[test]
    fn unknown_and_sensitive_data_are_always_local_strict() {
        let mut policy = ProjectPolicy::fail_closed(&tenant());
        policy.maximum_mode = HybridMode::CloudFirst;
        policy.cloud_enabled = true;
        assert_eq!(
            policy
                .effective_mode(Some(HybridMode::CloudFirst), DataClassification::Unknown)
                .unwrap(),
            HybridMode::LocalStrict
        );
        assert_eq!(
            policy
                .effective_mode(Some(HybridMode::Balanced), DataClassification::Sensitive)
                .unwrap(),
            HybridMode::LocalStrict
        );
    }

    #[test]
    fn callers_can_restrict_but_not_expand_project_mode() {
        let mut policy = ProjectPolicy::fail_closed(&tenant());
        policy.maximum_mode = HybridMode::Balanced;
        policy.cloud_enabled = true;
        assert_eq!(
            policy
                .effective_mode(Some(HybridMode::LocalStrict), DataClassification::Internal)
                .unwrap(),
            HybridMode::LocalStrict
        );
        assert!(
            policy
                .effective_mode(Some(HybridMode::CloudFirst), DataClassification::Internal)
                .is_err()
        );
    }

    #[test]
    fn dual_approval_requires_distinct_administrators() {
        let store = GovernanceStore::for_tests();
        let change = store
            .create_change_request(
                "usr_a",
                "admin-a",
                ChangeRequestInput {
                    action: "budget.hard_limit".to_owned(),
                    target: "org/project/prod".to_owned(),
                    payload: json!({"limitMicrounits": 1_000_000}),
                    reason: "raise production budget after review".to_owned(),
                },
            )
            .unwrap();
        assert!(
            store
                .approve_change_request(&change.id, "usr_a", "admin-a")
                .is_err()
        );
        let approved = store
            .approve_change_request(&change.id, "usr_b", "admin-b")
            .unwrap();
        assert_eq!(approved.status, "approved");
        assert_eq!(approved.approvals.len(), 2);
    }

    #[tokio::test]
    async fn local_scheduler_limits_per_user_queue_and_preserves_batch_priority() {
        let scheduler = LocalScheduler::new(LocalSchedulerConfig {
            executing_per_user: 1,
            queued_per_user: 2,
            global_interactive_queue: 16,
            global_batch_queue: 16,
            overflow_after: Duration::from_millis(20),
            strict_wait: Duration::from_millis(100),
        });
        let (_, lease) = scheduler
            .acquire(
                "usr_a",
                HybridMode::LocalStrict,
                WorkloadClass::Interactive,
                false,
            )
            .await
            .unwrap();
        let scheduler_one = scheduler.clone();
        let first = tokio::spawn(async move {
            scheduler_one
                .acquire(
                    "usr_a",
                    HybridMode::LocalStrict,
                    WorkloadClass::Interactive,
                    false,
                )
                .await
        });
        tokio::task::yield_now().await;
        let scheduler_two = scheduler.clone();
        let second = tokio::spawn(async move {
            scheduler_two
                .acquire(
                    "usr_a",
                    HybridMode::LocalStrict,
                    WorkloadClass::Interactive,
                    false,
                )
                .await
        });
        tokio::task::yield_now().await;
        let third = scheduler
            .acquire(
                "usr_a",
                HybridMode::LocalStrict,
                WorkloadClass::Interactive,
                false,
            )
            .await;
        assert!(matches!(third, Err(AppError::RateLimited { .. })));
        drop(lease);
        let first_lease = first.await.unwrap().unwrap().1.unwrap();
        drop(first_lease);
        drop(second.await.unwrap().unwrap().1);
    }

    #[tokio::test]
    async fn forty_user_baseline_caps_the_interactive_queue_at_sixteen() {
        let scheduler = LocalScheduler::new(LocalSchedulerConfig {
            executing_per_user: 1,
            queued_per_user: 2,
            global_interactive_queue: 16,
            global_batch_queue: 16,
            overflow_after: Duration::from_secs(5),
            strict_wait: Duration::from_secs(5),
        });
        let (_, running) = scheduler
            .acquire(
                "usr_00",
                HybridMode::LocalStrict,
                WorkloadClass::Interactive,
                false,
            )
            .await
            .unwrap();
        let mut queued = Vec::new();
        for index in 1..=16 {
            let scheduler = scheduler.clone();
            queued.push(tokio::spawn(async move {
                scheduler
                    .acquire(
                        &format!("usr_{index:02}"),
                        HybridMode::LocalStrict,
                        WorkloadClass::Interactive,
                        false,
                    )
                    .await
            }));
        }
        for _ in 0..100 {
            if scheduler.snapshot()["interactiveQueued"] == 16 {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert_eq!(scheduler.snapshot()["interactiveQueued"], 16);
        for index in 17..40 {
            let rejected = scheduler
                .acquire(
                    &format!("usr_{index:02}"),
                    HybridMode::LocalStrict,
                    WorkloadClass::Interactive,
                    false,
                )
                .await;
            assert!(matches!(rejected, Err(AppError::RateLimited { .. })));
        }
        drop(running);
        for task in queued {
            let lease = task.await.unwrap().unwrap().1.unwrap();
            drop(lease);
        }
    }

    #[tokio::test]
    async fn hybrid_request_overflows_when_estimated_wait_exceeds_five_seconds() {
        let scheduler = LocalScheduler::new(LocalSchedulerConfig::for_tests());
        let (_, running) = scheduler
            .acquire(
                "usr_running",
                HybridMode::LocalStrict,
                WorkloadClass::Interactive,
                false,
            )
            .await
            .unwrap();
        let queued_scheduler = scheduler.clone();
        let queued = tokio::spawn(async move {
            queued_scheduler
                .acquire(
                    "usr_queued",
                    HybridMode::LocalStrict,
                    WorkloadClass::Interactive,
                    false,
                )
                .await
        });
        for _ in 0..100 {
            if scheduler.snapshot()["interactiveQueued"] == 1 {
                break;
            }
            tokio::task::yield_now().await;
        }
        let overflow = scheduler
            .acquire(
                "usr_hybrid",
                HybridMode::LocalFirst,
                WorkloadClass::Interactive,
                true,
            )
            .await
            .unwrap();
        assert_eq!(overflow.0, LocalAdmission::OverflowToCloud);
        drop(running);
        drop(queued.await.unwrap().unwrap().1);
    }
}
