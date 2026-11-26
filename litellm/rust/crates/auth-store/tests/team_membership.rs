//! A team membership's database read against a Prisma-migrated Postgres. Each row literal is what
//! the repository hands the Python proxy, and each entry literal is what Python caches for that
//! row. `tests/unit/redis/stores/test_auth_cache_team_membership.py` checks both against Python's
//! own model and membership readers, and `redis-store/tests/auth_cache_team_membership.rs`
//! round-trips the entries through Redis. Change all three together.

mod support;

use std::sync::Arc;

use litellm_auth_store::{Error, TeamMembershipDatabase};
use litellm_db_testing::MigratedPostgres;
use litellm_repositories::{Side, TeamMembershipRepository};
use rstest::{fixture, rstest};
use sqlx::PgPool;

use support::{database, dumped, execute};

const CREATED_ROW: &str = concat!(
    r#"{"user_id":"member","team_id":"created","budget_id":null,"spend":0.0,"total_spend":0.0,"#,
    r#""litellm_budget_table":null}"#,
);
const CREATED_ENTRY: &str = concat!(
    r#"{"user_id": "member", "team_id": "created", "budget_id": null, "spend": 0.0,"#,
    r#" "total_spend": 0.0, "litellm_budget_table": null}"#,
);
const FULL_ROW: &str = concat!(
    r#"{"user_id":"member","team_id":"full","budget_id":"budget","spend":1.5,"rollover_credit":2.5,"#,
    r#""total_spend":3.0,"litellm_budget_table":{"budget_id":"budget","name":"Member","#,
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
    r#"{"user_id": "member", "team_id": "full", "budget_id": "budget", "spend": 1.5,"#,
    r#" "total_spend": 3.0, "rollover_credit": 2.5, "litellm_budget_table": {"budget_id": "budget","#,
    r#" "soft_budget": 8.0, "max_budget": 10.5, "rollover_max_budget": 20.0,"#,
    r#" "max_parallel_requests": 3, "tpm_limit": 1000, "rpm_limit": 10, "tpd_limit": 100000,"#,
    r#" "model_max_budget": {"claude": {"max_budget": 1}}, "model_tpm_limit": {"gpt": 7,"#,
    r#" "claude": 100}, "model_rpm_limit": {"claude": 5}, "budget_duration": "30d","#,
    r#" "allowed_models": ["claude"], "temp_budget_increase": 1.5,"#,
    r#" "temp_budget_expiry": "2026-11-20T12:30:00.250000Z","#,
    r#" "budget_reset_at": "2026-12-01T00:00:00Z", "created_at": "2026-11-01T00:00:00Z"}}"#,
);

fn memberships(postgres: &MigratedPostgres) -> TeamMembershipRepository {
    TeamMembershipRepository::new(database(postgres))
}

fn membership_database(postgres: &MigratedPostgres) -> TeamMembershipDatabase {
    TeamMembershipDatabase::new(Arc::new(memberships(postgres)))
}

/// A migrated Postgres holding a membership as a member add stores it without a budget, and one
/// with every column set, its budget's too.
#[fixture]
async fn postgres() -> MigratedPostgres {
    let postgres = MigratedPostgres::start().await.unwrap();
    insert_memberships(postgres.pool()).await;
    postgres
}

async fn insert_memberships(pool: &PgPool) {
    execute(
        pool,
        r#"INSERT INTO "LiteLLM_TeamMembership" (user_id, team_id) VALUES ('member', 'created')"#,
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
               'budget', 'Member', 10.5, 20.0, 8, 3,
               1000, 10, 100000, '{"claude": {"max_budget": 1}}', '{"claude": 100.0, "gpt": "7"}', '{"claude": 5}',
               '30d', '[]', 'hard', '2026-12-01 00:00:00', 2,
               ARRAY['claude'], 1.5, '2026-11-20 12:30:00.25', '2026-11-01 00:00:00', 'test',
               '2026-11-02 00:00:00', 'test'
           )"#,
    )
    .await;
    execute(
        pool,
        r#"INSERT INTO "LiteLLM_TeamMembership" (user_id, team_id, budget_id, spend, rollover_credit, total_spend)
           VALUES ('member', 'full', 'budget', 1.5, 2.5, 3.0)"#,
    )
    .await;
}

#[rstest]
#[case::as_created("created", CREATED_ROW, CREATED_ENTRY)]
#[case::every_column_set("full", FULL_ROW, FULL_ENTRY)]
#[tokio::test(flavor = "multi_thread")]
async fn a_team_membership_read_from_the_database_is_the_entry_python_caches(
    #[future(awt)] postgres: MigratedPostgres,
    #[case] team_id: &str,
    #[case] row: &str,
    #[case] cached: &str,
) {
    let found = memberships(&postgres)
        .find_with_budget("member", team_id, Side::Read)
        .await
        .unwrap()
        .unwrap();
    let read = membership_database(&postgres)
        .team_membership("member", team_id, Side::Read)
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
async fn a_user_who_is_not_a_member_is_not_found(#[future(awt)] postgres: MigratedPostgres) {
    let read = membership_database(&postgres)
        .team_membership("member", "gone", Side::Read)
        .await
        .unwrap();

    assert_eq!(read, None);
}

#[rstest]
#[case::a_budget_with_concurrent_windows(
    r#"UPDATE "LiteLLM_BudgetTable" SET budget_limits = '[{"budget_duration": "1d", "max_budget": 1}]'"#,
    "budget_limits"
)]
#[case::a_budget_model_limit_that_is_no_whole_number(
    r#"UPDATE "LiteLLM_BudgetTable" SET model_tpm_limit = '{"claude": 1.5}'"#,
    "model_tpm_limit"
)]
#[tokio::test(flavor = "multi_thread")]
async fn a_team_membership_row_the_model_refuses_fails_the_read(
    #[future(awt)] postgres: MigratedPostgres,
    #[case] corrupt: &'static str,
    #[case] column: &str,
) {
    execute(postgres.pool(), corrupt).await;

    let refused = membership_database(&postgres)
        .team_membership("member", "full", Side::Read)
        .await;

    assert!(
        matches!(refused, Err(Error::Row { column: named }) if named == column),
        "{refused:?}"
    );
}
