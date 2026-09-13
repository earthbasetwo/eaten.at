//! Identity: DIDs, handles, DID documents, and their resolution.
//!
//! Site routes are DID-addressed, so [`IdentityResolver::resolve_did`] is on
//! the read path and [`IdentityResolver::resolve_handle`] is only used for
//! the `/@handle` lookup redirect and for display.

mod did;
mod dns;
mod document;
mod handle;
mod hostname;
mod resolver;

pub use did::{Did, DidError, DidMethod};
pub use dns::{DnsError, DnsResolver, StaticDns, SystemDns};
pub use document::{DidDocument, Service};
pub use handle::Handle;
pub use hostname::HostnameError;
pub use resolver::{Identity, IdentityConfig, IdentityError, IdentityResolver};
