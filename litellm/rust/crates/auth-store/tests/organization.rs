//! An organization's database read against a Prisma-migrated Postgres, with and without its
//! budget. Each row literal is what the repository hands the Python proxy, and each entry literal
//! is what Python caches for that row, under `org_id:{id}:with_budget` when the budget is read and
//! `org_id:{id}` when it is not. `tests/unit/redis/stores/test_auth_cache_organization.py` checks
//! both against Python's own model and organization reader, and
//! `redis-store/tests/auth_cache_organization.rs` round-trips the entries through Redis. Change all
//! three together.

mod support;

use std::sync::Arc;

use litellm_auth_store::{Error, OrganizationDatabase};
use litellm_db_testing::MigratedPostgres;
use litellm_repositories::OrganizationRepository;
use rstest::{fixture, rstest};
use sqlx::PgPool;

use support::{database, dumped, execute};

const CREATED_ROW: &str = concat!(
    r#"{"organization_id":"created","organization_alias":"Created","budget_id":"budget","#,
    r#""metadata":{},"models":[],"spend":0.0,"model_spend":{},"object_permission_id":null,"#,
    r#""created_by":"creator","updated_by":"creator","litellm_budget_table":null}"#,
);
const CREATED_ENTRY: &str = concat!(
    r#"{"organization_id": "created", "organization_alias": "Created", "budget_id": "budget","#,
    r#" "spend": 0.0, "metadata": {}, "models": [], "model_spend": {}, "created_by": "creator","#,
    r#" "updated_by": "creator", "users": null, "litellm_budget_table": null,"#,
    r#" "object_permission": null, "object_permission_id": null}"#,
);
const FULL_ROW: &str = concat!(
    r#"{"organization_id":"full","organization_alias":"Full","budget_id":"budget","#,
    r#""metadata":{"guardrails": ["pii"]},"models":["claude","all-proxy-models"],"spend":1.5,"#,
    r#""model_spend":{"claude": 1.5},"object_permission_id":"permission","created_by":"creator","#,
    r#""updated_by":"updater","litellm_budget_table":{"budget_id":"budget","name":"Organization","#,
    r#""max_budget":10.5,"rollover_max_budget":20.0,"soft_budget":8.0,"max_parallel_requests":3,"#,
    r#""tpm_limit":1000,"rpm_limit":10,"tpd_limit":100000,"#,
    r#""model_max_budget":{"claude": {"max_budget": 1}},"#,
    r#""model_tpm_limit":{"gpt": "7", "claude": 100.0},"model_rpm_limit":{"claude": 5},"#,
    r#""budget_duration":"30d","budget_limits":[],"cap_policy":"hard","#,
    r#""budget_reset_at":"2026-12-01T00:00:00+00:00","budget_reset_count":2,"#,
    r#""allowed_models":["claude"],"temp_budget_increase":1.5,"#,
    r#""temp_budget_expiry":"2026-11-20T12:30:00.250+00:00","#,
    r#""created_at":"2026-11-01T00:00:00+00:00","created_by":"test","#,
    r#""updated_at":"2026-11-02T00:00:00+00:00","updated_by":"test"}}"#,
);
const FULL_ENTRY: &str = concat!(
    r#"{"organization_id": "full", "organization_alias": "Full", "budget_id": "budget","#,
    r#" "spend": 1.5, "metadata": {"guardrails": ["pii"]}, "models": ["claude", "all-proxy-models"],"#,
    r#" "model_spend": {"claude": 1.5}, "created_by": "creator", "updated_by": "updater","#,
    r#" "users": null, "litellm_budget_table": {"budget_id": "budget", "soft_budget": 8.0,"#,
    r#" "max_budget": 10.5, "rollover_max_budget": 20.0, "max_parallel_requests": 3,"#,
    r#" "tpm_limit": 1000, "rpm_limit": 10, "tpd_limit": 100000,"#,
    r#" "model_max_budget": {"claude": {"max_budget": 1}}, "model_tpm_limit": {"gpt": 7,"#,
    r#" "claude": 100}, "model_rpm_limit": {"claude": 5}, "budget_duration": "30d","#,
    r#" "allowed_models": ["claude"], "temp_budget_increase": 1.5,"#,
    r#" "temp_budget_expiry": "2026-11-20T12:30:00.250000Z"}, "object_permission": null,"#,
    r#" "object_permission_id": "permission"}"#,
);
const FULL_WITHOUT_BUDGET_ROW: &str = concat!(
    r#"{"organization_id":"full","organization_alias":"Full","budget_id":"budget","#,
    r#""metadata":{"guardrails": ["pii"]},"models":["claude","all-proxy-models"],"spend":1.5,"#,
    r#""model_spend":{"claude": 1.5},"object_permission_id":"permission","created_by":"creator","#,
    r#""updated_by":"updater","litellm_budget_table":null}"#,
);
const FULL_WITHOUT_BUDGET_ENTRY: &str = concat!(
    r#"{"organization_id": "full", "organization_alias": "Full", "budget_id": "budget","#,
    r#" "spend": 1.5, "metadata": {"guardrails": ["pii"]}, "models": ["claude", "all-proxy-models"],"#,
    r#" "model_spend": {"claude": 1.5}, "created_by": "creator", "updated_by": "updater","#,
    r#" "users": null, "litellm_budget_table": null, "object_permission": null,"#,
    r#" "object_permission_id": "permission"}"#,
);

fn organizations(postgres: &MigratedPostgres) -> OrganizationRepository {
    OrganizationRepository::new(database(postgres))
}

/// A migrated Postgres holding an organization as `/organization/new` stores it when given only
/// an alias, and one with every column set, its budget's too.
#[fixture]
async fn postgres() -> MigratedPostgres {
    let postgres = MigratedPostgres::start().await.unwrap();
    insert_organizations(postgres.pool()).await;
    postgres
}

async fn insert_organizations(pool: &PgPool) {
    execute(
        pool,
        r#"INSERT INTO "LiteLLM_ObjectPermissionTable" (object_permission_id) VALUES ('permission')"#,
    )
    .await;
    execute(
        pool,
        r#"INSERT INTO "LiteLLM_BudgetTable" (
               budget_id, name, max_budget, rollover_max_budget, soft_budget, max_parallel_requests,
               tpm_limit, rpm_limit, tpd_limit, model_max_budget, model_tpm_limit, model_rpm_limit,
               budget_duration, budget_limits, cap_policy, budget_reset_at, budget_reset_count,
               allowed_models, temp_budget_increase, temp_budget_expiry, created_at, created_by,
               updated_at, updated_by
           ) VALUES (
               'budget', 'Organization', 10.5, 20.0, 8, 3,
               1000, 10, 100000, '{"claude": {"max_budget": 1}}', '{"claude": 100.0, "gpt": "7"}', '{"claude": 5}',
               '30d', '[]', 'hard', '2026-12-01 00:00:00', 2,
               ARRAY['claude'], 1.5, '2026-11-20 12:30:00.25', '2026-11-01 00:00:00', 'test',
               '2026-11-02 00:00:00', 'test'
           )"#,
    )
    .await;
    execute(
        pool,
        r#"INSERT INTO "LiteLLM_OrganizationTable" (organization_id, organization_alias, budget_id, created_by, updated_by)
           VALUES ('created', 'Created', 'budget', 'creator', 'creator')"#,
    )
    .await;
    execute(
        pool,
        r#"INSERT INTO "LiteLLM_OrganizationTable" (
               organization_id, organization_alias, budget_id, metadata, models, spend, model_spend,
               object_permission_id, created_by, updated_by
           ) VALUES (
               'full', 'Full', 'budget', '{"guardrails": ["pii"]}', ARRAY['claude', 'all-proxy-models'],
               1.5, '{"claude": 1.5}', 'permission', 'creator', 'updater'
           )"#,
    )
    .await;
}

#[rstest]
#[case::as_created("created", false, CREATED_ROW, CREATED_ENTRY)]
#[case::every_column_set_with_its_budget("full", true, FULL_ROW, FULL_ENTRY)]
#[case::every_column_set_without_its_budget(
    "full",
    false,
    FULL_WITHOUT_BUDGET_ROW,
    FULL_WITHOUT_BUDGET_ENTRY
)]
#[tokio::test(flavor = "multi_thread")]
async fn an_organization_read_from_the_database_is_the_entry_python_caches(
    #[future(awt)] postgres: MigratedPostgres,
    #[case] organization_id: &str,
    #[case] include_budget_table: bool,
    #[case] row: &str,
    #[case] cached: &str,
) {
    let found = organizations(&postgres)
        .find_with_optional_budget(organization_id, include_budget_table)
        .await
        .unwrap()
        .unwrap();
    let read = OrganizationDatabase::new(Arc::new(organizations(&postgres)))
        .organization(organization_id, include_budget_table)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        (
            serde_json::to_string(&found).unwrap().as_str(),
            dumped(read).as_str()
        ),
        (row, cached)
    );
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
async fn an_organization_no_row_has_is_not_found(#[future(awt)] postgres: MigratedPostgres) {
    let read = OrganizationDatabase::new(Arc::new(organizations(&postgres)))
        .organization("gone", true)
        .await
        .unwrap();

    assert_eq!(read, None);
}

#[rstest]
#[case::metadata_that_is_a_list(
    r#"UPDATE "LiteLLM_OrganizationTable" SET metadata = '[1]'"#,
    "metadata"
)]
#[case::model_spend_that_is_a_list(
    r#"UPDATE "LiteLLM_OrganizationTable" SET model_spend = '[1]'"#,
    "model_spend"
)]
#[case::a_budget_with_concurrent_windows(
    r#"UPDATE "LiteLLM_BudgetTable" SET budget_limits = '[{"budget_duration": "1d", "max_budget": 1}]'"#,
    "budget_limits"
)]
#[tokio::test(flavor = "multi_thread")]
async fn an_organization_row_the_model_refuses_fails_the_read(
    #[future(awt)] postgres: MigratedPostgres,
    #[case] corrupt: &'static str,
    #[case] column: &str,
) {
    execute(postgres.pool(), corrupt).await;

    let refused = OrganizationDatabase::new(Arc::new(organizations(&postgres)))
        .organization("full", true)
        .await;

    assert!(
        matches!(refused, Err(Error::Row { column: named }) if named == column),
        "{refused:?}"
    );
}
