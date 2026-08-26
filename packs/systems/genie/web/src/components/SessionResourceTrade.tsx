import React, { useState } from 'react';
import {
  cardClass,
  cardTitleClass,
  fieldClass,
  hintClass,
  primaryButtonClass,
  sectionHeadingClass,
  smallButtonClass,
  smallPrimaryButtonClass,
} from './styles';

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
  /** Spec 019: the counterpart declines a still-pending proposal. */
  onDeclineProposal?: (proposalId: string) => void | Promise<void>;
}

export const SessionResourceTrade: React.FC<SessionResourceTradeProps> = ({
  myHoldings,
  resourceTypes,
  partyMembers,
  incomingProposals,
  onProposeTrade,
  onAcceptProposal,
  onDeclineProposal,
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
    <div className={cardClass} data-testid="session-resource-trade">
      <h2 className={cardTitleClass}>Your Session Resources</h2>
      <ul className="mt-3 flex flex-wrap gap-2">
        {resourceTypes.map((rt) => (
          <li
            key={rt.key}
            className="flex min-w-20 flex-col items-center gap-0.5 rounded-lg border border-border bg-muted/40 px-3 py-2"
          >
            <span className={hintClass}>{rt.label}</span>
            <span className="text-xl leading-none font-semibold">{holdingFor(rt.key)}</span>
          </li>
        ))}
      </ul>

      {incomingProposals.length > 0 && (
        <div className="mt-5 border-t border-border pt-4">
          <h3 className={sectionHeadingClass}>Incoming Trade Proposals</h3>
          <ul className="mt-3 flex flex-col gap-2">
            {incomingProposals.map((p) => (
              <li
                key={p.id}
                className="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-border bg-muted/40 p-3 text-sm"
              >
                <span>
                  <strong className="font-medium">{p.fromActorLabel}</strong> offers {p.fromQuantity}{' '}
                  {p.fromResourceType} for your {p.toQuantity} {p.toResourceType}
                </span>
                <div className="flex gap-2">
                  <button
                    type="button"
                    className={smallPrimaryButtonClass}
                    onClick={() => onAcceptProposal?.(p.id)}
                    disabled={!onAcceptProposal}
                  >
                    Accept
                  </button>
                  <button
                    type="button"
                    className={smallButtonClass}
                    onClick={() => onDeclineProposal?.(p.id)}
                    disabled={!onDeclineProposal}
                  >
                    Decline
                  </button>
                </div>
              </li>
            ))}
          </ul>
        </div>
      )}

      {partyMembers.length > 0 && resourceTypes.length > 0 && (
        <form onSubmit={handlePropose} className="mt-5 flex flex-col gap-3 border-t border-border pt-4">
          <h3 className={sectionHeadingClass}>Propose a Trade</h3>
          <div className="flex flex-wrap items-center gap-2 text-sm">
            <span className="text-muted-foreground">Give</span>
            <input
              type="number"
              min={1}
              aria-label="Quantity to give"
              className={`w-16 ${fieldClass}`}
              value={fromQuantity}
              onChange={(e) => setFromQuantity(Number(e.target.value))}
            />
            <select
              aria-label="Resource to give"
              className={fieldClass}
              value={fromResourceType}
              onChange={(e) => setFromResourceType(e.target.value)}
            >
              {resourceTypes.map((rt) => (
                <option key={rt.key} value={rt.key}>
                  {rt.label}
                </option>
              ))}
            </select>
            <span className="text-muted-foreground">to</span>
            <select
              aria-label="Trade partner"
              className={fieldClass}
              value={toActorId}
              onChange={(e) => setToActorId(e.target.value)}
            >
              {partyMembers.map((m) => (
                <option key={m.actorId} value={m.actorId}>
                  {m.label}
                </option>
              ))}
            </select>
            <span className="text-muted-foreground">for</span>
            <input
              type="number"
              min={1}
              aria-label="Quantity to receive"
              className={`w-16 ${fieldClass}`}
              value={toQuantity}
              onChange={(e) => setToQuantity(Number(e.target.value))}
            />
            <select
              aria-label="Resource to receive"
              className={fieldClass}
              value={toResourceType}
              onChange={(e) => setToResourceType(e.target.value)}
            >
              {resourceTypes.map((rt) => (
                <option key={rt.key} value={rt.key}>
                  {rt.label}
                </option>
              ))}
            </select>
          </div>
          <button type="submit" className={`self-start ${primaryButtonClass}`} disabled={!canPropose}>
            Propose Trade
          </button>
        </form>
      )}
    </div>
  );
};

export default SessionResourceTrade;
