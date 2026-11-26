//! The auth cache's typed team membership entry on real Redis. The entry literals are the ones
//! `tests/unit/redis/stores/test_auth_cache_team_membership.py` checks against Python's own model
//! and membership readers, and `auth-store/tests/team_membership.rs` finds after a database read.
//! Change all three together.

mod support;

use std::time::Duration;

use litellm_redis_client::commands;
use litellm_redis_store::{
    FromValue, IntoValue,
    auth_cache::{AuthCache, AuthObject, CachedTeamMembership, Settings},
};
use litellm_redis_testing::{RedisDeployment, Topology};
use rstest::rstest;
use support::{connect, remote};

const TTL: Duration = Duration::from_secs(60);

const CREATED_ENTRY: &str = concat!(
    r#"{"user_id": "member", "team_id": "created", "budget_id": null, "spend": 0.0,"#,
    r#" "total_spend": 0.0, "litellm_budget_table": null}"#,
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

#[rstest]
#[case::as_created("created", CREATED_ENTRY)]
#[case::every_column_set("full", FULL_ENTRY)]
#[tokio::test]
async fn a_team_membership_entry_python_wrote_is_read_typed_and_written_back_in_the_same_bytes(
    #[values(Topology::Standalone, Topology::Cluster)] topology: Topology,
    #[case] team_id: &str,
    #[case] entry: &str,
) {
    let deployment = RedisDeployment::start(topology).await.unwrap();
    let raw = connect(&deployment);
    let cache = AuthCache::new(Some(remote(connect(&deployment))), Settings::default());
    let read = AuthObject::TeamMembership {
        team_id: team_id.to_owned(),
        user_id: "member".to_owned(),
    };
    let rewritten = AuthObject::TeamMembership {
        team_id: "rewritten".to_owned(),
        user_id: "member".to_owned(),
    };
    raw.send(commands::set(&read.key(), entry, TTL))
        .await
        .unwrap();

    let held = cache.get(None, &read).await.unwrap().unwrap();
    let membership = CachedTeamMembership::from_value(&held).unwrap();
    cache
        .set(None, &rewritten, membership.clone().into_value(), Some(TTL))
        .await
        .unwrap();

    assert_eq!(
        (
            membership.team_id.as_str(),
            membership.unread,
            raw.send(commands::get::<String>(&rewritten.key()))
                .await
                .unwrap()
                .as_deref(),
        ),
        (team_id, Vec::new(), Some(entry))
    );
}
