//! OpenID Connect: reading identity out of what a provider says.
//!
//! Every function here parses a document written by someone else, so every
//! function here is total — arbitrary bytes produce `Err`/`None`, never a
//! panic. That is the property the proptests in each module exist to hold,
//! and it is not a nicety: these parsers sit on an unauthenticated code path
//! reachable by anyone who can make our server talk to a provider.
//!
//! **Nothing in this crate verifies a signature.** See [`id_token`] for the
//! trust argument and its limits.

pub mod discovery;
pub mod id_token;
pub mod userinfo;
