<!--
The prose of /legal/privacy.

BASE DRAFT — never reviewed by a lawyer. See ./README.md.

Written against what the software actually does, verified in the schema and
source on 2026-09-04, not adapted from a template. Where it makes a factual
claim ("no analytics", "sessions record no IP address") that claim was checked,
and it stops being true the moment someone adds the thing it denies. Treat a
change to data collection as a change to this file.

Two things an operator MUST replace before publishing, marked [OPERATOR] below:
who runs the instance, and how to contact them.
-->

ThunderForge is self-hosted software. This policy describes what **this
instance** collects and does with it. Whoever operates this instance decides
that, not the ThunderForge project — if you are reading this on someone's
server, they are the people who hold your data and the people to ask about it.

**Operator of this instance:** [OPERATOR — name and, if applicable, legal
entity]

**Contact for privacy questions:** [OPERATOR — email address]

## What this instance stores about you

Your account: a username, an email address, a password (stored only as a hash
that cannot be reversed into your password), and optionally a first and last
name if you supply them. If you enable two-factor authentication, the secret
that makes it work, encrypted.

If you sign in through another service — Discord, GitHub, a Keycloak server your
group runs — this instance also stores the identifier that service uses for you,
the email address it reports, and encrypted tokens that let it confirm you are
still you. It never receives or stores your password for that service.

Everything you make while playing: worlds, characters, items, abilities, lore
entries and their full revision history, scenes, maps and images you upload,
dice rolls, chat, and the record of who changed what and when. A revision
history is a record of your writing over time, kept deliberately so that a
mistake can be undone.

## What this instance does not do

It does not run analytics, tracking, advertising, or third-party measurement of
any kind. There is no such code in it.

It does not record your IP address or browser against your session. A session
records only that it exists, who it belongs to, when it expires, and whether it
was revoked.

It does not sell anything about you, and it does not share your data with anyone
except as described in the next section.

## Where your data goes

Nowhere, in the ordinary case. The application talks to its own database and its
own file storage, both run by this instance's operator.

There are two exceptions, and both are things you choose:

**Signing in through another service.** If you use one, this instance contacts
that service to confirm your sign-in and to read the profile information you
approved. That exchange is governed by that service's own privacy policy.

**Synchronising a world's lore to a repository you own.** If a Game Master
connects one, the lore in that world is copied to a service the operator of this
instance does not run and does not control. Anyone with access to that
repository can read it, **including entries that were restricted to some members
of the world** — repository access is not per-entry. A Game Master is shown this
before the first synchronisation and has to acknowledge it. Once content is
there, this instance can stop sending more and can disconnect the link, but it
cannot retract what was already sent.

## Who can see what you write

Other members of your world, according to the permissions its Game Master sets.
A Game Master can see their world's content. An instance administrator has
database access and can therefore see anything on the instance — that is true of
any self-hosted service, and it is worth knowing rather than assuming otherwise.

Nothing you write is public unless you make it so.

## Getting your data, and deleting it

You can export everything the instance holds about you, and you can delete your
account and the data you own. Both are actions you take yourself; neither
requires asking the operator.

**Deletion is not always total, and the exception is worth stating.** If a
copyright takedown notice was filed against content of yours, the record of that
notice and how it was resolved is kept even after your account is gone. It has
to be — the platform has to be able to show it handled notices consistently, and
a record that disappears when its subject deletes their account cannot do that.
The record is about the notice, not about your other activity.

Content you contributed to someone else's world may remain in that world. It is
theirs to run, and removing your account does not unmake their campaign.

## Children

This instance is not directed at children. [OPERATOR — if you are in a
jurisdiction with a specific minimum age, such as 13 under COPPA or 16 under
GDPR in some member states, state it here.]

## Changes

[OPERATOR — say how you will tell people this policy changed. For a small
private instance, telling your group is enough. For anything public, say what
you will do and then do it.]
