//! An organization as the database holds it, in the shape the auth cache holds it.

use std::sync::Arc;

use litellm_redis_store::auth_cache::CachedOrganization;
use litellm_repositories::{Organization, OrganizationRepository};

use crate::{Error, budget::cached_budget, column::nullable};

/// The organization as the database holds it, in the shape the auth cache holds it: the row with
/// its budget when asked for, and nothing else nested.
pub struct OrganizationDatabase {
    organizations: Arc<OrganizationRepository>,
}

impl OrganizationDatabase {
    pub fn new(organizations: Arc<OrganizationRepository>) -> Self {
        Self { organizations }
    }

    /// The organization `organization_id`, or `None` when no row has that id. With
    /// `include_budget_table` it is the shape `get_org_object` caches under `org_id:{id}:with_budget`,
    /// and without it the shape under `org_id:{id}`, where `litellm_budget_table` is `None`. The
    /// object permission the organization points at is not read.
    pub async fn organization(
        &self,
        organization_id: &str,
        include_budget_table: bool,
    ) -> Result<Option<CachedOrganization>, Error> {
        let Some(found) = self
            .organizations
            .find_with_optional_budget(organization_id, include_budget_table)
            .await?
        else {
            return Ok(None);
        };
        cached_organization(found).map(Some)
    }
}

/// What `LiteLLM_OrganizationTable.model_dump(mode="json")` holds for `organization`, or the
/// column the model would refuse the row for. The JSON columns are never SQL NULL, but each may
/// hold a JSON `null`, which the model reads as unset.
fn cached_organization(organization: Organization) -> Result<CachedOrganization, Error> {
    let Organization {
        organization_id,
        organization_alias,
        budget_id,
        metadata,
        models,
        spend,
        model_spend,
        object_permission_id,
        created_by,
        updated_by,
        litellm_budget_table,
    } = organization;
    Ok(CachedOrganization {
        organization_id: Some(organization_id),
        organization_alias: Some(organization_alias),
        budget_id,
        spend,
        metadata: nullable("metadata", Some(&metadata))?,
        models,
        model_spend: nullable("model_spend", Some(&model_spend))?,
        created_by,
        updated_by,
        users: None,
        litellm_budget_table: litellm_budget_table
            .map(|budget| cached_budget(budget.0))
            .transpose()?,
        object_permission: None,
        object_permission_id,
        unread: Vec::new(),
    })
}
