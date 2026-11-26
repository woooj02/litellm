use litellm_python_compat::Value;

use super::{
    budget::CachedBudgetFull,
    record::{ExcludedIfNone, Field, fields, record, required},
};
use crate::{DecodeError, FromValue, IntoValue};

record! {
    /// `LiteLLM_TeamMembership.model_dump(mode="json")`, the value under
    /// [`AuthObject::TeamMembership`](crate::auth_cache::AuthObject::TeamMembership), which the
    /// key's team member budget check writes. `get_team_membership` caches the same dump under
    /// [`AuthObject::TeamMembershipReservation`](crate::auth_cache::AuthObject::TeamMembershipReservation),
    /// or the string `__no_team_membership__` when the user is not a member. The auth prefetch
    /// writes both keys from the key owners row, whose budget times have no `Z` and whose NULL
    /// `allowed_models` is `[]`, so its bytes differ from these for the same row.
    pub struct CachedTeamMembership {
        user_id: String = required!("user_id"),
        team_id: String = required!("team_id"),
        budget_id: Option<String>,
        spend: Option<f64> = Some(0.0),
        total_spend: Option<f64> = Some(0.0),
        rollover_credit: ExcludedIfNone<f64>,
        litellm_budget_table: Option<CachedBudgetFull>,
    }
}
