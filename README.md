# ThunderForgeVTT

An open source virtual tabletop that is currently purely a concept.

## Current Objective

The current objective of this is to get a proof of concept going using Rocket.rs, wasm, and yew.

## Curious on whats happening

- [Check out the discussions](https://github.com/ThunderForgeVTT/ThunderForgeVTT/discussions)
- [Take a look at project progress](https://github.com/ThunderForgeVTT/ThunderForgeVTT/projects)

### What features the PoC should include

- Login screen, Basic authentication (argon2 for passwords)
- Landing screen, landing page should redirect to login if no auth present or else provide a list of games the user belongs to.
- Game screen, a simplified game screen with a red square and a blue square.
- Game screen controls, wasd should move the token the user has ownership over.
- Token positions should update for each user connected.

### When is the PoC considered successful

- User can create a game
- User can can invite another user to join game
- Game window allows players to move tokens around
- Tokens move around with little to no lag (gauge the time delay for tic rates)

### What happens after a successful PoC

#### Step 2

This simplified approach is designed to not be pretty for a version 0.0.1 and if the PoC is successful, the next steps are to get a pipeline running in github for deployments. The project should have a solidified deployment plan with releases configured appropriately. Ultimately, the goal is to cement the concept of easy deployments and provide indivduals the oppertunity to test ThunderForgeVTT with a well established release model.

#### Step 3..X

- Begin solifify data models for each object type.
- Provide a module plugin layer.
- Allow plugins to be pre-installed or dynamically installed.
- Build a basic module.
- Build a system module for D&D 5th edition based on the SRD.
- Clean up the interface.
- Add a chat, roll, etc function.
- Provide a discord hook module for a rich discord interface.
- Integrate a character creation screen
- Provide a dndbeyond integration module
- TBD

## Timelines

This project is a larger project inspired by many great providers such as FoundryVTT, Roll20, and BattleMapp. The current initial release cannot be given a date becuase it takes time to create great software but rest assured this project is getting worked on daily.


