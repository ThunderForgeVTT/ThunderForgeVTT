# ThunderForgeVTT

> Use of AI disclaimer! I am using copilot and gemini to help me build my dream.
> In September of 2024 I had to stop because I was out of work and needed to focus on finding a job or lose it all.
> In May of 2026, I am picking this back up because I landed work and can continue to follow my dream.

> Note from the creator!
> I love what FoundrVTT and Roll20 and many others have done for us but they never fit what ive been trying to do.
> My goal with this passion project is to provide an environment thats agnostic if people are in person / online only / a mixed with great tools to run a game.
> At the end of the day, we play D&D or similar to escape reality for but a brief respite in a fantasy world and I feel a lot of the tools today get in the way of this.
> With this tool, I want it to help facilite the fantasy and not be an encomberence on it. I want it easy to host, easy to use, and resiliant.
> I have been burned too many times by dndbeyond trying to push dnd next which became 5.5 and hurting the source material.
> Burned with FoundryVTT major version upgrades breaking everything and in some cases major data loss.
> Burned by patreons for modules I have been using collecting fat paycheck but lagging so far behind on updates things get too unbearable.

> Closing Notes: This project is my passion, my love for the fantasy, and im pouring my heart and soul into its existence.
> I apologize for being offline for so long and using AI to help me build the dream.
> I hope you find this project useful once I am able to finally release a working version of it.
> This message isnt a cry for money, stars, or anything. Its a note that this might take some time as I am only one individual and I wanted to thank you all for the patients.

An open source virtual tabletop that is currently purely a concept.

## Current Objective

The current objective is to get a proof of concept going with the Rust backend and the pnpm-managed React/Vite frontend in apps/web.

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

## License

ThunderForgeVTT is licensed under the [GNU Affero General Public License v3.0 (AGPL-3.0-or-later)](LICENSE). Self-hosting is always free, including for commercial/community use — the AGPL's only additional condition beyond ordinary open-source terms is that anyone who runs a *modified* version of this software as a network service must also make that modified source available to their users (AGPL §13). No further restriction exists.
