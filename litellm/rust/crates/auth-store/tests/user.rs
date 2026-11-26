//! A user's database read against a Prisma-migrated Postgres. Each row literal is what the
//! repository hands the Python proxy, and each entry literal is what Python caches for that row
//! under the bare user id. `tests/unit/redis/stores/test_auth_cache_user.py` checks both against
//! Python's own model and user reader, and `redis-store/tests/auth_cache_user.rs` round-trips the
//! entries through Redis. Change all three together.

mod support;

use std::sync::Arc;

use litellm_auth_store::{Error, UserDatabase};
use litellm_db_testing::MigratedPostgres;
use litellm_repositories::{Side, SignIn, UserRepository};
use rstest::{fixture, rstest};
use sqlx::PgPool;

use support::{database, dumped, execute};

const CREATED_ROW: &str = concat!(
    r#"{"user_id":"created","user_alias":null,"team_id":null,"sso_user_id":null,"#,
    r#""organization_id":null,"object_permission_id":null,"password_reset_required":null,"#,
    r#""last_breach_check_at":null,"teams":[],"user_role":null,"max_budget":null,"#,
    r#""budget_limits":null,"spend":0.0,"user_email":null,"models":[],"metadata":{},"#,
    r#""max_parallel_requests":null,"tpm_limit":null,"rpm_limit":null,"budget_duration":null,"#,
    r#""budget_reset_at":null,"allowed_cache_controls":[],"policies":[],"model_spend":{},"#,
    r#""model_max_budget":{},"created_at":"2026-11-01T00:00:00+00:00","#,
    r#""updated_at":"2026-11-01T00:00:00+00:00","organization_memberships":[]}"#,
);
const CREATED_ENTRY: &str = concat!(
    r#"{"user_id": "created", "user_alias": null, "team_id": null, "sso_user_id": null,"#,
    r#" "organization_id": null, "object_permission_id": null, "password_reset_required": null,"#,
    r#" "last_breach_check_at": null, "teams": [], "user_role": null, "max_budget": null,"#,
    r#" "budget_limits": null, "spend": 0.0, "user_email": null, "models": [], "metadata": {},"#,
    r#" "max_parallel_requests": null, "tpm_limit": null, "rpm_limit": null,"#,
    r#" "budget_duration": null, "budget_reset_at": null, "allowed_cache_controls": [],"#,
    r#" "policies": [], "model_spend": {}, "model_max_budget": {},"#,
    r#" "created_at": "2026-11-01T00:00:00Z", "updated_at": "2026-11-01T00:00:00Z","#,
    r#" "organization_memberships": [], "object_permission": null}"#,
);
const FULL_ROW: &str = concat!(
    r#"{"user_id":"full","user_alias":"Full","team_id":"team","sso_user_id":"sso","#,
    r#""organization_id":"org-a","object_permission_id":"permission","#,
    r#""password_reset_required":true,"last_breach_check_at":"2026-11-20T12:30:00.250+00:00","#,
    r#""teams":["team","other-team"],"user_role":"internal_user","max_budget":10.5,"#,
    r#""rollover_max_budget":20.0,"rollover_credit":2.5,"#,
    r#""budget_limits":[{"reset_at": "2026-12-01T00:00:00+00:00", "max_budget": 5,"#,
    r#" "window_start": "2026-11-30T00:00:00+00:00", "budget_duration": "1d"}],"spend":1.5,"#,
    r#""user_email":"full@example.com","models":["claude","gpt"],"#,
    r#""metadata":{"guardrails": ["pii"]},"max_parallel_requests":3,"tpm_limit":1000,"#,
    r#""rpm_limit":10,"budget_duration":"30d","budget_reset_at":"2026-12-01T00:00:00+00:00","#,
    r#""allowed_cache_controls":["no-cache"],"policies":["policy"],"#,
    r#""model_spend":{"claude": 1.5},"model_max_budget":{"claude": {"max_budget": 1}},"#,
    r#""created_at":"2026-11-01T00:00:00+00:00","updated_at":"2026-11-02T00:00:00+00:00","#,
    r#""organization_memberships":[{"user_id":"full","organization_id":"org-a","user_role":null,"#,
    r#""spend":0.0,"budget_id":null,"created_at":"2026-11-03T00:00:00+00:00","#,
    r#""updated_at":"2026-11-03T00:00:00+00:00"},{"user_id":"full","organization_id":"org-b","#,
    r#""user_role":"org_admin","spend":0.5,"budget_id":"budget","#,
    r#""created_at":"2026-11-03T00:00:00+00:00","updated_at":"2026-11-04T00:00:00.500+00:00"}]}"#,
);
const FULL_ENTRY: &str = concat!(
    r#"{"user_id": "full", "user_alias": "Full", "team_id": "team", "sso_user_id": "sso","#,
    r#" "organization_id": "org-a", "object_permission_id": "permission","#,
    r#" "password_reset_required": true, "last_breach_check_at": "2026-11-20T12:30:00.250000Z","#,
    r#" "teams": ["team", "other-team"], "user_role": "internal_user", "max_budget": 10.5,"#,
    r#" "rollover_max_budget": 20.0, "rollover_credit": 2.5,"#,
    r#" "budget_limits": [{"budget_duration": "1d", "max_budget": 5.0,"#,
    r#" "reset_at": "2026-12-01T00:00:00Z", "window_start": "2026-11-30T00:00:00Z"}],"#,
    r#" "spend": 1.5, "user_email": "full@example.com", "models": ["claude", "gpt"],"#,
    r#" "metadata": {"guardrails": ["pii"]}, "max_parallel_requests": 3, "tpm_limit": 1000,"#,
    r#" "rpm_limit": 10, "budget_duration": "30d", "budget_reset_at": "2026-12-01T00:00:00Z","#,
    r#" "allowed_cache_controls": ["no-cache"], "policies": ["policy"],"#,
    r#" "model_spend": {"claude": 1.5}, "model_max_budget": {"claude": {"max_budget": 1}},"#,
    r#" "created_at": "2026-11-01T00:00:00Z", "updated_at": "2026-11-02T00:00:00Z","#,
    r#" "organization_memberships": [{"user_id": "full", "organization_id": "org-a","#,
    r#" "user_role": null, "spend": 0.0, "budget_id": null,"#,
    r#" "created_at": "2026-11-03T00:00:00Z", "updated_at": "2026-11-03T00:00:00Z","#,
    r#" "user": null, "litellm_budget_table": null, "user_email": null}, {"user_id": "full","#,
    r#" "organization_id": "org-b", "user_role": "org_admin", "spend": 0.5,"#,
    r#" "budget_id": "budget", "created_at": "2026-11-03T00:00:00Z","#,
    r#" "updated_at": "2026-11-04T00:00:00.500000Z", "user": null, "litellm_budget_table": null,"#,
    r#" "user_email": null}], "object_permission": null}"#,
);

fn users(postgres: &MigratedPostgres) -> UserRepository {
    UserRepository::new(database(postgres))
}

/// A migrated Postgres holding a user inserted with only its id, and one with every column set
/// and a membership in two organizations, the later one by id inserted first.
#[fixture]
async fn postgres() -> MigratedPostgres {
    let postgres = MigratedPostgres::start().await.unwrap();
    insert_users(postgres.pool()).await;
    postgres
}

async fn insert_users(pool: &PgPool) {
    execute(
        pool,
        r#"INSERT INTO "LiteLLM_ObjectPermissionTable" (object_permission_id) VALUES ('permission')"#,
    )
    .await;
    execute(
        pool,
        r#"INSERT INTO "LiteLLM_BudgetTable" (budget_id, max_budget, created_by, updated_by)
           VALUES ('budget', 10.0, 'test', 'test')"#,
    )
    .await;
    execute(
        pool,
        r#"INSERT INTO "LiteLLM_OrganizationTable" (organization_id, organization_alias, budget_id, created_by, updated_by)
           VALUES ('org-a', 'A', 'budget', 'test', 'test'), ('org-b', 'B', 'budget', 'test', 'test')"#,
    )
    .await;
    execute(
        pool,
        r#"INSERT INTO "LiteLLM_UserTable" (user_id, created_at, updated_at)
           VALUES ('created', '2026-11-01 00:00:00', '2026-11-01 00:00:00')"#,
    )
    .await;
    execute(
        pool,
        r#"INSERT INTO "LiteLLM_UserTable" (
               user_id, user_alias, team_id, sso_user_id, organization_id, object_permission_id, password,
               password_reset_required, last_breach_check_at, teams, user_role, max_budget,
               rollover_max_budget, rollover_credit, budget_limits, spend, user_email, models, metadata,
               max_parallel_requests, tpm_limit, rpm_limit, budget_duration, budget_reset_at,
               allowed_cache_controls, policies, model_spend, model_max_budget, created_at, updated_at
           ) VALUES (
               'full', 'Full', 'team', 'sso', 'org-a', 'permission', 'hash',
               true, '2026-11-20 12:30:00.25', ARRAY['team', 'other-team'], 'internal_user', 10.5,
               20.0, 2.5,
               '[{"budget_duration": "1d", "max_budget": 5, "reset_at": "2026-12-01T00:00:00+00:00", "window_start": "2026-11-30T00:00:00+00:00"}]',
               1.5, 'full@example.com', ARRAY['claude', 'gpt'], '{"guardrails": ["pii"]}',
               3, 1000, 10, '30d', '2026-12-01 00:00:00',
               ARRAY['no-cache'], ARRAY['policy'], '{"claude": 1.5}', '{"claude": {"max_budget": 1}}',
               '2026-11-01 00:00:00', '2026-11-02 00:00:00'
           )"#,
    )
    .await;
    execute(
        pool,
        r#"INSERT INTO "LiteLLM_OrganizationMembership" (user_id, organization_id, user_role, spend, budget_id, created_at, updated_at)
           VALUES ('full', 'org-b', 'org_admin', 0.5, 'budget', '2026-11-03 00:00:00', '2026-11-04 00:00:00.5')"#,
    )
    .await;
    execute(
        pool,
        r#"INSERT INTO "LiteLLM_OrganizationMembership" (user_id, organization_id, created_at, updated_at)
           VALUES ('full', 'org-a', '2026-11-03 00:00:00', '2026-11-03 00:00:00')"#,
    )
    .await;
}

fn sign_in(user_id: &str) -> SignIn {
    SignIn {
        user_id: user_id.to_owned(),
        sso_user_id: None,
        user_email: None,
        email_verified: None,
        links_any_user: false,
    }
}

#[rstest]
#[case::as_created("created", CREATED_ROW, CREATED_ENTRY)]
#[case::every_column_set("full", FULL_ROW, FULL_ENTRY)]
#[tokio::test(flavor = "multi_thread")]
async fn a_user_read_from_the_database_is_the_entry_python_caches(
    #[future(awt)] postgres: MigratedPostgres,
    #[case] user_id: &str,
    #[case] row: &str,
    #[case] cached: &str,
) {
    let found = users(&postgres)
        .resolve(&sign_in(user_id), Side::Read)
        .await
        .unwrap()
        .unwrap();
    let read = UserDatabase::new(Arc::new(users(&postgres)))
        .user(user_id, Side::Read)
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
async fn a_user_no_row_has_is_not_found(#[future(awt)] postgres: MigratedPostgres) {
    let read = UserDatabase::new(Arc::new(users(&postgres)))
        .user("gone", Side::Read)
        .await
        .unwrap();

    assert_eq!(read, None);
}

#[rstest]
#[case::metadata_that_is_a_list(r#"UPDATE "LiteLLM_UserTable" SET metadata = '[1]'"#, "metadata")]
#[case::a_budget_window_without_its_max_budget(
    r#"UPDATE "LiteLLM_UserTable" SET budget_limits = '[{"budget_duration": "1d"}]'"#,
    "budget_limits"
)]
#[case::a_membership_without_its_spend(
    r#"UPDATE "LiteLLM_OrganizationMembership" SET spend = NULL WHERE organization_id = 'org-b'"#,
    "organization_memberships"
)]
#[case::a_membership_without_its_creation_time(
    r#"UPDATE "LiteLLM_OrganizationMembership" SET created_at = NULL WHERE organization_id = 'org-a'"#,
    "organization_memberships"
)]
#[tokio::test(flavor = "multi_thread")]
async fn a_user_row_the_model_refuses_fails_the_read(
    #[future(awt)] postgres: MigratedPostgres,
    #[case] corrupt: &'static str,
    #[case] column: &str,
) {
    execute(postgres.pool(), corrupt).await;

    let refused = UserDatabase::new(Arc::new(users(&postgres)))
        .user("full", Side::Read)
        .await;

    assert!(
        matches!(refused, Err(Error::Row { column: named }) if named == column),
        "{refused:?}"
    );
}
