# MVP 1 Roadmap

This document outlines the 10 phases for reaching the Minimum Viable Product (MVP) 1 for ThunderForgeVTT.

## Core Concepts and Objects

This section provides a high-level overview of the core objects and concepts that will be implemented as part of the MVP.

- **World:** A container for all the data related to a single game. This includes scenes, actors, game systems, etc.
- **World Events:** A log of all the changes that happen within a world. This is used to keep all clients in sync.
- **Game System:** A set of rules that govern how the game is played. This includes things like character stats, skills, and dice roll formulas (e.g., "1d6+STR"). The game system should be designed to be easily extendable and shareable.
- **Scene:** A single map or location within a world. It has a background, a grid, and can contain tokens, walls, and lights.
- **Actor:** A character or creature within the world. Actors have stats, skills, and other properties defined by the game system.
- **Actor Events:** A log of all the changes that happen to an actor.
- **Token:** A visual representation of an actor on a scene. Tokens have a position, a type (NPC, player, vehicle, etc.), and are bound to an actor.
- **Token Events:** A log of all the changes that happen to a token.
- **Actor-Token Binding:** The link between an actor and a token. This allows the token to display the actor's information and for the actor's stats to affect the token's behavior.
- **Permissions and Policies:** A system for controlling who can do what within a world. This will be used to define roles like "player", "trusted player", "assistant DM", and "owner".

## MVP 1 Roadmap

- [ ] **Phase 1: User Login**

  - Users can log in to the application.

- [ ] **Phase 2: World Creation**

  - Users can create a "world" with a basic game system/ruleset.

- [ ] **Phase 3: Scene Creation**

  - Users can create a scene within a world.
  - Users can set a background for the scene.
  - Users can add a grid pattern to the scene at layer zero.

- [ ] **Phase 4: Token Creation**

  - Users can create tokens of different types (NPC, player, vehicle, etc.).
  - Different token types should have distinct visual representations.
  - Users can add tokens to a scene.

- [ ] **Phase 5: Actor Stats and Customization**

  - Users can add stats and customizations to "Actors" (e.g., health).
  - Actors are bound to tokens.
  - This phase introduces more video game-like logic.

- [ ] **Phase 6: Walls and Lighting**

  - Users can add walls and lighting to a scene.
  - These elements should restrict token vision.

- [ ] **Phase 7: Scene Levels**

  - Users can add levels to a scene (e.g., upstairs, downstairs).
  - Each level can have its own set of walls and token assignments.

- [ ] **Phase - 8: Game System Integration**

  - The application should enforce the rules of the game system loaded onto the world.
  - This includes basic mechanics like movement speed, considering actor specs.
  - This is where the Bevy engine should be utilized.

- [ ] **Phase 9: Multiplayer**

  - The owner of a world can invite other players via an invite code or a shareable link.
  - Invited players can join the world.
  - Players can select their "player" type actor as their character.
  - The Game Master (GM) should be able to override character selection.

- [ ] **Phase 10: Permissions Model**
  - A robust permissions model should be implemented.
  - The Dungeon Master (DM) can edit policies for different roles (player, trusted player, assistant DM).
  - The DM can promote other players to owner.

## Post-MVP

- **Sharing and Federation:**
  - Users can create and share their own game systems, tokens, maps, etc. via a share code.
  - The system should be able to talk to other systems to share content (federation).
- **Marketplace:**
  - A marketplace where users can upload and share their creations.
