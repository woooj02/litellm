//! A team membership as the database holds it, in the shape the auth cache holds it.

use std::sync::Arc;

use litellm_redis_store::auth_cache::{CachedTeamMembership, ExcludedIfNone};
use litellm_repositories::{Side, TeamMembership, TeamMembershipRepository};

use crate::{Error, budget::cached_budget_full};

/// The team membership as the database holds it, in the shape the auth cache holds it: the row
/// with its budget.
pub struct TeamMembershipDatabase {
    memberships: Arc<TeamMembershipRepository>,
}

impl TeamMembershipDatabase {
    pub fn new(memberships: Arc<TeamMembershipRepository>) -> Self {
        Self { memberships }
    }

    /// `user_id`'s membership of `team_id`, or `None` when the user is not a member.
    pub async fn team_membership(
        &self,
        user_id: &str,
        team_id: &str,
        side: Side,
    ) -> Result<Option<CachedTeamMembership>, Error> {
        let Some(found) = self
            .memberships
            .find_with_budget(user_id, team_id, side)
            .await?
        else {
            return Ok(None);
        };
        cached_team_membership(found).map(Some)
    }
}

/// What `LiteLLM_TeamMembership.model_dump(mode="json")` holds for `membership`, or the column
/// the model would refuse the row for.
fn cached_team_membership(membership: TeamMembership) -> Result<CachedTeamMembership, Error> {
    let TeamMembership {
        user_id,
        team_id,
        budget_id,
        spend,
        rollover_credit,
        total_spend,
        litellm_budget_table,
    } = membership;
    Ok(CachedTeamMembership {
        user_id,
        team_id,
        budget_id,
        spend: Some(spend),
        total_spend: Some(total_spend),
        rollover_credit: ExcludedIfNone(rollover_credit),
        litellm_budget_table: litellm_budget_table
            .map(|budget| cached_budget_full(budget.0))
            .transpose()?,
        unread: Vec::new(),
    })
}
