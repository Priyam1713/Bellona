"""Bellona Python SDK — typed mirror of the forge primitives and the
Praetorian Gate wire protocol. Zero dependencies.

Agent = Model + Harness.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Literal, Optional
from urllib.request import Request, urlopen
import json

EffectKind = Literal[
    "file_read",
    "file_write",
    "shell_exec",
    "browser_navigate",
    "browser_act",
    "mcp_call",
    "memory_write",
    "component_publish",
]


def is_read(effect: str) -> bool:
    """Anything not positively classified as a read is treated as a write."""
    return effect == "file_read"


@dataclass(frozen=True)
class ActionRequest:
    agent_id: str
    tool_name: str
    effect: EffectKind
    target_uri: str = ""
    params: Any = None
    intent: str = ""
    session_id: Optional[str] = None
    id: Optional[str] = None


@dataclass(frozen=True)
class IdentityRecord:
    agent_pub: str
    owner_pub: str
    agent_sig: str
    owner_sig: str


@dataclass(frozen=True)
class LedgerRecord:
    seq: int
    ts_ms: int
    kind: str
    payload: Any
    prev_hash: str
    hash: str


@dataclass
class CustosClient:
    """Thin client over the Praetorian Gate HTTP surface. Adds no trust,
    only types — resolve/policy/audit happen server-side before execution."""

    base_url: str
    _timeout: float = field(default=30.0, repr=False)

    def submit(self, req: ActionRequest) -> dict[str, Any]:
        return self._json(
            "POST", "/v1/gate/submit",
            {k: v for k, v in req.__dict__.items() if v is not None},
        )

    def approve(self, ticket_id: str, approver: str) -> dict[str, Any]:
        return self._json("POST", "/v1/gate/approve",
                          {"ticket_id": ticket_id, "approver": approver})

    def reject(self, ticket_id: str, approver: str, reason: str) -> None:
        self._json("POST", "/v1/gate/reject",
                   {"ticket_id": ticket_id, "approver": approver, "reason": reason})

    def veto(self, reason: str) -> None:
        """The Tribunician Veto — freezes every layer."""
        self._json("POST", "/v1/veto", {"reason": reason})

    def ledger(self) -> list[LedgerRecord]:
        raw = self._json("GET", "/v1/ledger")
        return [LedgerRecord(**r) for r in raw]

    def _json(self, method: str, path: str, body: Any | None = None) -> Any:
        data = json.dumps(body).encode() if body is not None else None
        req = Request(
            f"{self.base_url.rstrip('/')}{path}",
            data=data,
            method=method,
            headers={"content-type": "application/json"},
        )
        with urlopen(req, timeout=self._timeout) as resp:
            return json.loads(resp.read().decode())


__all__ = [
    "ActionRequest", "IdentityRecord", "LedgerRecord", "CustosClient",
    "EffectKind", "is_read",
]
__version__ = "0.1.0"
