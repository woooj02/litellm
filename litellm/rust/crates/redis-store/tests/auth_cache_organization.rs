//! The auth cache's typed organization entry on real Redis. The entry literals are the ones
//! `tests/unit/redis/stores/test_auth_cache_organization.py` checks against Python's own model and
//! organization readers, and `auth-store/tests/organization.rs` finds after a database read. One
//! record is the value under every organization key, so each entry is round-tripped under the keys
//! Python writes it to. Change all three together.

mod support;

use std::time::Duration;

use litellm_redis_client::commands;
use litellm_redis_store::{
    FromValue, IntoValue,
    auth_cache::{AuthCache, AuthObject, CachedOrganization, Settings},
};
use litellm_redis_testing::{RedisDeployment, Topology};
use rstest::rstest;
use support::{connect, remote};

const TTL: Duration = Duration::from_secs(60);

const CREATED_ENTRY: &str = concat!(
    r#"{"organization_id": "created", "organization_alias": "Created", "budget_id": "budget","#,
    r#" "spend": 0.0, "metadata": {}, "models": [], "model_spend": {}, "created_by": "creator","#,
    r#" "updated_by": "creator", "users": null, "litellm_budget_table": null,"#,
    r#" "object_permission": null, "object_permission_id": null}"#,
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
const FULL_WITHOUT_BUDGET_ENTRY: &str = concat!(
    r#"{"organization_id": "full", "organization_alias": "Full", "budget_id": "budget","#,
    r#" "spend": 1.5, "metadata": {"guardrails": ["pii"]}, "models": ["claude", "all-proxy-models"],"#,
    r#" "model_spend": {"claude": 1.5}, "created_by": "creator", "updated_by": "updater","#,
    r#" "users": null, "litellm_budget_table": null, "object_permission": null,"#,
    r#" "object_permission_id": "permission"}"#,
);

#[rstest]
#[case::as_created(AuthObject::Org("created".to_owned()), CREATED_ENTRY)]
#[case::with_its_budget(AuthObject::OrgWithBudget("full".to_owned()), FULL_ENTRY)]
#[case::last_known_with_its_budget(AuthObject::OrgWithBudgetLastKnown("full".to_owned()), FULL_ENTRY)]
#[case::without_its_budget(AuthObject::Org("full".to_owned()), FULL_WITHOUT_BUDGET_ENTRY)]
#[case::by_alias(AuthObject::OrgAlias("Full".to_owned()), FULL_WITHOUT_BUDGET_ENTRY)]
#[tokio::test]
async fn an_organization_entry_python_wrote_is_read_typed_and_written_back_in_the_same_bytes(
    #[values(Topology::Standalone, Topology::Cluster)] topology: Topology,
    #[case] read: AuthObject,
    #[case] entry: &str,
) {
    let deployment = RedisDeployment::start(topology).await.unwrap();
    let raw = connect(&deployment);
    let cache = AuthCache::new(Some(remote(connect(&deployment))), Settings::default());
    let rewritten = AuthObject::Org("rewritten".to_owned());
    raw.send(commands::set(&read.key(), entry, TTL))
        .await
        .unwrap();

    let held = cache.get(None, &read).await.unwrap().unwrap();
    let organization = CachedOrganization::from_value(&held).unwrap();
    cache
        .set(
            None,
            &rewritten,
            organization.clone().into_value(),
            Some(TTL),
        )
        .await
        .unwrap();

    assert_eq!(
        (
            organization.unread,
            raw.send(commands::get::<String>(&rewritten.key()))
                .await
                .unwrap()
                .as_deref(),
        ),
        (Vec::new(), Some(entry))
    );
}
