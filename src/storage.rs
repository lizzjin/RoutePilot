use std::{
    fs,
    fs::{File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
};

use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use sqlx::{Executor, PgPool, Postgres};

#[cfg(test)]
use std::sync::Arc;

use crate::{
    database::{connect_pool, database_url, redact_database_url},
    error::AppError,
};

const STATE_TABLE: &str = "modelport_state";
const STATE_SCHEMA_LOCK_KEY: i64 = 0x4d4f_4445_4c50_4f52;
static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub enum JsonStore {
    #[cfg(test)]
    File(PathBuf),
    #[cfg(test)]
    Memory {
        namespace: String,
        state: Arc<Mutex<VersionedValue>>,
    },
    Postgres(PostgresJsonStore),
}

pub struct PostgresJsonStore {
    namespace: String,
    database_url: String,
    location: String,
    worker: Mutex<mpsc::Sender<PostgresCommand>>,
}

#[derive(Debug, Clone)]
pub(crate) struct VersionedValue {
    pub(crate) value: Option<Value>,
    pub(crate) revision: u64,
}

#[derive(Debug)]
struct StateWrite {
    namespace: String,
    expected_revision: u64,
    value: Value,
}

enum PostgresCommand {
    Read {
        respond_to: mpsc::Sender<Result<VersionedValue, String>>,
    },
    CompareAndSwap {
        value: Value,
        expected_revision: u64,
        respond_to: mpsc::Sender<Result<u64, AppError>>,
    },
    CompareAndSwapMany {
        writes: Vec<StateWrite>,
        respond_to: mpsc::Sender<Result<Vec<u64>, AppError>>,
    },
}

impl std::fmt::Debug for PostgresJsonStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PostgresJsonStore")
            .field("namespace", &self.namespace)
            .field("location", &self.location)
            .finish_non_exhaustive()
    }
}

impl JsonStore {
    pub fn open(namespace: &str) -> Result<Self, AppError> {
        let database_url = database_url().ok_or_else(|| {
            AppError::Config(
                "MODELPORT_DATABASE_URL is required; current releases store auth and control state in PostgreSQL"
                    .to_owned(),
            )
        })?;

        let worker = spawn_postgres_worker(database_url.clone(), namespace.to_owned())?;

        Ok(Self::Postgres(PostgresJsonStore {
            namespace: namespace.to_owned(),
            database_url: database_url.clone(),
            location: format!(
                "{}#{}:{}",
                redact_database_url(&database_url),
                STATE_TABLE,
                namespace
            ),
            worker: Mutex::new(worker),
        }))
    }

    pub fn read_versioned_or_default<T>(&self, default: Value) -> Result<(T, u64), AppError>
    where
        T: DeserializeOwned,
    {
        let document = self.read_versioned()?;
        let value = document.value.unwrap_or(default);
        Ok((serde_json::from_value(value)?, document.revision))
    }

    pub fn read_value(&self) -> Result<Option<Value>, AppError> {
        self.read_versioned().map(|document| document.value)
    }

    pub(crate) fn read_versioned(&self) -> Result<VersionedValue, AppError> {
        match self {
            #[cfg(test)]
            Self::File(path) => {
                if !path.exists() {
                    return Ok(VersionedValue {
                        value: None,
                        revision: 0,
                    });
                }
                let value = serde_json::from_str(&fs::read_to_string(path)?)?;
                Ok(VersionedValue {
                    value: Some(value),
                    revision: 0,
                })
            }
            #[cfg(test)]
            Self::Memory { state, .. } => {
                Ok(state.lock().expect("memory store lock poisoned").clone())
            }
            Self::Postgres(store) => {
                let (respond_to, response) = mpsc::channel();
                store
                    .worker
                    .lock()
                    .expect("postgres worker lock poisoned")
                    .send(PostgresCommand::Read { respond_to })
                    .map_err(|err| AppError::Database(format!("postgres worker stopped: {err}")))?;
                response
                    .recv()
                    .map_err(|err| AppError::Database(format!("postgres worker stopped: {err}")))?
                    .map_err(AppError::Database)
            }
        }
    }

    pub fn compare_and_swap_json<T>(
        &self,
        expected_revision: u64,
        value: &T,
    ) -> Result<u64, AppError>
    where
        T: Serialize,
    {
        self.compare_and_swap_value(expected_revision, &serde_json::to_value(value)?)
    }

    pub fn compare_and_swap_value(
        &self,
        expected_revision: u64,
        value: &Value,
    ) -> Result<u64, AppError> {
        match self {
            #[cfg(test)]
            Self::File(path) => {
                write_json_file_atomic(path, value)?;
                Ok(expected_revision)
            }
            #[cfg(test)]
            Self::Memory { namespace, state } => {
                let mut document = state.lock().expect("memory store lock poisoned");
                if document.revision != expected_revision {
                    return Err(state_conflict(namespace, expected_revision));
                }
                document.value = Some(value.clone());
                document.revision = document
                    .revision
                    .checked_add(1)
                    .ok_or_else(|| AppError::Database("state revision exhausted".to_owned()))?;
                Ok(document.revision)
            }
            Self::Postgres(store) => {
                let (respond_to, response) = mpsc::channel();
                store
                    .worker
                    .lock()
                    .expect("postgres worker lock poisoned")
                    .send(PostgresCommand::CompareAndSwap {
                        value: value.clone(),
                        expected_revision,
                        respond_to,
                    })
                    .map_err(|err| AppError::Database(format!("postgres worker stopped: {err}")))?;
                response
                    .recv()
                    .map_err(|err| AppError::Database(format!("postgres worker stopped: {err}")))?
            }
        }
    }

    pub fn compare_and_swap_pair(
        first: (&Self, u64, &Value),
        second: (&Self, u64, &Value),
    ) -> Result<(u64, u64), AppError> {
        let (first_store, second_store) = match (first.0, second.0) {
            (Self::Postgres(first_store), Self::Postgres(second_store)) => {
                (first_store, second_store)
            }
            #[cfg(test)]
            _ => {
                return Err(AppError::Config(
                    "atomic state restore requires PostgreSQL stores".to_owned(),
                ));
            }
        };
        if first_store.database_url != second_store.database_url {
            return Err(AppError::Config(
                "atomic state restore requires auth and control to use the same PostgreSQL database"
                    .to_owned(),
            ));
        }
        if first_store.namespace == second_store.namespace {
            return Err(AppError::InvalidRequest(
                "atomic state restore requires distinct namespaces".to_owned(),
            ));
        }

        let writes = vec![
            StateWrite {
                namespace: first_store.namespace.clone(),
                expected_revision: first.1,
                value: first.2.clone(),
            },
            StateWrite {
                namespace: second_store.namespace.clone(),
                expected_revision: second.1,
                value: second.2.clone(),
            },
        ];
        let (respond_to, response) = mpsc::channel();
        first_store
            .worker
            .lock()
            .expect("postgres worker lock poisoned")
            .send(PostgresCommand::CompareAndSwapMany { writes, respond_to })
            .map_err(|err| AppError::Database(format!("postgres worker stopped: {err}")))?;
        let revisions = response
            .recv()
            .map_err(|err| AppError::Database(format!("postgres worker stopped: {err}")))??;
        match revisions.as_slice() {
            [first_revision, second_revision] => Ok((*first_revision, *second_revision)),
            _ => Err(AppError::Database(
                "atomic state restore returned an invalid revision count".to_owned(),
            )),
        }
    }

    pub fn location(&self) -> String {
        match self {
            #[cfg(test)]
            Self::File(path) => path.to_string_lossy().into_owned(),
            #[cfg(test)]
            Self::Memory { namespace, .. } => format!("memory://{namespace}"),
            Self::Postgres(store) => store.location.clone(),
        }
    }
}

pub(crate) fn write_json_file_atomic(path: &Path, value: &Value) -> Result<(), AppError> {
    let contents = serde_json::to_vec_pretty(value)?;
    let parent = parent_directory(path);
    fs::create_dir_all(parent)?;

    let (temporary_path, temporary_file) = create_secure_temporary_file(path, parent)?;
    let result = write_and_replace(temporary_file, &temporary_path, path, parent, &contents);
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}

fn parent_directory(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn create_secure_temporary_file(path: &Path, parent: &Path) -> io::Result<(PathBuf, File)> {
    let file_name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "JSON store path must include a file name",
        )
    })?;

    for _ in 0..16 {
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary_name = format!(
            ".{}.{}.{}.tmp",
            file_name.to_string_lossy(),
            std::process::id(),
            sequence
        );
        let temporary_path = parent.join(temporary_name);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }

        match options.open(&temporary_path) {
            Ok(file) => return Ok((temporary_path, file)),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique temporary JSON store file",
    ))
}

fn write_and_replace(
    mut temporary_file: File,
    temporary_path: &Path,
    destination: &Path,
    parent: &Path,
    contents: &[u8],
) -> Result<(), AppError> {
    temporary_file.write_all(contents)?;
    temporary_file.flush()?;
    temporary_file.sync_all()?;
    drop(temporary_file);

    fs::rename(temporary_path, destination)?;
    sync_directory(parent)?;
    Ok(())
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> io::Result<()> {
    Ok(())
}

async fn initialize_postgres(pool: &PgPool) -> Result<(), AppError> {
    let mut transaction = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(STATE_SCHEMA_LOCK_KEY)
        .execute(&mut *transaction)
        .await?;
    sqlx::query(&format!(
        "CREATE TABLE IF NOT EXISTS {STATE_TABLE} (
            namespace TEXT PRIMARY KEY,
            document JSONB NOT NULL,
            revision BIGINT NOT NULL DEFAULT 0 CHECK (revision >= 0),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
        )"
    ))
    .execute(&mut *transaction)
    .await?;
    sqlx::query(&format!(
        "ALTER TABLE {STATE_TABLE}
         ADD COLUMN IF NOT EXISTS revision BIGINT NOT NULL DEFAULT 0"
    ))
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(())
}

fn spawn_postgres_worker(
    database_url: String,
    namespace: String,
) -> Result<mpsc::Sender<PostgresCommand>, AppError> {
    let (command_sender, command_receiver) = mpsc::channel::<PostgresCommand>();
    let (ready_sender, ready_receiver) = mpsc::channel::<Result<(), String>>();
    let thread_name = format!("modelport-postgres-{namespace}");
    thread::Builder::new().name(thread_name).spawn({
        let namespace = namespace.clone();
        move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(err) => {
                    let _ = ready_sender
                        .send(Err(format!("could not create PostgreSQL runtime: {err}")));
                    return;
                }
            };
            let pool = match runtime.block_on(connect_and_initialize(&database_url)) {
                Ok(pool) => {
                    let _ = ready_sender.send(Ok(()));
                    pool
                }
                Err(err) => {
                    let _ = ready_sender.send(Err(err.to_string()));
                    return;
                }
            };

            for command in command_receiver {
                match command {
                    PostgresCommand::Read { respond_to } => {
                        let result = runtime
                            .block_on(read_postgres_value(&pool, &namespace))
                            .map_err(|err| err.to_string());
                        let _ = respond_to.send(result);
                    }
                    PostgresCommand::CompareAndSwap {
                        value,
                        expected_revision,
                        respond_to,
                    } => {
                        let result = runtime.block_on(compare_and_swap_postgres_value(
                            &pool,
                            &namespace,
                            expected_revision,
                            &value,
                        ));
                        let _ = respond_to.send(result);
                    }
                    PostgresCommand::CompareAndSwapMany { writes, respond_to } => {
                        let result =
                            runtime.block_on(compare_and_swap_postgres_values(&pool, &writes));
                        let _ = respond_to.send(result);
                    }
                }
            }
        }
    })?;

    ready_receiver
        .recv()
        .map_err(|err| AppError::Database(format!("postgres worker failed to start: {err}")))?
        .map_err(AppError::Database)?;

    Ok(command_sender)
}

async fn connect_and_initialize(database_url: &str) -> Result<PgPool, AppError> {
    let pool = connect_pool(database_url, Some(1)).await?;
    initialize_postgres(&pool).await?;
    Ok(pool)
}

async fn read_postgres_value(pool: &PgPool, namespace: &str) -> Result<VersionedValue, AppError> {
    let row = sqlx::query_as::<_, (Value, i64)>(&format!(
        "SELECT document, revision FROM {STATE_TABLE} WHERE namespace = $1"
    ))
    .bind(namespace)
    .fetch_optional(pool)
    .await?;
    let Some((value, revision)) = row else {
        return Ok(VersionedValue {
            value: None,
            revision: 0,
        });
    };
    Ok(VersionedValue {
        value: Some(value),
        revision: revision.try_into().map_err(|_| {
            AppError::Database(format!(
                "{STATE_TABLE}.{namespace} contains a negative revision"
            ))
        })?,
    })
}

async fn compare_and_swap_postgres_value<'executor, E>(
    executor: E,
    namespace: &str,
    expected_revision: u64,
    value: &Value,
) -> Result<u64, AppError>
where
    E: Executor<'executor, Database = Postgres>,
{
    let expected_revision = i64::try_from(expected_revision)
        .map_err(|_| AppError::Database("state revision exceeds PostgreSQL BIGINT".to_owned()))?;
    let revision = sqlx::query_scalar::<_, i64>(&format!(
        "WITH updated AS ( \
             UPDATE {STATE_TABLE} \
             SET document = $2, revision = revision + 1, updated_at = now() \
             WHERE namespace = $1 AND revision = $3 \
             RETURNING revision \
         ), inserted AS ( \
             INSERT INTO {STATE_TABLE} (namespace, document, revision, updated_at) \
             SELECT $1, $2, 1, now() \
             WHERE $3 = 0 \
               AND NOT EXISTS ( \
                   SELECT 1 FROM {STATE_TABLE} WHERE namespace = $1 \
               ) \
             ON CONFLICT (namespace) DO NOTHING \
             RETURNING revision \
         ) \
         SELECT revision FROM updated \
         UNION ALL \
         SELECT revision FROM inserted"
    ))
    .bind(namespace)
    .bind(value)
    .bind(expected_revision)
    .fetch_optional(executor)
    .await?;
    let Some(revision) = revision else {
        return Err(state_conflict(
            namespace,
            expected_revision.try_into().unwrap_or(u64::MAX),
        ));
    };
    revision
        .try_into()
        .map_err(|_| AppError::Database("PostgreSQL returned a negative state revision".to_owned()))
}

async fn compare_and_swap_postgres_values(
    pool: &PgPool,
    writes: &[StateWrite],
) -> Result<Vec<u64>, AppError> {
    let mut transaction = pool.begin().await?;
    let mut revisions = Vec::with_capacity(writes.len());
    for write in writes {
        revisions.push(
            compare_and_swap_postgres_value(
                &mut *transaction,
                &write.namespace,
                write.expected_revision,
                &write.value,
            )
            .await?,
        );
    }
    transaction.commit().await?;
    Ok(revisions)
}

fn state_conflict(namespace: &str, expected_revision: u64) -> AppError {
    AppError::StateConflict(format!(
        "{namespace} state changed after revision {expected_revision}; reload the latest state before retrying"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temporary_test_directory(label: &str) -> PathBuf {
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "modelport-storage-{label}-{}-{sequence}",
            std::process::id()
        ))
    }

    #[test]
    fn state_cas_rejects_a_stale_memory_writer() {
        let state = Arc::new(Mutex::new(VersionedValue {
            value: None,
            revision: 0,
        }));
        let first = JsonStore::Memory {
            namespace: "control".to_owned(),
            state: Arc::clone(&state),
        };
        let second = JsonStore::Memory {
            namespace: "control".to_owned(),
            state,
        };
        let first_snapshot = first.read_versioned().unwrap();
        let second_snapshot = second.read_versioned().unwrap();

        assert_eq!(
            first
                .compare_and_swap_value(first_snapshot.revision, &json!({ "writer": "first" }))
                .unwrap(),
            1
        );
        assert!(matches!(
            second.compare_and_swap_value(second_snapshot.revision, &json!({ "writer": "second" })),
            Err(AppError::StateConflict(_))
        ));
        assert_eq!(
            first.read_value().unwrap().unwrap(),
            json!({ "writer": "first" })
        );
    }

    #[tokio::test]
    async fn postgres_state_cas_allows_exactly_one_concurrent_writer() {
        let Ok(database_url) = std::env::var("MODELPORT_TEST_DATABASE_URL") else {
            return;
        };
        let pool = connect_pool(&database_url, Some(4))
            .await
            .expect("connect to MODELPORT_TEST_DATABASE_URL");
        initialize_postgres(&pool)
            .await
            .expect("initialize versioned state table");
        let namespace = format!("test_cas_{}", uuid::Uuid::new_v4().simple());

        let left_value = json!({ "writer": "left" });
        let right_value = json!({ "writer": "right" });
        let left = compare_and_swap_postgres_value(&pool, &namespace, 0, &left_value);
        let right = compare_and_swap_postgres_value(&pool, &namespace, 0, &right_value);
        let (left, right) = tokio::join!(left, right);
        assert_eq!(
            usize::from(left.is_ok()) + usize::from(right.is_ok()),
            1,
            "exactly one writer must win the initial revision"
        );
        assert_eq!(
            usize::from(matches!(left, Err(AppError::StateConflict(_))))
                + usize::from(matches!(right, Err(AppError::StateConflict(_)))),
            1,
            "the stale writer must receive a state conflict"
        );

        let stored = read_postgres_value(&pool, &namespace)
            .await
            .expect("read winning state");
        assert_eq!(stored.revision, 1);
        assert!(matches!(
            stored.value,
            Some(value)
                if value == json!({ "writer": "left" })
                    || value == json!({ "writer": "right" })
        ));

        sqlx::query(&format!("DELETE FROM {STATE_TABLE} WHERE namespace = $1"))
            .bind(&namespace)
            .execute(&pool)
            .await
            .expect("remove test state");
    }

    #[tokio::test]
    async fn postgres_state_batch_cas_is_atomic() {
        let Ok(database_url) = std::env::var("MODELPORT_TEST_DATABASE_URL") else {
            return;
        };
        let pool = connect_pool(&database_url, Some(2))
            .await
            .expect("connect to MODELPORT_TEST_DATABASE_URL");
        initialize_postgres(&pool)
            .await
            .expect("initialize versioned state table");
        let suffix = uuid::Uuid::new_v4().simple();
        let auth_namespace = format!("test_auth_{suffix}");
        let control_namespace = format!("test_control_{suffix}");
        compare_and_swap_postgres_value(
            &pool,
            &auth_namespace,
            0,
            &json!({ "version": "auth-original" }),
        )
        .await
        .unwrap();
        compare_and_swap_postgres_value(
            &pool,
            &control_namespace,
            0,
            &json!({ "version": "control-original" }),
        )
        .await
        .unwrap();

        let conflict = compare_and_swap_postgres_values(
            &pool,
            &[
                StateWrite {
                    namespace: auth_namespace.clone(),
                    expected_revision: 1,
                    value: json!({ "version": "auth-restored" }),
                },
                StateWrite {
                    namespace: control_namespace.clone(),
                    expected_revision: 0,
                    value: json!({ "version": "control-restored" }),
                },
            ],
        )
        .await;
        assert!(matches!(conflict, Err(AppError::StateConflict(_))));
        let auth_after_conflict = read_postgres_value(&pool, &auth_namespace).await.unwrap();
        assert_eq!(auth_after_conflict.revision, 1);
        assert_eq!(
            auth_after_conflict.value.unwrap(),
            json!({ "version": "auth-original" })
        );

        let revisions = compare_and_swap_postgres_values(
            &pool,
            &[
                StateWrite {
                    namespace: auth_namespace.clone(),
                    expected_revision: 1,
                    value: json!({ "version": "auth-restored" }),
                },
                StateWrite {
                    namespace: control_namespace.clone(),
                    expected_revision: 1,
                    value: json!({ "version": "control-restored" }),
                },
            ],
        )
        .await
        .unwrap();
        assert_eq!(revisions, vec![2, 2]);

        sqlx::query(&format!(
            "DELETE FROM {STATE_TABLE} WHERE namespace = ANY($1)"
        ))
        .bind(vec![auth_namespace, control_namespace])
        .execute(&pool)
        .await
        .expect("remove batch test state");
    }

    #[test]
    fn atomic_json_write_replaces_content_without_leaving_temporary_files() {
        let directory = temporary_test_directory("atomic");
        let path = directory.join("state.json");

        write_json_file_atomic(&path, &json!({ "version": 1 })).unwrap();
        write_json_file_atomic(&path, &json!({ "version": 2 })).unwrap();

        let stored: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(stored, json!({ "version": 2 }));
        let entries = fs::read_dir(&directory)
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path(), path);

        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn atomic_json_write_enforces_owner_only_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let directory = temporary_test_directory("permissions");
        let path = directory.join("state.json");
        write_json_file_atomic(&path, &json!({ "secure": true })).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

        write_json_file_atomic(&path, &json!({ "secure": "replaced" })).unwrap();

        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        fs::remove_dir_all(directory).unwrap();
    }
}
