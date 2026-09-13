//! Identity resolution for the OAuth client, through our own resolver so
//! the guarded client, the PLC directory setting, and the development
//! overrides all apply to sign-in exactly as they do to reading.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use atrium_api::did_doc::{DidDocument as AtriumDocument, Service as AtriumService};
use atrium_api::types::string::{Did as AtriumDid, Handle as AtriumHandle};
use atrium_common::resolver::Resolver;

use crate::identity::{Did, Handle, Identity, IdentityError, IdentityResolver};

/// The future an [`IdentitySource`] returns.
pub type IdentityFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Identity, IdentityError>> + Send + 'a>>;

/// Something that turns a DID or handle into an [`Identity`]. The plain
/// resolver implements it; an application may wrap it in a cache.
pub trait IdentitySource: Send + Sync + fmt::Debug {
    fn did<'a>(&'a self, did: &'a Did) -> IdentityFuture<'a>;
    fn handle<'a>(&'a self, handle: &'a Handle) -> IdentityFuture<'a>;
}

impl IdentitySource for IdentityResolver {
    fn did<'a>(&'a self, did: &'a Did) -> IdentityFuture<'a> {
        Box::pin(self.resolve_did(did))
    }

    fn handle<'a>(&'a self, handle: &'a Handle) -> IdentityFuture<'a> {
        Box::pin(self.resolve_handle(handle))
    }
}

/// DID → DID document, in the library's types.
pub(super) struct DidResolverAdapter(pub(super) Arc<dyn IdentitySource>);

impl Resolver for DidResolverAdapter {
    type Input = AtriumDid;
    type Output = AtriumDocument;
    type Error = atrium_identity::Error;

    async fn resolve(&self, input: &AtriumDid) -> Result<AtriumDocument, atrium_identity::Error> {
        let did =
            Did::parse(input.as_str()).map_err(|e| atrium_identity::Error::Did(e.to_string()))?;
        let identity = self.0.did(&did).await.map_err(identity_error)?;
        Ok(document_of(&identity))
    }
}

/// Handle → DID, in the library's types.
pub(super) struct HandleResolverAdapter(pub(super) Arc<dyn IdentitySource>);

impl Resolver for HandleResolverAdapter {
    type Input = AtriumHandle;
    type Output = AtriumDid;
    type Error = atrium_identity::Error;

    async fn resolve(&self, input: &AtriumHandle) -> Result<AtriumDid, atrium_identity::Error> {
        let handle = Handle::parse(input.as_str())
            .map_err(|e| atrium_identity::Error::AtIdentifier(e.to_string()))?;
        let identity = self.0.handle(&handle).await.map_err(identity_error)?;
        AtriumDid::new(identity.did.as_str().to_owned())
            .map_err(|e| atrium_identity::Error::Did(e.to_owned()))
    }
}

/// Our document, in the library's shape. Only what the library reads is
/// carried over: aliases (for the handle check) and the PDS service.
fn document_of(identity: &Identity) -> AtriumDocument {
    let services = identity
        .document
        .service
        .iter()
        .filter_map(|s| {
            s.endpoint.as_str().map(|endpoint| AtriumService {
                id: s.id.clone(),
                r#type: s.kind.clone(),
                service_endpoint: endpoint.to_owned(),
            })
        })
        .collect::<Vec<_>>();
    AtriumDocument {
        context: None,
        id: identity.did.as_str().to_owned(),
        also_known_as: Some(identity.document.also_known_as.clone()),
        verification_method: None,
        service: Some(services),
    }
}

fn identity_error(err: IdentityError) -> atrium_identity::Error {
    match err {
        IdentityError::DidNotFound(_)
        | IdentityError::HandleNotFound(_)
        | IdentityError::HandleNotClaimed { .. }
        | IdentityError::AmbiguousDns(_) => atrium_identity::Error::NotFound,
        other => atrium_identity::Error::HttpClient(Box::new(other)),
    }
}
