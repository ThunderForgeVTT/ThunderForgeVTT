//! Which of this pack's root fields a paused world refuses (spec 051).
//!
//! Every root field `GenieSessionQuery` and `GenieSessionMutation` merge into
//! the schema is listed here once. The app crate's `play_pause_surface_tests`
//! fails on one that is not, and calls each gated document against a paused
//! world as its Game Master and as a site admin who is a member.

use thunderforge_server::play_pause::surface::{PackSurface, SeedStep};

inventory::submit! {
    PackSurface {
        system_id: crate::SYSTEM_ID,
        // Each mutation moves one world's session state, and each records a
        // world event on success.
        gated: &[
            (
                "startGenieSession",
                r#"mutation { startGenieSession(input: { worldId: "{world}", doomClockMax: 6 }) { id } }"#,
            ),
            (
                "spendWish",
                r#"mutation { spendWish(sessionId: "{session}", narrativeEffect: "x") { id } }"#,
            ),
            (
                "advanceDoomClock",
                r#"mutation { advanceDoomClock(sessionId: "{session}", delta: 1) { id } }"#,
            ),
            (
                "createPuzzleClock",
                r#"mutation { createPuzzleClock(sessionId: "{session}", label: "x", segmentsMax: 4) { id } }"#,
            ),
            (
                "advancePuzzleClock",
                r#"mutation { advancePuzzleClock(clockId: "{clock}", delta: 1) { id } }"#,
            ),
            (
                "grantSessionResource",
                r#"mutation { grantSessionResource(sessionId: "{session}", actorId: "{actor}", resourceType: "coin", amount: 1) { quantity } }"#,
            ),
            (
                "createShopListing",
                r#"mutation { createShopListing(actorId: "{actor}", itemId: "{item}", priceKind: RESOURCE, priceResourceType: "coin", priceResourceAmount: 1) { id } }"#,
            ),
            (
                "purchaseFromShop",
                r#"mutation { purchaseFromShop(listingId: "{listing}", buyerActorId: "{counterpart}") { id } }"#,
            ),
            (
                "configurePuzzleClockReward",
                r#"mutation { configurePuzzleClockReward(clockId: "{clock}", triggerSegment: 1, rewardResourceType: "coin", rewardResourceAmount: 1, recipientMode: TRIGGERING_ACTOR) { id } }"#,
            ),
            (
                "proposeResourceTrade",
                r#"mutation { proposeResourceTrade(sessionId: "{session}", fromActorId: "{actor}", fromResourceType: "coin", fromQuantity: 1, toActorId: "{counterpart}", toResourceType: "coin", toQuantity: 1) { id } }"#,
            ),
            (
                "acceptResourceTrade",
                r#"mutation { acceptResourceTrade(proposalId: "{proposal}") { quantity } }"#,
            ),
            (
                "declineResourceTrade",
                r#"mutation { declineResourceTrade(proposalId: "{proposal}") { id } }"#,
            ),
            (
                "spendResourceOnPuzzleClock",
                r#"mutation { spendResourceOnPuzzleClock(clockId: "{clock}", actorId: "{actor}", resourceType: "coin", quantity: 1) { id } }"#,
            ),
        ],
        not_world_scoped: &[],
        // What the session panel shows. Readable while paused.
        reads: &[
            "genieSession",
            "genieResourceHoldings",
            "genieTradeProposals",
            "genieShopListings",
            "geniePuzzleClockRewards",
        ],
        seed: &[
            // An actor needs a scene to stand in.
            SeedStep {
                key: "scene",
                field: "createScene",
                document: r#"mutation { createScene(input: { worldId: "{world}", name: "Seeded" }) { sceneId } }"#,
                pick: "/sceneId",
            },
            SeedStep {
                key: "actor",
                field: "createActor",
                document: r#"mutation { createActor(input: { worldId: "{world}", label: "Seller", isNpc: true }) { id } }"#,
                pick: "/id",
            },
            SeedStep {
                key: "counterpart",
                field: "createActor",
                document: r#"mutation { createActor(input: { worldId: "{world}", label: "Buyer", isNpc: false }) { id } }"#,
                pick: "/id",
            },
            SeedStep {
                key: "item",
                field: "createItem",
                document: r#"mutation { createItem(input: { worldId: "{world}", name: "Lamp" }) { id } }"#,
                pick: "/id",
            },
            SeedStep {
                key: "session",
                field: "startGenieSession",
                document: r#"mutation { startGenieSession(input: { worldId: "{world}", doomClockMax: 6 }) { id } }"#,
                pick: "/id",
            },
            SeedStep {
                key: "clock",
                field: "createPuzzleClock",
                document: r#"mutation { createPuzzleClock(sessionId: "{session}", label: "Seeded", segmentsMax: 4) { id } }"#,
                pick: "/id",
            },
            SeedStep {
                key: "listing",
                field: "createShopListing",
                document: r#"mutation { createShopListing(actorId: "{actor}", itemId: "{item}", priceKind: RESOURCE, priceResourceType: "coin", priceResourceAmount: 1) { id } }"#,
                pick: "/id",
            },
            SeedStep {
                key: "proposal",
                field: "proposeResourceTrade",
                document: r#"mutation { proposeResourceTrade(sessionId: "{session}", fromActorId: "{actor}", fromResourceType: "coin", fromQuantity: 1, toActorId: "{counterpart}", toResourceType: "coin", toQuantity: 1) { id } }"#,
                pick: "/id",
            },
        ],
    }
}
