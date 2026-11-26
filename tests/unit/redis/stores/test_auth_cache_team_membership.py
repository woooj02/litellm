"""The auth cache entry the Rust auth store shapes for a team membership, against Python's own model and its own
membership readers and writers. The row and entry literals are the ones
``litellm/rust/crates/auth-store/tests/team_membership.rs`` reads from Postgres and
``litellm/rust/crates/redis-store/tests/auth_cache_team_membership.rs`` round-trips through Redis. Change all three
together."""

import json
from typing import Final
from unittest.mock import MagicMock

import pytest
from pydantic import TypeAdapter

from litellm.proxy._types import LiteLLM_TeamMembership
from litellm.proxy.auth.auth_checks import get_team_membership
from litellm.proxy.auth.host.steps.budgets import BudgetChecks
from litellm.proxy.common_utils.cache_pydantic_utils import CacheCodec
from litellm.proxy.common_utils.user_api_key_cache import UserApiKeyCache
from litellm.redis.stores.auth_cache import AuthObject
from litellm.repositories import rust_database
from tests.unit.proxy.rust_auth_stores import install_rust_auth_stores

CREATED_ROW: Final = (
    r'{"user_id":"member","team_id":"created","budget_id":null,"spend":0.0,"total_spend":0.0,'
    r'"litellm_budget_table":null}'
)
CREATED_ENTRY: Final = (
    r'{"user_id": "member", "team_id": "created", "budget_id": null, "spend": 0.0, "total_spend": 0.0,'
    r' "litellm_budget_table": null}'
)
FULL_ROW: Final = (
    r'{"user_id":"member","team_id":"full","budget_id":"budget","spend":1.5,"rollover_credit":2.5,'
    r'"total_spend":3.0,"litellm_budget_table":{"budget_id":"budget","name":"Member","max_budget":10.5,'
    r'"rollover_max_budget":20.0,"soft_budget":8.0,"max_parallel_requests":3,"tpm_limit":1000,"rpm_limit":10,'
    r'"tpd_limit":100000,"model_max_budget":{"claude": {"max_budget": 1}},"model_tpm_limit":{"gpt": "7",'
    r' "claude": 100.0},"model_rpm_limit":{"claude": 5},"budget_duration":"30d","budget_limits":[],'
    r'"cap_policy":"hard","budget_reset_at":"2026-12-01T00:00:00+00:00","budget_reset_count":2,'
    r'"allowed_models":["claude"],"temp_budget_increase":1.5,'
    r'"temp_budget_expiry":"2026-11-20T12:30:00.250+00:00","created_at":"2026-11-01T00:00:00+00:00",'
    r'"created_by":"test","updated_at":"2026-11-02T00:00:00+00:00","updated_by":"test"}}'
)
FULL_ENTRY: Final = (
    r'{"user_id": "member", "team_id": "full", "budget_id": "budget", "spend": 1.5, "total_spend": 3.0,'
    r' "rollover_credit": 2.5, "litellm_budget_table": {"budget_id": "budget", "soft_budget": 8.0,'
    r' "max_budget": 10.5, "rollover_max_budget": 20.0, "max_parallel_requests": 3, "tpm_limit": 1000,'
    r' "rpm_limit": 10, "tpd_limit": 100000, "model_max_budget": {"claude": {"max_budget": 1}},'
    r' "model_tpm_limit": {"gpt": 7, "claude": 100}, "model_rpm_limit": {"claude": 5},'
    r' "budget_duration": "30d", "allowed_models": ["claude"], "temp_budget_increase": 1.5,'
    r' "temp_budget_expiry": "2026-11-20T12:30:00.250000Z", "budget_reset_at": "2026-12-01T00:00:00Z",'
    r' "created_at": "2026-11-01T00:00:00Z"}}'
)

USER_ID: Final = "member"
_FIELDS: Final = TypeAdapter(dict[str, object])
_MEMBERSHIPS: Final = pytest.mark.parametrize(
    ("team_id", "row", "entry"),
    [("created", CREATED_ROW, CREATED_ENTRY), ("full", FULL_ROW, FULL_ENTRY)],
    ids=["as_created", "every_column_set"],
)


def _budget_checks(cache: UserApiKeyCache) -> BudgetChecks:
    return BudgetChecks(
        request=MagicMock(),
        request_data={},
        route="/chat/completions",
        prisma_client=MagicMock(),
        user_api_key_cache=cache,
        proxy_logging_obj=MagicMock(),
        llm_router=None,
        llm_model_list=None,
        model_max_budget_limiter=MagicMock(),
    )


@_MEMBERSHIPS
def test_the_membership_entry_is_what_python_dumps_for_the_repository_row(team_id: str, row: str, entry: str) -> None:
    """The repository's answer decoded as ``LiteLLM_TeamMembership`` and serialized the way the cache writes it.
    The budget has a ``created_at``, so pydantic reads it as ``LiteLLM_BudgetTableFull``."""
    membership: Final = rust_database.decode(LiteLLM_TeamMembership, row)
    assert membership is not None
    assert membership.team_id == team_id

    assert json.dumps(CacheCodec.serialize(membership, model_type=LiteLLM_TeamMembership)) == entry


@_MEMBERSHIPS
@pytest.mark.asyncio
async def test_both_proxy_writers_cache_a_membership_read_from_the_database_in_the_bytes_the_rust_store_caches(
    team_id: str, row: str, entry: str, monkeypatch: pytest.MonkeyPatch
) -> None:
    """The team member budget check writes ``{team_id}_{user_id}`` and ``get_team_membership`` writes
    ``team_membership:{user_id}:{team_id}``, each from its own repository read of the same row."""
    stores: Final = install_rust_auth_stores(monkeypatch)
    stores.team_membership.rows[(USER_ID, team_id)] = _FIELDS.validate_json(row)
    cache: Final = UserApiKeyCache()

    await _budget_checks(cache)._team_membership(MagicMock(), USER_ID, team_id)  # pyright: ignore[reportPrivateUsage]  # the only writer of the key outside the prefetch, reached publicly only past a spend read
    await get_team_membership(user_id=USER_ID, team_id=team_id, prisma_client=MagicMock(), user_api_key_cache=cache)

    assert (
        json.dumps(cache.get_local(AuthObject.team_membership(team_id=team_id, user_id=USER_ID))),
        json.dumps(cache.get_local(AuthObject.team_membership_reservation(user_id=USER_ID, team_id=team_id))),
    ) == (entry, entry)


@pytest.mark.parametrize(
    ("team_id", "entry"), [("created", CREATED_ENTRY), ("full", FULL_ENTRY)], ids=["as_created", "every_column_set"]
)
@pytest.mark.asyncio
async def test_python_caches_the_membership_it_read_from_the_entry_in_the_same_bytes(team_id: str, entry: str) -> None:
    """A field added to ``LiteLLM_TeamMembership`` or the budget it nests shows up here first: Python then writes a
    field the Rust record does not name, and the Rust side of this contract has to name it too."""
    cache: Final = UserApiKeyCache()
    cache.set_local(AuthObject.team_membership(team_id=team_id, user_id=USER_ID), _FIELDS.validate_json(entry), None)

    membership: Final = await _budget_checks(cache)._team_membership(MagicMock(), USER_ID, team_id)  # pyright: ignore[reportPrivateUsage]  # the key's one reader, reached publicly only past a spend read

    assert json.dumps(CacheCodec.serialize(membership, model_type=LiteLLM_TeamMembership)) == entry
