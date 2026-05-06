/**
 * inviteReplicationHandler.ts
 * Phase 4.10.C.1: Replication handler for invite and member events
 *
 * Connects GraphQL subscription (worldEventCreated) to RxDB collections.
 * 
 * Flow:
 *  1. GraphQL subscription emits worldEventCreated event
 *  2. Handler receives event with event_code (2, 3, or 4)
 *  3. Queries backend for fresh data (worldInvites or worldMembers)
 *  4. Bulk upserts to RxDB collections
 *  5. RxDB observables emit → React hooks re-render
 */

import { ApolloClient, gql } from '@apollo/client';
import type { WorldDatabase } from '@/engine/world/sync/database';
import type { WorldInviteDoc } from '@/db/collections/worldInvitesCollection';
import type { WorldMemberDoc } from '@/db/collections/worldMembersCollection';

// Event codes from backend (src/server/src/graphql/mutations_invites.rs)
const EVENT_CODE_INVITE_CREATED = 2;
const EVENT_CODE_MEMBER_JOINED = 3;
const EVENT_CODE_MEMBER_ROLE_CHANGED = 4;

/**
 * GraphQL queries to fetch fresh data after events
 */
const QUERY_WORLD_INVITES = gql`
  query worldInvites($world_id: ID!) {
    worldInvites(world_id: $world_id) {
      id
      world_id
      invite_code
      max_uses
      used_count
      expires_at
      created_by
      created_at
      updated_at
      status
    }
  }
`;

const QUERY_WORLD_MEMBERS = gql`
  query worldMembers($world_id: ID!) {
    worldMembers(world_id: $world_id) {
      id
      world_id
      user_id
      role
      joined_at
      created_at
      updated_at
    }
  }
`;

/**
 * Setup replication handler for invites and members.
 *
 * This function subscribes to the worldEventCreated subscription and syncs
 * changes to the RxDB collections.
 *
 * @param db - RxDB World Database instance
 * @param graphqlClient - Apollo GraphQL client
 * @param worldId - World ID to listen for events
 * @returns Cleanup function to unsubscribe
 */
export async function setupInviteReplication(
  db: WorldDatabase,
  graphqlClient: ApolloClient<any>,
  worldId: string
): Promise<() => void> {
  let lastEventId = 0;
  const abortController = new AbortController();

  // Start listening to world events
  const subscription = graphqlClient
    .subscribe<any>({
      query: gql`
        subscription worldEventCreated($world_id: ID!, $last_event_id: Int) {
          worldEventsCreated(world_id: $world_id, lastEventId: $last_event_id) {
            id
            world_id
            event_code
            created_at
            created_by
          }
        }
      `,
      variables: {
        world_id: worldId,
        last_event_id: lastEventId,
      },
    })
    .subscribe({
      next: async (result) => {
        if (abortController.signal.aborted) return;

        try {
          const event = result.data?.worldEventsCreated;
          if (!event) return;

          lastEventId = event.id;

          // Handle different event types
          switch (event.event_code) {
            case EVENT_CODE_INVITE_CREATED:
              // Fetch fresh invites and sync
              await syncWorldInvites(graphqlClient, db, worldId);
              break;

            case EVENT_CODE_MEMBER_JOINED:
            case EVENT_CODE_MEMBER_ROLE_CHANGED:
              // Fetch fresh members and sync
              await syncWorldMembers(graphqlClient, db, worldId);
              break;
          }
        } catch (error) {
          console.error('Error processing world event:', error);
        }
      },
      error: (error) => {
        console.error('Subscription error:', error);
      },
      complete: () => {
        console.log('Invite replication subscription completed');
      },
    });

  // Return cleanup function
  return () => {
    abortController.abort();
    subscription.unsubscribe();
  };
}

/**
 * Sync world invites from server to RxDB.
 *
 * Queries the backend for all invites in the world and bulk upserts them.
 */
async function syncWorldInvites(
  graphqlClient: ApolloClient<any>,
  db: WorldDatabase,
  worldId: string
): Promise<void> {
  try {
    const result = await graphqlClient.query<any>({
      query: QUERY_WORLD_INVITES,
      variables: { world_id: worldId },
      fetchPolicy: 'network-only', // Always fetch fresh from server
    });

    const invites: WorldInviteDoc[] = result.data?.worldInvites || [];

    if (invites.length > 0) {
      // Bulk upsert to RxDB
      await db.collections.world_invites.bulkUpsert(invites);
      console.log(`[Invites] Synced ${invites.length} invite(s)`);
    }
  } catch (error) {
    console.error('Failed to sync invites:', error);
  }
}

/**
 * Sync world members from server to RxDB.
 *
 * Queries the backend for all members in the world and bulk upserts them.
 */
async function syncWorldMembers(
  graphqlClient: ApolloClient<any>,
  db: WorldDatabase,
  worldId: string
): Promise<void> {
  try {
    const result = await graphqlClient.query<any>({
      query: QUERY_WORLD_MEMBERS,
      variables: { world_id: worldId },
      fetchPolicy: 'network-only', // Always fetch fresh from server
    });

    const members: WorldMemberDoc[] = result.data?.worldMembers || [];

    if (members.length > 0) {
      // Bulk upsert to RxDB
      await db.collections.world_members.bulkUpsert(members);
      console.log(`[Members] Synced ${members.length} member(s)`);
    }
  } catch (error) {
    console.error('Failed to sync members:', error);
  }
}

/**
 * Initial catch-up: Fetch all data when subscription is established.
 *
 * Call this after setupInviteReplication() to populate RxDB with initial data.
 */
export async function fetchInitialData(
  graphqlClient: ApolloClient<any>,
  db: WorldDatabase,
  worldId: string
): Promise<void> {
  try {
    console.log('[Sync] Fetching initial data for world:', worldId);

    // Fetch and sync all invites
    await syncWorldInvites(graphqlClient, db, worldId);

    // Fetch and sync all members
    await syncWorldMembers(graphqlClient, db, worldId);

    console.log('[Sync] Initial data loaded');
  } catch (error) {
    console.error('Failed to fetch initial data:', error);
  }
}
