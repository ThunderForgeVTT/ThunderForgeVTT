import React, { useState } from 'react';

/**
 * Spec 018 (Genie) User Story 7 — Session Resources (FR-017/FR-018): the
 * current player's holdings, a propose-trade form, and accept/reject UI
 * for incoming proposals. Props-driven like `SessionWishPool.tsx`/
 * `SessionClocks.tsx`: the host page owns the
 * `genieResourceHoldings(sessionId, actorId)` query and the
 * `proposeResourceTrade`/`acceptResourceTrade` mutations
 * (`contracts/genie-session-loop.md`) — this component only renders
 * state and calls back on user action.
 *
 * Unlike the Wish Pool/Clocks (GM-only), trading is a peer-to-peer
 * negotiation (research.md R8's two-party-consent pattern) — every
 * control here is available to any player, not just the GM.
 */

export interface GenieResourceHoldingData {
  actorId: string;
  resourceType: string;
  quantity: number;
}

export interface GeniePartyMemberOption {
  actorId: string;
  label: string;
}

export interface GenieIncomingTradeProposal {
  id: string;
  fromActorId: string;
  fromActorLabel: string;
  fromResourceType: string;
  fromQuantity: number;
  toResourceType: string;
  toQuantity: number;
}

export interface SessionResourceTradeProps {
  /** The current player's own actor id. */
  myActorId: string;
  /** The current player's own holdings only (data-model.md: one row per (session, actor, resourceType)). */
  myHoldings: GenieResourceHoldingData[];
  /** The manifest's declared Session Resource types (`system.json`'s `sessionResources`), e.g. insight/favor/essence. */
  resourceTypes: { key: string; label: string }[];
  /** Other party members this player can propose a trade to. */
  partyMembers: GeniePartyMemberOption[];
  /** Proposals naming this player's actor as the counterpart, still pending (research.md R8: only the counterpart may accept). */
  incomingProposals: GenieIncomingTradeProposal[];
  onProposeTrade?: (input: {
    toActorId: string;
    fromResourceType: string;
    fromQuantity: number;
    toResourceType: string;
    toQuantity: number;
  }) => void | Promise<void>;
  onAcceptProposal?: (proposalId: string) => void | Promise<void>;
}

export const SessionResourceTrade: React.FC<SessionResourceTradeProps> = ({
  myHoldings,
  resourceTypes,
  partyMembers,
  incomingProposals,
  onProposeTrade,
  onAcceptProposal,
}) => {
  const [toActorId, setToActorId] = useState(partyMembers[0]?.actorId ?? '');
  const [fromResourceType, setFromResourceType] = useState(resourceTypes[0]?.key ?? '');
  const [fromQuantity, setFromQuantity] = useState(1);
  const [toResourceType, setToResourceType] = useState(resourceTypes[0]?.key ?? '');
  const [toQuantity, setToQuantity] = useState(1);

  const holdingFor = (resourceType: string) =>
    myHoldings.find((h) => h.resourceType === resourceType)?.quantity ?? 0;

  const canPropose =
    !!onProposeTrade && !!toActorId && !!fromResourceType && !!toResourceType && fromQuantity > 0 && toQuantity > 0;

  const handlePropose = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!canPropose) return;
    await onProposeTrade?.({ toActorId, fromResourceType, fromQuantity, toResourceType, toQuantity });
  };

  return (
    <div className="p-4 border rounded-lg bg-white shadow-sm" data-testid="session-resource-trade">
      <h2 className="text-lg font-bold mb-2">Your Session Resources</h2>
      <ul className="flex gap-4 mb-4">
        {resourceTypes.map((rt) => (
          <li key={rt.key} className="flex flex-col items-center border rounded px-3 py-2">
            <span className="text-xs text-gray-600">{rt.label}</span>
            <span className="text-xl font-bold">{holdingFor(rt.key)}</span>
          </li>
        ))}
      </ul>

      {incomingProposals.length > 0 && (
        <div className="mb-4">
          <h3 className="font-semibold text-sm mb-1">Incoming Trade Proposals</h3>
          <ul className="flex flex-col gap-2">
            {incomingProposals.map((p) => (
              <li key={p.id} className="flex items-center justify-between border rounded p-2 text-sm">
                <span>
                  <strong>{p.fromActorLabel}</strong> offers {p.fromQuantity} {p.fromResourceType} for your{' '}
                  {p.toQuantity} {p.toResourceType}
                </span>
                <button
                  type="button"
                  className="px-2 py-1 bg-green-600 text-white rounded text-xs font-semibold"
                  onClick={() => onAcceptProposal?.(p.id)}
                  disabled={!onAcceptProposal}
                >
                  Accept
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}

      {partyMembers.length > 0 && resourceTypes.length > 0 && (
        <form onSubmit={handlePropose} className="flex flex-col gap-2">
          <h3 className="font-semibold text-sm">Propose a Trade</h3>
          <div className="flex items-center gap-2 flex-wrap text-sm">
            <span>Give</span>
            <input
              type="number"
              min={1}
              className="border rounded p-1 w-16"
              value={fromQuantity}
              onChange={(e) => setFromQuantity(Number(e.target.value))}
            />
            <select className="border rounded p-1" value={fromResourceType} onChange={(e) => setFromResourceType(e.target.value)}>
              {resourceTypes.map((rt) => (
                <option key={rt.key} value={rt.key}>
                  {rt.label}
                </option>
              ))}
            </select>
            <span>to</span>
            <select className="border rounded p-1" value={toActorId} onChange={(e) => setToActorId(e.target.value)}>
              {partyMembers.map((m) => (
                <option key={m.actorId} value={m.actorId}>
                  {m.label}
                </option>
              ))}
            </select>
            <span>for</span>
            <input
              type="number"
              min={1}
              className="border rounded p-1 w-16"
              value={toQuantity}
              onChange={(e) => setToQuantity(Number(e.target.value))}
            />
            <select className="border rounded p-1" value={toResourceType} onChange={(e) => setToResourceType(e.target.value)}>
              {resourceTypes.map((rt) => (
                <option key={rt.key} value={rt.key}>
                  {rt.label}
                </option>
              ))}
            </select>
          </div>
          <button
            type="submit"
            className="self-start px-4 py-1.5 bg-indigo-600 text-white rounded font-semibold disabled:opacity-50"
            disabled={!canPropose}
          >
            Propose Trade
          </button>
        </form>
      )}
    </div>
  );
};

export default SessionResourceTrade;
