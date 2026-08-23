/**
 * Bellona TypeScript SDK — the typed mirror of the forge primitives and
 * the Praetorian Gate wire protocol. Zero dependencies.
 *
 * Agent = Model + Harness. The harness is the product.
 */

// ---------- identifiers ----------

export type AgentId = string & { readonly __brand: "AgentId" };
export type SessionId = string & { readonly __brand: "SessionId" };
export type ActionId = string & { readonly __brand: "ActionId" };

// ---------- effects & decisions ----------

export type EffectKind =
  | "file_read"
  | "file_write"
  | "shell_exec"
  | "browser_navigate"
  | "browser_act"
  | "mcp_call"
  | "memory_write"
  | "component_publish"
  | { custom: string };

/** Anything not positively classified as a read is treated as a write. */
export const isRead = (k: EffectKind): boolean => k === "file_read";

export interface ActionRequest {
  readonly id: ActionId;
  readonly agent_id: AgentId;
  readonly session_id?: SessionId;
  readonly tool_name: string;
  readonly effect: EffectKind;
  readonly target_uri: string;
  readonly params: unknown;
  readonly intent: string;
}

export type Decision =
  | { decision: "allow"; rule_id: string }
  | { decision: "deny"; rule_id: string; reason: string }
  | { decision: "require_approval"; rule_id: string };

export type Outcome =
  | { outcome: "completed"; result: unknown }
  | { outcome: "failed"; error: string };

export type GateOutcome =
  | { gate: "executed"; action_id: string; outcome: Outcome }
  | { gate: "denied"; rule_id: string; reason: string }
  | { gate: "pending_approval"; ticket_id: string };

// ---------- ledger ----------

export interface LedgerRecord {
  seq: number;
  ts_ms: number;
  kind: string;
  payload: unknown;
  prev_hash: string;
  hash: string;
}

// ---------- identity (Law V) ----------

export interface IdentityRecord {
  agent_pub: string;
  owner_pub: string;
  agent_sig: string;
  owner_sig: string;
}

// ---------- AG-UI surface events ----------

export type AgUiEvent =
  | { type: "run_started"; run_id: string }
  | { type: "text_message_content"; delta: string }
  | { type: "tool_call_started"; name: string }
  | { type: "tool_call_ended"; name: string; ok: boolean }
  | { type: "state_snapshot"; state: unknown }
  | { type: "run_finished"; run_id: string; ok: boolean }
  | { type: "error"; message: string };

// ---------- the client ----------

export interface CustosClientOptions {
  /** Base URL of a Bellona server (Hono API on :3001 by default). */
  baseUrl: string;
  fetchImpl?: typeof fetch;
}

/**
 * Thin client over the Praetorian Gate HTTP surface. Every submit is
 * resolved → policy-checked → audited server-side before execution;
 * this SDK adds no trust, only types.
 */
export class CustosClient {
  private readonly baseUrl: string;
  private readonly f: typeof fetch;

  constructor(opts: CustosClientOptions) {
    this.baseUrl = opts.baseUrl.replace(/\/$/, "");
    this.f = opts.fetchImpl ?? globalThis.fetch.bind(globalThis);
  }

  async submit(req: ActionRequest): Promise<GateOutcome> {
    return this.json("POST", "/v1/gate/submit", req);
  }

  async approve(ticketId: string, approver: string): Promise<GateOutcome> {
    return this.json("POST", "/v1/gate/approve", { ticket_id: ticketId, approver });
  }

  async reject(ticketId: string, approver: string, reason: string): Promise<void> {
    await this.json("POST", "/v1/gate/reject", {
      ticket_id: ticketId,
      approver,
      reason,
    });
  }

  async veto(reason: string): Promise<void> {
    await this.json("POST", "/v1/veto", { reason });
  }

  async ledger(): Promise<LedgerRecord[]> {
    return this.json("GET", "/v1/ledger");
  }

  private async json<T>(method: string, path: string, body?: unknown): Promise<T> {
    const init: RequestInit = {
      method,
      headers: { "content-type": "application/json" },
      ...(body === undefined ? {} : { body: JSON.stringify(body) }),
    };
    const res = await this.f(`${this.baseUrl}${path}`, init);
    if (!res.ok) {
      throw new Error(`bellona: ${method} ${path} -> ${res.status}`);
    }
    return (await res.json()) as T;
  }
}
