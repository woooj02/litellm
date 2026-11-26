use jiff::Timestamp;
use litellm_python_compat::Value;

use super::{
    bag::Bag,
    budget::CachedBudget,
    budget_limit::CachedBudgetLimit,
    object_permission::CachedObjectPermission,
    record::{ExcludedIfNone, Field, fields, record, required},
};
use crate::{DecodeError, FromValue, IntoValue};

record! {
    /// One of a user's organization memberships, as `LiteLLM_OrganizationMembershipTable` dumps it
    /// nested in the user. The user's read nests neither the membership's budget nor its user, so
    /// `litellm_budget_table`, `user` and `user_email` are `None` in what it caches.
    pub struct CachedOrganizationMembership {
        user_id: String = required!("user_id"),
        organization_id: String = required!("organization_id"),
        user_role: Option<String>,
        spend: f64,
        budget_id: Option<String>,
        created_at: Timestamp = required!("created_at"),
        updated_at: Timestamp = required!("updated_at"),
        #[redact(with = "[REDACTED]")]
        user: Option<Value>,
        litellm_budget_table: Option<CachedBudget>,
        user_email: Option<String>,
    }
}

record! {
    /// `LiteLLM_UserTable.model_dump(mode="json")`, the value under
    /// [`AuthObject::User`](crate::auth_cache::AuthObject::User), the bare user id.
    /// `get_user_object` caches it for the row it read, the row it created under `user_id_upsert`,
    /// and that row with its email backfilled, and the Python auth host recaches it after a role or
    /// team change. Each is the user repository's row with its organization memberships ordered by
    /// organization id and nothing else nested, so `object_permission` is `None`. The password is
    /// never dumped. Nothing is cached for an id no row has.
    ///
    /// The auth prefetch writes the key from the key owners row instead, whose times have no `Z`
    /// and whose memberships come in no set order, so its bytes differ from this shape for the
    /// same row.
    pub struct CachedUser {
        user_id: String = required!("user_id"),
        user_alias: Option<String>,
        team_id: Option<String>,
        sso_user_id: Option<String>,
        organization_id: Option<String>,
        object_permission_id: Option<String>,
        password_reset_required: Option<bool>,
        last_breach_check_at: Option<Timestamp>,
        teams: Vec<String>,
        user_role: Option<String>,
        max_budget: Option<f64>,
        rollover_max_budget: ExcludedIfNone<f64>,
        rollover_credit: ExcludedIfNone<f64>,
        budget_limits: Option<Vec<CachedBudgetLimit>>,
        spend: f64,
        user_email: Option<String>,
        models: Vec<String>,
        metadata: Option<Bag>,
        max_parallel_requests: Option<i64>,
        tpm_limit: Option<i64>,
        rpm_limit: Option<i64>,
        budget_duration: Option<String>,
        budget_reset_at: Option<Timestamp>,
        allowed_cache_controls: Vec<String>,
        policies: Vec<String>,
        model_spend: Option<Bag> = Some(Bag::default()),
        model_max_budget: Option<Bag> = Some(Bag::default()),
        created_at: Option<Timestamp>,
        updated_at: Option<Timestamp>,
        organization_memberships: Option<Vec<CachedOrganizationMembership>>,
        object_permission: Option<CachedObjectPermission>,
    }
}
