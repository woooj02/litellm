//! A user as the database holds it, in the shape the auth cache holds it.

use std::sync::Arc;

use litellm_redis_store::auth_cache::{CachedOrganizationMembership, CachedUser, ExcludedIfNone};
use litellm_repositories::{OrganizationMembership, Side, SignIn, User, UserRepository};

use crate::{
    Error,
    column::{instant, nullable, nullable_list, time},
};

/// The user as the database holds it, in the shape the auth cache holds it: the row with its
/// organization memberships, and nothing else nested.
pub struct UserDatabase {
    users: Arc<UserRepository>,
}

impl UserDatabase {
    pub fn new(users: Arc<UserRepository>) -> Self {
        Self { users }
    }

    /// The user `user_id`, or `None` when no row has that id. It is the repository's resolve of a
    /// sign-in that carries only the id, which `get_user_object` makes for a plain lookup: with no
    /// SSO id or email there is nothing to fall back to or link, so it only reads. The object
    /// permission the user points at is not read.
    pub async fn user(&self, user_id: &str, side: Side) -> Result<Option<CachedUser>, Error> {
        let sign_in = SignIn {
            user_id: user_id.to_owned(),
            sso_user_id: None,
            user_email: None,
            email_verified: None,
            links_any_user: false,
        };
        let Some(found) = self.users.resolve(&sign_in, side).await? else {
            return Ok(None);
        };
        cached_user(found).map(Some)
    }
}

/// What `LiteLLM_OrganizationMembershipTable.model_dump(mode="json")` holds for `membership`. The
/// model requires `spend` and both times, which the columns let be NULL, so such a membership
/// refuses the whole user.
fn cached_membership(
    membership: OrganizationMembership,
) -> Result<CachedOrganizationMembership, Error> {
    let OrganizationMembership {
        user_id,
        organization_id,
        user_role,
        spend,
        budget_id,
        created_at,
        updated_at,
    } = membership;
    let refused = || Error::Row {
        column: "organization_memberships",
    };
    Ok(CachedOrganizationMembership {
        user_id,
        organization_id,
        user_role,
        spend: spend.ok_or_else(refused)?,
        budget_id,
        created_at: instant("organization_memberships", created_at.ok_or_else(refused)?)?,
        updated_at: instant("organization_memberships", updated_at.ok_or_else(refused)?)?,
        user: None,
        litellm_budget_table: None,
        user_email: None,
        unread: Vec::new(),
    })
}

/// What `LiteLLM_UserTable.model_dump(mode="json")` holds for `user`, or the column the model
/// would refuse the row for.
fn cached_user(user: User) -> Result<CachedUser, Error> {
    let User {
        user_id,
        user_alias,
        team_id,
        sso_user_id,
        organization_id,
        object_permission_id,
        password_reset_required,
        last_breach_check_at,
        teams,
        user_role,
        max_budget,
        rollover_max_budget,
        rollover_credit,
        budget_limits,
        spend,
        user_email,
        models,
        metadata,
        max_parallel_requests,
        tpm_limit,
        rpm_limit,
        budget_duration,
        budget_reset_at,
        allowed_cache_controls,
        policies,
        model_spend,
        model_max_budget,
        created_at,
        updated_at,
        organization_memberships,
    } = user;
    Ok(CachedUser {
        user_id,
        user_alias,
        team_id,
        sso_user_id,
        organization_id,
        object_permission_id,
        password_reset_required,
        last_breach_check_at: time("last_breach_check_at", last_breach_check_at)?,
        teams,
        user_role,
        max_budget,
        rollover_max_budget: ExcludedIfNone(rollover_max_budget),
        rollover_credit: ExcludedIfNone(rollover_credit),
        budget_limits: nullable_list("budget_limits", budget_limits.as_ref())?,
        spend,
        user_email,
        models,
        metadata: nullable("metadata", Some(&metadata))?,
        max_parallel_requests: max_parallel_requests.map(i64::from),
        tpm_limit,
        rpm_limit,
        budget_duration,
        budget_reset_at: time("budget_reset_at", budget_reset_at)?,
        allowed_cache_controls,
        policies,
        model_spend: nullable("model_spend", Some(&model_spend))?,
        model_max_budget: nullable("model_max_budget", Some(&model_max_budget))?,
        created_at: time("created_at", created_at)?,
        updated_at: time("updated_at", updated_at)?,
        organization_memberships: Some(
            organization_memberships
                .0
                .into_iter()
                .map(cached_membership)
                .collect::<Result<_, _>>()?,
        ),
        object_permission: None,
        unread: Vec::new(),
    })
}
