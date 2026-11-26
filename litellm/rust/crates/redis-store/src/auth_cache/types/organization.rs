use litellm_python_compat::Value;

use super::{
    bag::Bag,
    budget::CachedBudget,
    object_permission::CachedObjectPermission,
    record::{Field, fields, record, required},
};
use crate::{DecodeError, FromValue, IntoValue};

record! {
    /// `LiteLLM_OrganizationTable.model_dump(mode="json")`, the value under every organization
    /// key. `get_org_object` caches it under
    /// [`AuthObject::OrgWithBudget`](crate::auth_cache::AuthObject::OrgWithBudget) with the budget
    /// nested, and under [`AuthObject::Org`](crate::auth_cache::AuthObject::Org) with
    /// `litellm_budget_table` `None`. `get_org_object_by_alias` caches the organization an alias
    /// names, read without its budget, under
    /// [`AuthObject::OrgAlias`](crate::auth_cache::AuthObject::OrgAlias) and `Org` alike, and
    /// `get_org_object_for_request` copies what it read under `OrgWithBudget` to
    /// [`AuthObject::OrgWithBudgetLastKnown`](crate::auth_cache::AuthObject::OrgWithBudgetLastKnown).
    /// `users` and `object_permission` are never set by those readers. Nothing is cached for an
    /// id or alias no row has.
    ///
    /// The auth prefetch writes `Org` and `OrgWithBudget` from the key owners row instead: both
    /// with the budget and the object permission nested, budget times without a `Z`, and `[]` for
    /// a NULL `allowed_models`, so its bytes differ from this shape for the same row.
    pub struct CachedOrganization {
        organization_id: Option<String>,
        organization_alias: Option<String>,
        budget_id: String = required!("budget_id"),
        spend: f64,
        metadata: Option<Bag>,
        models: Vec<String>,
        model_spend: Option<Bag>,
        created_by: String = required!("created_by"),
        updated_by: String = required!("updated_by"),
        #[redact(with = "[REDACTED]")]
        users: Option<Vec<Value>>,
        litellm_budget_table: Option<CachedBudget>,
        object_permission: Option<CachedObjectPermission>,
        object_permission_id: Option<String>,
    }
}
