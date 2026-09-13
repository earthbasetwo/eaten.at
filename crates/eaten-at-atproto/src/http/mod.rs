//! Outbound HTTP with SSRF guards, size caps, timeouts, and per-host
//! concurrency limits. Every upstream fetch in the application goes
//! through [`GuardedClient`].

mod client;
mod hosts;
mod policy;

pub use client::{GuardedClient, HttpError, RawRequest, Response};
pub use hosts::{HostResolver, StaticHosts, SystemHosts};
pub use policy::{classify, IpClass, Policy};
