use sqlx::{Connection, Executor, PgConnection, postgres::PgPoolOptions};
use uuid::Uuid;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[tokio::test]
async fn current_schema_migration_preserves_legacy_requests_and_attempts() {
    let Ok(database_url) = std::env::var("MODELPORT_TEST_DATABASE_URL") else {
        return;
    };
    let mut connection = PgConnection::connect(&database_url)
        .await
        .expect("connect to MODELPORT_TEST_DATABASE_URL");
    let schema = format!("modelport_migration_{}", Uuid::new_v4().simple());
    connection
        .execute(format!("CREATE SCHEMA {schema}").as_str())
        .await
        .expect("create isolated migration schema");
    connection
        .execute(format!("SET search_path TO {schema}").as_str())
        .await
        .expect("select isolated migration schema");

    let migration_result = async {
        sqlx::raw_sql(include_str!("../migrations/0001_enterprise_foundation.sql"))
            .execute(&mut connection)
            .await?;
        sqlx::raw_sql(include_str!(
            "../migrations/0002_idempotency_and_leases.sql"
        ))
        .execute(&mut connection)
        .await?;
        sqlx::raw_sql(include_str!("../migrations/0003_transactional_budgets.sql"))
            .execute(&mut connection)
            .await?;
        sqlx::raw_sql(include_str!(
            "../migrations/0004_redact_historical_error_details.sql"
        ))
        .execute(&mut connection)
        .await?;

        sqlx::query(
            "INSERT INTO modelport_gateway_requests (
                ledger_id, request_id,
                organization_id, project_id, environment_id,
                principal_id, client_protocol, requested_model, stream,
                created_at, updated_at, completed_at
            ) VALUES (
                'ldr_legacy', 'req_legacy',
                'org_local', 'prj_default', 'env_default',
                'usr_legacy', 'anthropic-messages', 'auto', false,
                '2026-07-01T00:00:00Z',
                '2026-07-01T00:00:03Z',
                '2026-07-01T00:00:03Z'
            )",
        )
        .execute(&mut connection)
        .await?;
        sqlx::query(
            "INSERT INTO modelport_provider_attempts (
                attempt_id, request_ledger_id,
                organization_id, project_id, environment_id,
                provider_id, resolved_model, provider_protocol,
                created_at, updated_at, completed_at
            ) VALUES
            (
                'att_legacy_1', 'ldr_legacy',
                'org_local', 'prj_default', 'env_default',
                'provider_a', 'model_a', 'openai-compat',
                '2026-07-01T00:00:00Z',
                '2026-07-01T00:00:01Z',
                '2026-07-01T00:00:01Z'
            ),
            (
                'att_legacy_2', 'ldr_legacy',
                'org_local', 'prj_default', 'env_default',
                'provider_b', 'model_b', 'anthropic',
                '2026-07-01T00:00:01Z',
                '2026-07-01T00:00:03Z',
                '2026-07-01T00:00:03Z'
            )",
        )
        .execute(&mut connection)
        .await?;

        sqlx::raw_sql(include_str!(
            "../migrations/0005_current_operational_schema.sql"
        ))
        .execute(&mut connection)
        .await?;
        sqlx::raw_sql(include_str!(
            "../migrations/0006_operational_query_indexes.sql"
        ))
        .execute(&mut connection)
        .await?;
        sqlx::raw_sql(include_str!(
            "../migrations/0007_smart_routing_decisions.sql"
        ))
        .execute(&mut connection)
        .await?;

        let request = sqlx::query_as::<_, (String, String, String, i32, Option<String>, i64)>(
            "SELECT
                username,
                request_path,
                provider_id,
                retry_count,
                fallback_from_provider,
                latency_ms
             FROM modelport_gateway_requests
             WHERE ledger_id = 'ldr_legacy'",
        )
        .fetch_one(&mut connection)
        .await?;
        let attempt_count =
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM modelport_provider_attempts")
                .fetch_one(&mut connection)
                .await?;
        let last_attempt = sqlx::query_as::<_, (i32, Option<String>, i64)>(
            "SELECT retry_count, fallback_from_provider, latency_ms
             FROM modelport_provider_attempts
             WHERE attempt_id = 'att_legacy_2'",
        )
        .fetch_one(&mut connection)
        .await?;
        let routing_table_exists = sqlx::query_scalar::<_, bool>(
            "SELECT to_regclass('modelport_routing_decisions') IS NOT NULL",
        )
        .fetch_one(&mut connection)
        .await?;
        Ok::<_, sqlx::Error>((request, attempt_count, last_attempt, routing_table_exists))
    }
    .await;

    connection
        .execute("SET search_path TO public")
        .await
        .expect("restore search path");
    connection
        .execute(format!("DROP SCHEMA {schema} CASCADE").as_str())
        .await
        .expect("remove isolated migration schema");

    let (request, attempt_count, last_attempt, routing_table_exists) =
        migration_result.expect("apply migrations to populated legacy schema");
    assert_eq!(request.0, "usr_legacy");
    assert_eq!(request.1, "/v1/messages");
    assert_eq!(request.2, "provider_b");
    assert_eq!(request.3, 1);
    assert_eq!(request.4.as_deref(), Some("provider_a"));
    assert_eq!(request.5, 3_000);
    assert_eq!(attempt_count, 2);
    assert_eq!(last_attempt.0, 1);
    assert_eq!(last_attempt.1.as_deref(), Some("provider_a"));
    assert_eq!(last_attempt.2, 2_000);
    assert!(routing_table_exists);
}

#[tokio::test]
async fn embedded_migrations_preserve_restored_operational_row_counts() {
    let Ok(database_url) = std::env::var("MODELPORT_MIGRATION_DRILL_DATABASE_URL") else {
        return;
    };
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .expect("connect to restored migration-drill database");
    let requests_before =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM modelport_gateway_requests")
            .fetch_one(&pool)
            .await
            .expect("count requests before migration");
    let attempts_before =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM modelport_provider_attempts")
            .fetch_one(&pool)
            .await
            .expect("count attempts before migration");

    MIGRATOR
        .run(&pool)
        .await
        .expect("apply embedded migrations to restored database");

    let requests_after =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM modelport_gateway_requests")
            .fetch_one(&pool)
            .await
            .expect("count requests after migration");
    let attempts_after =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM modelport_provider_attempts")
            .fetch_one(&pool)
            .await
            .expect("count attempts after migration");
    let incomplete_backfill = sqlx::query_scalar::<_, i64>(
        "SELECT count(*)
         FROM modelport_gateway_requests
         WHERE username IS NULL
            OR request_path IS NULL
            OR traffic_class IS NULL
            OR tool_use_requested IS NULL",
    )
    .fetch_one(&pool)
    .await
    .expect("verify required operational backfill");

    assert_eq!(requests_after, requests_before);
    assert_eq!(attempts_after, attempts_before);
    assert_eq!(incomplete_backfill, 0);
}
