//! The three tables of `play_pause_surface_tests`, and who calls each gated
//! field. Apart from the tests so neither file passes the length limit, and
//! public under `test-support` so the app crate's merged-schema test reads the
//! same tables rather than a copy; read `play_pause_surface_tests` first.

/// World-scoped: each calls the gate, and each is run below against a paused
/// world. `{name}` placeholders are the rows `seed` makes before the pause.
pub const GATED: &[(&str, &str)] = &[
    // --- play itself (T029, T030) ---------------------------------------
    ("heartbeat", r#"mutation { heartbeat(worldId: "{world}") }"#),
    (
        "worldSyncPlan",
        r#"{ worldSyncPlan(worldId: "{world}", held: []) { __typename } }"#,
    ),
    (
        "worldEventsSince",
        r#"{ worldEventsSince(worldId: "{world}", afterId: 0) { __typename } }"#,
    ),
    (
        "launchScene",
        r#"mutation { launchScene(worldId: "{world}", sceneId: "{scene}") { __typename } }"#,
    ),
    // Answers with a report, never an error; see `REPORTS_INSTEAD`.
    (
        "reconcileQueuedChanges",
        r#"mutation { reconcileQueuedChanges(worldId: "{world}", changes: [{ localId: "a", command: { type: "upsert_token", token: { id: "{token}", x: 1.0 } } }]) { reason } }"#,
    ),
    (
        "sendPeerSignal",
        r#"mutation { sendPeerSignal(input: { worldId: "{world}", fromSessionId: "sess-a", toSessionId: "sess-b", payload: "{}" }) }"#,
    ),
    // --- streams (T021) --------------------------------------------------
    (
        "worldEventsCreated",
        r#"subscription { worldEventsCreated(worldId: "{world}") { __typename } }"#,
    ),
    (
        "playersOnline",
        r#"subscription { playersOnline(worldId: "{world}") { __typename } }"#,
    ),
    (
        "playField",
        r#"subscription { playField(worldId: "{world}", clientId: "window-a") { __typename } }"#,
    ),
    (
        "peerSignals",
        r#"subscription { peerSignals(worldId: "{world}", sessionId: "sess-a") { __typename } }"#,
    ),
    (
        "worldActorSystemDataUpdated",
        r#"subscription { worldActorSystemDataUpdated(worldId: "{world}", gameSystemId: "test-system") { __typename } }"#,
    ),
    // --- scenes, fog, exploration (T031, T033) ---------------------------
    (
        "createScene",
        r#"mutation { createScene(input: { worldId: "{world}", name: "x" }) { __typename } }"#,
    ),
    (
        "updateScene",
        r#"mutation { updateScene(sceneId: "{scene}", input: { name: "x" }) { __typename } }"#,
    ),
    (
        "updateSceneHidden",
        r#"mutation { updateSceneHidden(sceneId: "{scene}", hidden: true) { __typename } }"#,
    ),
    (
        "updateSceneAmbientLight",
        r#"mutation { updateSceneAmbientLight(sceneId: "{scene}", ambientLight: "dim") { __typename } }"#,
    ),
    (
        "deleteScene",
        r#"mutation { deleteScene(sceneId: "{scene}") }"#,
    ),
    (
        "updateFogMask",
        r#"mutation { updateFogMask(input: { sceneId: "{scene}", bitmapDataBase64: "AA==", width: 1, height: 1 }) { __typename } }"#,
    ),
    (
        "setSceneExploration",
        r#"mutation { setSceneExploration(sceneId: "{scene}", enabled: true) }"#,
    ),
    (
        "resetSceneExploration",
        r#"mutation { resetSceneExploration(sceneId: "{scene}") }"#,
    ),
    (
        "bringPartyToScene",
        r#"mutation { bringPartyToScene(input: { sceneId: "{scene}", actorIds: ["{actor}"] }) { __typename } }"#,
    ),
    ("uploadCanvasImage", r#"UPLOAD uploadCanvasImage"#),
    // --- tokens (T031) ---------------------------------------------------
    (
        "createToken",
        r#"mutation { createToken(input: { sceneId: "{scene}", x: 0, y: 0 }) { __typename } }"#,
    ),
    (
        "updateToken",
        r#"mutation { updateToken(tokenId: "{token}", input: { x: 1 }) { __typename } }"#,
    ),
    (
        "deleteToken",
        r#"mutation { deleteToken(tokenId: "{token}") }"#,
    ),
    (
        "moveOwnToken",
        r#"mutation { moveOwnToken(tokenId: "{token}", x: 1, y: 1) { __typename } }"#,
    ),
    (
        "setOwnPrimaryTokenPhoto",
        r#"mutation { setOwnPrimaryTokenPhoto(tokenId: "{token}", photoUrl: "/x.webp") { __typename } }"#,
    ),
    (
        "setTokenNameVisibility",
        r#"mutation { setTokenNameVisibility(tokenId: "{token}", visible: true) { __typename } }"#,
    ),
    (
        "setTokenDisclosure",
        r#"mutation { setTokenDisclosure(input: { tokenId: "{token}", resourceId: "hp", state: VISIBLE }) { __typename } }"#,
    ),
    (
        "pickUpPlacedItem",
        r#"mutation { pickUpPlacedItem(input: { tokenId: "{placedItem}", actorId: "{actor}" }) { __typename } }"#,
    ),
    (
        "createWorldToken",
        r#"mutation { createWorldToken(input: { worldId: "{world}" }) { __typename } }"#,
    ),
    (
        "upsertWorldToken",
        r#"mutation { upsertWorldToken(input: { worldId: "{world}", tokenId: "{worldToken}", x: 1 }) { __typename } }"#,
    ),
    (
        "moveToken",
        r#"mutation { moveToken(input: { tokenId: "{worldToken}", x: 1, y: 1 }) { __typename } }"#,
    ),
    (
        "deleteWorldToken",
        r#"mutation { deleteWorldToken(tokenId: "{worldToken}") }"#,
    ),
    // --- walls, doors, lights, shapes (T031) -----------------------------
    (
        "createWall",
        r#"mutation { createWall(input: { sceneId: "{scene}", x1: 0, y1: 0, x2: 1, y2: 1 }) { __typename } }"#,
    ),
    (
        "updateWall",
        r#"mutation { updateWall(wallId: "{wall}", input: { x1: 2 }) { __typename } }"#,
    ),
    ("deleteWall", r#"mutation { deleteWall(wallId: "{wall}") }"#),
    (
        "setDoorDesignation",
        r#"mutation { setDoorDesignation(wallId: "{wall}", isDoor: true) }"#,
    ),
    (
        "setDoorLock",
        r#"mutation { setDoorLock(wallId: "{door}", locked: true) }"#,
    ),
    (
        "setDoorSecret",
        r#"mutation { setDoorSecret(wallId: "{door}", secret: true) }"#,
    ),
    (
        "createLightSource",
        r#"mutation { createLightSource(input: { sceneId: "{scene}", x: 0, y: 0, radius: 1 }) { __typename } }"#,
    ),
    (
        "updateLightSource",
        r#"mutation { updateLightSource(lightId: "{light}", input: { x: 2 }) { __typename } }"#,
    ),
    (
        "deleteLightSource",
        r#"mutation { deleteLightSource(lightId: "{light}") }"#,
    ),
    (
        "createShape",
        r#"mutation { createShape(input: { sceneId: "{scene}", kind: RECT, geometry: { x: 0, y: 0, w: 1, h: 1 } }) { __typename } }"#,
    ),
    (
        "updateShape",
        r#"mutation { updateShape(shapeId: "{shape}", input: { text: "x" }) { __typename } }"#,
    ),
    (
        "deleteShape",
        r#"mutation { deleteShape(shapeId: "{shape}") }"#,
    ),
    // --- interactives (T032) ---------------------------------------------
    (
        "createInteractive",
        r#"mutation { createInteractive(input: { sceneId: "{scene}", subjectKind: "door", subjectRef: "{door}", trigger: "click", activation: "anyone" }) { __typename } }"#,
    ),
    (
        "updateInteractive",
        r#"mutation { updateInteractive(interactiveId: "{interactive}", input: { trigger: "click" }) { __typename } }"#,
    ),
    (
        "deleteInteractive",
        r#"mutation { deleteInteractive(interactiveId: "{interactive}") }"#,
    ),
    (
        "resetInteractive",
        r#"mutation { resetInteractive(interactiveId: "{interactive}") { __typename } }"#,
    ),
    (
        "activateInteractive",
        r#"mutation { activateInteractive(interactiveId: "{interactive}") { __typename } }"#,
    ),
    (
        "approveRequest",
        r#"mutation { approveRequest(requestId: "{request}") { __typename } }"#,
    ),
    (
        "refuseRequest",
        r#"mutation { refuseRequest(requestId: "{request}") { __typename } }"#,
    ),
    // --- combat, chat, rolls (T033) --------------------------------------
    (
        "startCombat",
        r#"mutation { startCombat(input: { worldId: "{world}" }) { __typename } }"#,
    ),
    (
        "addCombatant",
        r#"mutation { addCombatant(input: { combatId: "{combat}", label: "x" }) { __typename } }"#,
    ),
    (
        "updateCombatant",
        r#"mutation { updateCombatant(input: { combatantId: "{combatant}", initiative: 3 }) { __typename } }"#,
    ),
    (
        "removeCombatant",
        r#"mutation { removeCombatant(combatantId: "{combatant}") { __typename } }"#,
    ),
    (
        "changeHitPoints",
        r#"mutation { changeHitPoints(tokenId: "{token}", kind: DAMAGE, amount: 1) { __typename } }"#,
    ),
    (
        "setTokenLink",
        r#"mutation { setTokenLink(tokenId: "{token}", linked: false) { __typename } }"#,
    ),
    (
        "setActorUnique",
        r#"mutation { setActorUnique(actorId: "{actor}", unique: true) { __typename } }"#,
    ),
    (
        "advanceTurn",
        r#"mutation { advanceTurn(combatId: "{combat}") { __typename } }"#,
    ),
    (
        "endCombat",
        r#"mutation { endCombat(combatId: "{combat}") { __typename } }"#,
    ),
    (
        "sendChatMessage",
        r#"mutation { sendChatMessage(input: { worldId: "{world}", body: "x" }) { __typename } }"#,
    ),
    (
        "rollDice",
        r#"mutation { rollDice(input: { worldId: "{world}", formula: "1d20" }) { __typename } }"#,
    ),
    (
        "rollCheck",
        r#"mutation { rollCheck(worldId: "{world}", actorId: "{actor}", checkId: "x") { __typename } }"#,
    ),
    (
        "setAuthoringToolGrant",
        r#"mutation { setAuthoringToolGrant(worldId: "{world}", worldMemberId: "{playerMember}", tool: "walls", granted: true) }"#,
    ),
    // --- actors ------------------------------------------------------------
    (
        "createActor",
        r#"mutation { createActor(input: { worldId: "{world}", label: "x", isNpc: true }) { __typename } }"#,
    ),
    (
        "updateActor",
        r#"mutation { updateActor(input: { actorId: "{actor}", label: "y" }) { __typename } }"#,
    ),
    (
        "setActorPermission",
        r#"mutation { setActorPermission(input: { actorId: "{actor}", userId: "{player}", level: VIEWER }) { __typename } }"#,
    ),
    (
        "removeActorPermission",
        r#"mutation { removeActorPermission(actorId: "{actor}", userId: "{player}") }"#,
    ),
    (
        "createActorShareLink",
        r#"mutation { createActorShareLink(actorId: "{actor}", attestation: { termsVersionId: "sharing-terms@0000000000000000" }) { __typename } }"#,
    ),
    (
        "revokeActorShareLink",
        r#"mutation { revokeActorShareLink(shareId: "{actorShare}") }"#,
    ),
    (
        "copySharedActorToWorld",
        r#"mutation { copySharedActorToWorld(input: { shareCode: "nothing", destinationWorldId: "{world}" }) { __typename } }"#,
    ),
    ("uploadActorImage", r#"UPLOAD uploadActorImage"#),
    (
        "removeActorImage",
        r#"mutation { removeActorImage(actorId: "{actor}", role: "portrait") }"#,
    ),
    (
        "updateActorSystemData",
        r#"mutation { updateActorSystemData(input: { actorId: "{actor}", gameSystemId: "test-system", dataType: "abilities", data: {} }) { __typename } }"#,
    ),
    (
        "attachAbilityToActor",
        r#"mutation { attachAbilityToActor(input: { actorId: "{actor}", abilityId: "{ability}" }) { __typename } }"#,
    ),
    (
        "detachAbilityFromActor",
        r#"mutation { detachAbilityFromActor(entryId: "{actorAbility}") }"#,
    ),
    (
        "claimActor",
        r#"mutation { claimActor(worldId: "{world}", actorId: "{actor}") { __typename } }"#,
    ),
    (
        "createAndClaimActor",
        r#"mutation { createAndClaimActor(worldId: "{world}", name: "x") { __typename } }"#,
    ),
    (
        "setActorAvailability",
        r#"mutation { setActorAvailability(actorId: "{actor}", available: true) { __typename } }"#,
    ),
    (
        "unclaimActor",
        r#"mutation { unclaimActor(actorId: "{actor}") { __typename } }"#,
    ),
    (
        "setPlayerCharacterBinding",
        r#"mutation { setPlayerCharacterBinding(worldId: "{world}", worldMemberId: "{playerMember}", actorId: "{actor}") { __typename } }"#,
    ),
    // --- abilities ---------------------------------------------------------
    (
        "createAbility",
        r#"mutation { createAbility(input: { worldId: "{world}", name: "x", classification: "spell" }) { __typename } }"#,
    ),
    (
        "updateAbility",
        r#"mutation { updateAbility(input: { abilityId: "{ability}", name: "y" }) { __typename } }"#,
    ),
    (
        "deleteAbility",
        r#"mutation { deleteAbility(abilityId: "{ability}") }"#,
    ),
    (
        "addAbilityEffect",
        r#"mutation { addAbilityEffect(abilityId: "{ability}", effect: { effectType: HEAL, formula: "1", target: "self" }) { __typename } }"#,
    ),
    (
        "updateAbilityEffect",
        r#"mutation { updateAbilityEffect(effectId: "{abilityEffect}", effect: { effectType: HEAL, formula: "2", target: "self" }) { __typename } }"#,
    ),
    (
        "removeAbilityEffect",
        r#"mutation { removeAbilityEffect(effectId: "{abilityEffect}") }"#,
    ),
    (
        "setAbilityGmOnly",
        r#"mutation { setAbilityGmOnly(abilityId: "{ability}", gmOnly: true) { __typename } }"#,
    ),
    (
        "setAbilityPermission",
        r#"mutation { setAbilityPermission(input: { abilityId: "{ability}", userId: "{player}", level: VIEWER }) { __typename } }"#,
    ),
    (
        "removeAbilityPermission",
        r#"mutation { removeAbilityPermission(abilityId: "{ability}", userId: "{player}") }"#,
    ),
    (
        "createAbilityShareLink",
        r#"mutation { createAbilityShareLink(abilityId: "{ability}", attestation: { termsVersionId: "sharing-terms@0000000000000000" }) { __typename } }"#,
    ),
    (
        "revokeAbilityShareLink",
        r#"mutation { revokeAbilityShareLink(shareId: "{abilityShare}") }"#,
    ),
    (
        "copySharedAbilityToWorld",
        r#"mutation { copySharedAbilityToWorld(input: { shareCode: "nothing", destinationWorldId: "{world}" }) { __typename } }"#,
    ),
    // --- items and inventory -------------------------------------------------
    (
        "createItem",
        r#"mutation { createItem(input: { worldId: "{world}", name: "x" }) { __typename } }"#,
    ),
    (
        "updateItem",
        r#"mutation { updateItem(input: { itemId: "{item}", name: "y" }) { __typename } }"#,
    ),
    ("deleteItem", r#"mutation { deleteItem(itemId: "{item}") }"#),
    (
        "addItemEffect",
        r#"mutation { addItemEffect(itemId: "{item}", effect: { effectType: HEAL, formula: "1", target: "self" }) { __typename } }"#,
    ),
    (
        "updateItemEffect",
        r#"mutation { updateItemEffect(effectId: "{itemEffect}", effect: { effectType: HEAL, formula: "2", target: "self" }) { __typename } }"#,
    ),
    (
        "removeItemEffect",
        r#"mutation { removeItemEffect(effectId: "{itemEffect}") }"#,
    ),
    (
        "setItemPermission",
        r#"mutation { setItemPermission(input: { itemId: "{item}", userId: "{player}", level: VIEWER }) { __typename } }"#,
    ),
    (
        "removeItemPermission",
        r#"mutation { removeItemPermission(itemId: "{item}", userId: "{player}") }"#,
    ),
    (
        "createItemShareLink",
        r#"mutation { createItemShareLink(itemId: "{item}", attestation: { termsVersionId: "sharing-terms@0000000000000000" }) { __typename } }"#,
    ),
    (
        "revokeItemShareLink",
        r#"mutation { revokeItemShareLink(shareId: "{itemShare}") }"#,
    ),
    (
        "copySharedItemToWorld",
        r#"mutation { copySharedItemToWorld(input: { shareCode: "nothing", destinationWorldId: "{world}" }) { __typename } }"#,
    ),
    (
        "setItemPrice",
        r#"mutation { setItemPrice(input: { itemId: "{item}", amount: 5 }) { __typename } }"#,
    ),
    (
        "clearItemPrice",
        r#"mutation { clearItemPrice(itemId: "{item}") }"#,
    ),
    (
        "attachAbilityToItem",
        r#"mutation { attachAbilityToItem(input: { itemId: "{item}", abilityId: "{ability}" }) { __typename } }"#,
    ),
    (
        "addItemToInventory",
        r#"mutation { addItemToInventory(input: { actorId: "{actor}", itemId: "{item}", quantity: 1 }) { __typename } }"#,
    ),
    (
        "adjustInventoryQuantity",
        r#"mutation { adjustInventoryQuantity(input: { inventoryEntryId: "{inventoryEntry}", quantity: 2 }) { __typename } }"#,
    ),
    (
        "removeInventoryEntry",
        r#"mutation { removeInventoryEntry(inventoryEntryId: "{inventoryEntry}") }"#,
    ),
    // --- lore (T034) ---------------------------------------------------------
    (
        "createLoreEntry",
        r#"mutation { createLoreEntry(input: { worldId: "{world}", title: "x" }) { __typename } }"#,
    ),
    (
        "updateLoreEntry",
        r#"mutation { updateLoreEntry(input: { loreEntryId: "{lore}", title: "y" }) { __typename } }"#,
    ),
    (
        "deleteLoreEntry",
        r#"mutation { deleteLoreEntry(loreEntryId: "{lore}") }"#,
    ),
    (
        "restoreLoreRevision",
        r#"mutation { restoreLoreRevision(revisionId: "{loreRevision}") { __typename } }"#,
    ),
    (
        "setLorePermission",
        r#"mutation { setLorePermission(input: { loreEntryId: "{lore}", userId: "{player}", level: VIEWER }) { __typename } }"#,
    ),
    (
        "removeLorePermission",
        r#"mutation { removeLorePermission(loreEntryId: "{lore}", userId: "{player}") }"#,
    ),
    ("uploadLoreImage", r#"UPLOAD uploadLoreImage"#),
    (
        "moveLoreEntry",
        r#"mutation { moveLoreEntry(input: { loreEntryId: "{lore}" }) { __typename } }"#,
    ),
    (
        "addLoreTag",
        r#"mutation { addLoreTag(input: { loreEntryId: "{lore}", tag: "x" }) }"#,
    ),
    (
        "removeLoreTag",
        r#"mutation { removeLoreTag(input: { loreEntryId: "{lore}", tag: "x" }) }"#,
    ),
    (
        "beginLoreRepositoryConnection",
        r#"mutation { beginLoreRepositoryConnection(worldId: "{world}") { __typename } }"#,
    ),
    (
        "completeLoreRepositoryConnection",
        r#"mutation { completeLoreRepositoryConnection(input: { worldId: "{world}", grantResponse: "x" }) { __typename } }"#,
    ),
    (
        "acknowledgeLoreSyncNotice",
        r#"mutation { acknowledgeLoreSyncNotice(worldId: "{world}") { __typename } }"#,
    ),
    (
        "resolveLoreSyncDivergence",
        r#"mutation { resolveLoreSyncDivergence(worldId: "{world}", resolution: ABANDON_CONNECTION) }"#,
    ),
    (
        "acceptLoreIncomingChange",
        r#"mutation { acceptLoreIncomingChange(worldId: "{world}", changeId: "00000000-0000-0000-0000-000000000001") }"#,
    ),
    (
        "declineLoreIncomingChange",
        r#"mutation { declineLoreIncomingChange(worldId: "{world}", changeId: "00000000-0000-0000-0000-000000000001") }"#,
    ),
    (
        "removeLoreRepositoryConnection",
        r#"mutation { removeLoreRepositoryConnection(worldId: "{world}") }"#,
    ),
    // --- collections -------------------------------------------------------
    (
        "createCollection",
        r#"mutation { createCollection(input: { worldId: "{world}", name: "x" }) { __typename } }"#,
    ),
    (
        "updateCollection",
        r#"mutation { updateCollection(input: { collectionId: "{collection}", name: "y" }) { __typename } }"#,
    ),
    (
        "deleteCollection",
        r#"mutation { deleteCollection(collectionId: "{collection}") }"#,
    ),
    (
        "addCollectionMember",
        r#"mutation { addCollectionMember(input: { collectionId: "{collection}", memberType: "item", memberId: "{item}" }) { __typename } }"#,
    ),
    (
        "removeCollectionMember",
        r#"mutation { removeCollectionMember(collectionId: "{collection}", memberId: "{collectionMember}") }"#,
    ),
    (
        "createCollectionShareLink",
        r#"mutation { createCollectionShareLink(collectionId: "{collection}", attestation: { termsVersionId: "sharing-terms@0000000000000000" }) { __typename } }"#,
    ),
    (
        "revokeCollectionShareLink",
        r#"mutation { revokeCollectionShareLink(shareId: "{collectionShare}") }"#,
    ),
    (
        "copySharedCollectionToWorld",
        r#"mutation { copySharedCollectionToWorld(shareCode: "nothing", destinationWorldId: "{world}") { __typename } }"#,
    ),
    // --- the world's book list and its deltas (T034) -----------------------
    (
        "switchOnCompendium",
        r#"mutation { switchOnCompendium(worldId: "{world}", compendiumId: "{compendium}") { __typename } }"#,
    ),
    (
        "switchOffCompendium",
        r#"mutation { switchOffCompendium(worldId: "{world}", compendiumId: "{compendium}", confirm: true) { __typename } }"#,
    ),
    (
        "changeWorldEntry",
        r#"mutation { changeWorldEntry(worldId: "{world}", compendiumId: "{compendium}", kind: "monster", name: "x", proseText: "y") { __typename } }"#,
    ),
    (
        "hideWorldEntry",
        r#"mutation { hideWorldEntry(worldId: "{world}", compendiumId: "{compendium}", kind: "monster", name: "x") { __typename } }"#,
    ),
    (
        "addWorldEntry",
        r#"mutation { addWorldEntry(worldId: "{world}", compendiumId: "{compendium}", kind: "monster", name: "x", proseText: "y") { __typename } }"#,
    ),
    (
        "removeKeptAddition",
        r#"mutation { removeKeptAddition(worldId: "{world}", additionId: "00000000-0000-0000-0000-000000000001") }"#,
    ),
    (
        "restoreWorldEntry",
        r#"mutation { restoreWorldEntry(worldId: "{world}", compendiumId: "{compendium}", kind: "monster", name: "x") { __typename } }"#,
    ),
    // --- world settings (T034) ---------------------------------------------
    (
        "renameWorld",
        r#"mutation { renameWorld(worldId: "{world}", worldName: "Renamed while paused") { __typename } }"#,
    ),
    (
        "updateWorldSessionNotes",
        r#"mutation { updateWorldSessionNotes(input: { worldId: "{world}", notes: "x" }) { __typename } }"#,
    ),
    (
        "updateWorldGameSystem",
        r#"mutation { updateWorldGameSystem(input: { worldId: "{world}", gameSystemId: "test-system" }) { __typename } }"#,
    ),
    (
        "updateWorldInterfacePack",
        r#"mutation { updateWorldInterfacePack(input: { worldId: "{world}" }) { __typename } }"#,
    ),
    (
        "updateWorldAllowPlayerCreatedActors",
        r#"mutation { updateWorldAllowPlayerCreatedActors(input: { worldId: "{world}", allow: true }) { __typename } }"#,
    ),
    (
        "updateWorldGenieResourceCarryover",
        r#"mutation { updateWorldGenieResourceCarryover(input: { worldId: "{world}", enabled: true }) { __typename } }"#,
    ),
    (
        "updateWorldDefaultSceneGridType",
        r#"mutation { updateWorldDefaultSceneGridType(input: { worldId: "{world}", gridType: "hex" }) { __typename } }"#,
    ),
    // --- invitations and membership (T034) -----------------------------------
    (
        "generateInviteCode",
        r#"mutation { generateInviteCode(input: { worldId: "{world}", maxUses: 1 }) { __typename } }"#,
    ),
    (
        "revokeInviteCode",
        r#"mutation { revokeInviteCode(inviteId: "{invite}") { __typename } }"#,
    ),
    (
        "rotateInviteCode",
        r#"mutation { rotateInviteCode(inviteId: "{invite}") { __typename } }"#,
    ),
    (
        "joinWorld",
        r#"mutation { joinWorld(input: { inviteCode: "{inviteCode}" }) { __typename } }"#,
    ),
    (
        "updateMemberRole",
        r#"mutation { updateMemberRole(input: { worldId: "{world}", userId: "{player}", role: "TrustedPlayer" }) { __typename } }"#,
    ),
    (
        "removeMember",
        r#"mutation { removeMember(worldId: "{world}", userId: "{player}") }"#,
    ),
];

/// Who calls a gated field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Who {
    /// The world's Owner, who runs it.
    GameMaster,
    /// A site admin who is a member with the Player role, so every role check
    /// they pass is the site-admin short-circuit the gate must not share.
    SiteAdmin,
    /// A plain Player.
    Player,
    /// A signed-in account that is not a member, and one that is also a site
    /// admin.
    Newcomer,
    NewcomerSiteAdmin,
}

/// Unless [`CALLED_AS`] says otherwise, every gated field is called by both.
pub const DEFAULT_CALLERS: &[Who] = &[Who::GameMaster, Who::SiteAdmin];

/// Gated fields that one of the default callers cannot reach at all, for a
/// reason that has nothing to do with a pause, and who calls them instead.
///
/// A default caller left out is still called, and must be refused too — so an
/// entry here can narrow who reaches the gate, never who gets through.
pub const CALLED_AS: &[(&str, &[Who], &str)] = &[
    (
        "joinWorld",
        &[Who::Newcomer, Who::NewcomerSiteAdmin],
        "only someone who is not yet a member joins",
    ),
    (
        "claimActor",
        &[Who::Player, Who::SiteAdmin],
        "a Game Master does not claim characters",
    ),
    (
        "createAndClaimActor",
        &[Who::Player, Who::SiteAdmin],
        "a Game Master does not claim characters",
    ),
    (
        "renameWorld",
        &[Who::GameMaster],
        "renaming is the Owner's alone, with no site-admin bypass",
    ),
    (
        "switchOnCompendium",
        &[Who::GameMaster],
        "managing a world's books has no site-admin bypass (spec 050)",
    ),
    (
        "switchOffCompendium",
        &[Who::GameMaster],
        "managing a world's books has no site-admin bypass (spec 050)",
    ),
    (
        "changeWorldEntry",
        &[Who::GameMaster],
        "managing a world's books has no site-admin bypass (spec 050)",
    ),
    (
        "hideWorldEntry",
        &[Who::GameMaster],
        "managing a world's books has no site-admin bypass (spec 050)",
    ),
    (
        "addWorldEntry",
        &[Who::GameMaster],
        "managing a world's books has no site-admin bypass (spec 050)",
    ),
    (
        "removeKeptAddition",
        &[Who::GameMaster],
        "managing a world's books has no site-admin bypass (spec 050)",
    ),
    (
        "restoreWorldEntry",
        &[Who::GameMaster],
        "managing a world's books has no site-admin bypass (spec 050)",
    ),
];

/// Answered with a per-change report rather than an error (research R5): an
/// error would leave the changes queued and replayed forever. Refused means
/// every change comes back `PLAY_PAUSED`.
pub const REPORTS_INSTEAD: &[&str] = &["reconcileQueuedChanges"];

/// Touches no single world's play, each with the reason. The last group
/// touches a world and is deliberately left open; read it twice.
pub const NOT_WORLD_SCOPED: &[&str] = &[
    // A subscription that counts. No world, no data.
    "tick",
    // Makes a world that did not exist; nothing to be paused.
    "createWorld",
    // A person's own account and sessions.
    "deleteMyData",
    "endSession",
    "endAllSessions",
    // A person's own standing, and their route back.
    "fileAppeal",
    "submitCounterNotice",
    // Reporting is open to anybody, about anything, paused or not — a pause is
    // often what a report leads to.
    "submitTakedownNotice",
    "submitLegalEnquiry",
    // `worldId` there is context for the operator reading it, not a write to
    // the world.
    "submitFeedback",
    // The account's own shelf (spec 049). A book on the shelf belongs to its
    // importer, not to any world.
    "createCompendiumFromImport",
    // --- touches a world, and deliberately not gated -------------------------
    //
    // The shelf again, but removal reaches into worlds: it takes the book off
    // every world's list it is switched on in, paused ones included, with
    // the deltas made against it. Left open because the book is the
    // importer's, and a pause on somebody else's world must not stop a person
    // withdrawing their own content from the instance (the takedown path
    // depends on exactly that).
    "removeCompendium",
    // Deleting a world ends it for good; a pause holds play still while an
    // operator looks, and is not a reason to keep a world its Owner wants
    // gone. Spec 051 names world deletion as staying allowed. The record of
    // the pause keeps its world name for that case (data-model.md).
    "deleteWorld",
    // No "leave world" mutation exists: `removeMember` refuses self-removal
    // and is gated as a membership change. Were one added, it belongs here —
    // leaving a paused world harms no one.
];

/// The operator's surface, and none of it gated: acting on paused worlds is
/// what an operator is for. Every one is in `admin_surface_tests::ADMIN_ONLY`
/// or guarded by `admin_user`.
pub const OPERATOR: &[&str] = &[
    "pauseWorldPlay",
    // Approving pauses a world, and deciding a request for a world already
    // paused is exactly the case it must not be refused in (FR-036).
    "decidePlayPauseRequest",
    // Lifting is only ever done to a paused world (FR-040).
    "liftWorldPlayPause",
    // Refuses anyone but a site admin: the operator's way to stop a world's
    // repository sync, which a pause should never stand in the way of.
    "deactivateLoreSync",
    "resolveModerationCase",
    "resolveAppeal",
    "executeTermination",
    "acknowledgeOperatorStatement",
    "updateOauthProvider",
    "updateManifestKey",
    "recalculateDiskUsage",
    "updateTwoFactorPolicy",
    "setInstanceAccessPolicy",
    "createInstanceInvitation",
    "revokeInstanceInvitation",
    "setGithubApplication",
    "checkGithubApplication",
    "updateInstanceSetting",
    "sendTestMail",
    "retryOutboxMessage",
    "abandonFeedbackDelivery",
    "resumeFeedbackDelivery",
    "setLegalEnquiryStatus",
];

/// The queries that start play, and so must be gated though they write
/// nothing (contract: *The closed list*).
pub const PLAY_STARTING_QUERIES: &[&str] = &["worldSyncPlan", "worldEventsSince"];
