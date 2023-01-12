// DFN-467: clippy complains about the code generated by derive(Arbitrary)
#![cfg_attr(test, allow(clippy::unit_arg))]
//! HTTP requests that the Internet Computer is prepared to handle.

use super::Blob;
use crate::{
    crypto::SignedBytesWithoutDomainSeparator,
    messages::{
        message_id::hash_of_map, MessageId, ReadState, SignedIngressContent, UserQuery,
        UserSignature,
    },
    Height, Time, UserId,
};
use derive_more::Display;
use ic_base_types::{CanisterId, CanisterIdError, PrincipalId};
use ic_crypto_tree_hash::{MixedHashTree, Path};
use maplit::btreemap;
#[cfg(test)]
use proptest_derive::Arbitrary;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, convert::TryFrom, error::Error, fmt};

/// Describes the fields of a canister update call as defined in
/// https://sdk.dfinity.org/docs/interface-spec/index.html#api-update.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[cfg_attr(test, derive(Arbitrary))]
pub struct HttpCanisterUpdate {
    pub canister_id: Blob,
    pub method_name: String,
    pub arg: Blob,
    pub sender: Blob,
    /// Indicates when the message should expire.  Represented as nanoseconds
    /// since UNIX epoch.
    pub ingress_expiry: u64,
    // Do not include omitted fields in MessageId calculation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<Blob>,
}

impl HttpCanisterUpdate {
    /// Returns the representation-independent hash.
    // TODO(EXC-236): Avoid the duplication between this method and the one in
    // `SignedIngressContent`.
    pub fn representation_independent_hash(&self) -> [u8; 32] {
        use RawHttpRequestVal::*;
        let mut map = btreemap! {
            "request_type".to_string() => String("call".to_string()),
            "canister_id".to_string() => Bytes(self.canister_id.0.clone()),
            "method_name".to_string() => String(self.method_name.clone()),
            "arg".to_string() => Bytes(self.arg.0.clone()),
            "ingress_expiry".to_string() => U64(self.ingress_expiry),
            "sender".to_string() => Bytes(self.sender.0.clone()),
        };
        if let Some(nonce) = &self.nonce {
            map.insert("nonce".to_string(), Bytes(nonce.0.clone()));
        }
        hash_of_map(&map)
    }

    pub fn id(&self) -> MessageId {
        MessageId::from(self.representation_independent_hash())
    }
}

/// Describes the contents of a /api/v2/canister/_/call request.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(test, derive(Arbitrary))]
#[serde(rename_all = "snake_case")]
#[serde(tag = "request_type")]
pub enum HttpCallContent {
    Call {
        #[serde(flatten)]
        update: HttpCanisterUpdate,
    },
}

impl HttpCallContent {
    /// Returns the representation-independent hash.
    pub fn representation_independent_hash(&self) -> [u8; 32] {
        let Self::Call { update } = self;
        update.representation_independent_hash()
    }

    pub fn ingress_expiry(&self) -> u64 {
        match self {
            Self::Call { update } => update.ingress_expiry,
        }
    }
}

/// Describes the fields of a canister query call (a query from a user to a
/// canister) as defined in https://sdk.dfinity.org/docs/interface-spec/index.html#api-query.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HttpUserQuery {
    pub canister_id: Blob,
    pub method_name: String,
    pub arg: Blob,
    pub sender: Blob,
    /// Indicates when the message should expire.  Represented as nanoseconds
    /// since UNIX epoch.
    pub ingress_expiry: u64,
    // Do not include omitted fields in MessageId calculation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<Blob>,
}

/// Describes the contents of a /api/v2/canister/_/query request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "request_type")]
pub enum HttpQueryContent {
    Query {
        #[serde(flatten)]
        query: HttpUserQuery,
    },
}

impl HttpQueryContent {
    pub fn representation_independent_hash(&self) -> [u8; 32] {
        match self {
            Self::Query { query } => query.representation_independent_hash(),
        }
    }

    pub fn id(&self) -> MessageId {
        MessageId::from(self.representation_independent_hash())
    }
}

/// A `read_state` request as defined in https://sdk.dfinity.org/docs/interface-spec/index.html#api-request-read-state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HttpReadState {
    pub sender: Blob,
    // A list of paths, where a path is itself a sequence of labels.
    pub paths: Vec<Path>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<Blob>,
    pub ingress_expiry: u64,
}

/// Describes the contents of a /api/v2/canister/_/read_state request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "request_type")]
pub enum HttpReadStateContent {
    ReadState {
        #[serde(flatten)]
        read_state: HttpReadState,
    },
}

impl HttpReadStateContent {
    pub fn representation_independent_hash(&self) -> [u8; 32] {
        match self {
            Self::ReadState { read_state } => read_state.representation_independent_hash(),
        }
    }

    pub fn id(&self) -> MessageId {
        MessageId::from(self.representation_independent_hash())
    }
}

impl HttpUserQuery {
    /// Returns the representation-independent hash.
    // TODO(EXC-235): Avoid the duplication between this method and the one in
    // `UserQuery`.
    pub fn representation_independent_hash(&self) -> [u8; 32] {
        use RawHttpRequestVal::*;
        let mut map = btreemap! {
            "request_type".to_string() => String("query".to_string()),
            "canister_id".to_string() => Bytes(self.canister_id.0.clone()),
            "method_name".to_string() => String(self.method_name.clone()),
            "arg".to_string() => Bytes(self.arg.0.clone()),
            "ingress_expiry".to_string() => U64(self.ingress_expiry),
            "sender".to_string() => Bytes(self.sender.0.clone()),
        };
        if let Some(nonce) = &self.nonce {
            map.insert("nonce".to_string(), Bytes(nonce.0.clone()));
        }
        hash_of_map(&map)
    }
}

impl HttpReadState {
    /// Returns the representation-independent hash.
    // TODO(EXC-237): Avoid the duplication between this method and the one in
    // `ReadState`.
    pub fn representation_independent_hash(&self) -> [u8; 32] {
        use RawHttpRequestVal::*;
        let mut map = btreemap! {
            "request_type".to_string() => String("read_state".to_string()),
            "ingress_expiry".to_string() => U64(self.ingress_expiry),
            "paths".to_string() => Array(self
                    .paths
                    .iter()
                    .map(|p| {
                        RawHttpRequestVal::Array(
                            p.iter()
                                .map(|b| RawHttpRequestVal::Bytes(b.clone().to_vec()))
                                .collect(),
                        )
                    })
                    .collect()),
            "sender".to_string() => Bytes(self.sender.0.clone()),
        };
        if let Some(nonce) = &self.nonce {
            map.insert("nonce".to_string(), Bytes(nonce.0.clone()));
        }
        hash_of_map(&map)
    }
}

/// A request envelope as defined in
/// https://sdk.dfinity.org/docs/interface-spec/index.html#authentication.
///
/// The content is either [`HttpCallContent`], [`HttpQueryContent`] or
/// [`HttpReadStateContent`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(test, derive(Arbitrary))]
pub struct HttpRequestEnvelope<C> {
    pub content: C,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender_pubkey: Option<Blob>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender_sig: Option<Blob>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender_delegation: Option<Vec<SignedDelegation>>,
}

/// A strongly-typed version of [`HttpRequestEnvelope`].
#[derive(Debug, Eq, PartialEq, Clone, Hash, Serialize, Deserialize)]
pub struct HttpRequest<C> {
    content: C,
    auth: Authentication,
}

/// The authentication associated with an HTTP request.
#[derive(Debug, Eq, PartialEq, Clone, Hash, Serialize, Deserialize)]
pub enum Authentication {
    Authenticated(UserSignature),
    Anonymous,
}

impl<C> TryFrom<&HttpRequestEnvelope<C>> for Authentication {
    type Error = HttpRequestError;

    fn try_from(env: &HttpRequestEnvelope<C>) -> Result<Self, Self::Error> {
        match (&env.sender_pubkey, &env.sender_sig, &env.sender_delegation) {
            (Some(pubkey), Some(signature), delegation) => {
                Ok(Authentication::Authenticated(UserSignature {
                    signature: signature.0.clone(),
                    signer_pubkey: pubkey.0.clone(),
                    sender_delegation: delegation.clone(),
                }))
            }
            (None, None, None) => Ok(Authentication::Anonymous),
            rest => Err(Self::Error::MissingPubkeyOrSignature(format!(
                "Got {:?}",
                rest
            ))),
        }
    }
}

/// Common attributes that all HTTP request contents should have.
pub trait HttpRequestContent {
    fn id(&self) -> MessageId;

    fn sender(&self) -> UserId;

    fn ingress_expiry(&self) -> u64;

    fn nonce(&self) -> Option<Vec<u8>>;
}

/// A trait implemented by HTTP requests that contain a `canister_id`.
pub trait HasCanisterId {
    fn canister_id(&self) -> CanisterId;
}

impl<C: HttpRequestContent> HttpRequest<C> {
    pub fn id(&self) -> MessageId {
        self.content.id()
    }

    pub fn sender(&self) -> UserId {
        self.content.sender()
    }

    pub fn ingress_expiry(&self) -> u64 {
        self.content.ingress_expiry()
    }

    pub fn nonce(&self) -> Option<Vec<u8>> {
        self.content.nonce()
    }
}

impl<C> HttpRequest<C> {
    pub fn content(&self) -> &C {
        &self.content
    }

    pub fn take_content(self) -> C {
        self.content
    }

    pub fn authentication(&self) -> &Authentication {
        &self.auth
    }
}

impl HttpRequestContent for UserQuery {
    fn id(&self) -> MessageId {
        self.id()
    }

    fn sender(&self) -> UserId {
        self.source
    }

    fn ingress_expiry(&self) -> u64 {
        self.ingress_expiry
    }

    fn nonce(&self) -> Option<Vec<u8>> {
        self.nonce.clone()
    }
}

impl HttpRequestContent for ReadState {
    fn id(&self) -> MessageId {
        self.id()
    }

    fn sender(&self) -> UserId {
        self.source
    }

    fn ingress_expiry(&self) -> u64 {
        self.ingress_expiry
    }

    fn nonce(&self) -> Option<Vec<u8>> {
        self.nonce.clone()
    }
}

impl TryFrom<HttpRequestEnvelope<HttpQueryContent>> for HttpRequest<UserQuery> {
    type Error = HttpRequestError;

    fn try_from(envelope: HttpRequestEnvelope<HttpQueryContent>) -> Result<Self, Self::Error> {
        let auth = Authentication::try_from(&envelope)?;
        match envelope.content {
            HttpQueryContent::Query { query } => Ok(HttpRequest {
                content: UserQuery::try_from(query)?,
                auth,
            }),
        }
    }
}

impl TryFrom<HttpRequestEnvelope<HttpReadStateContent>> for HttpRequest<ReadState> {
    type Error = HttpRequestError;

    fn try_from(envelope: HttpRequestEnvelope<HttpReadStateContent>) -> Result<Self, Self::Error> {
        let auth = Authentication::try_from(&envelope)?;
        match envelope.content {
            HttpReadStateContent::ReadState { read_state } => Ok(HttpRequest {
                content: ReadState::try_from(read_state)?,
                auth,
            }),
        }
    }
}

impl TryFrom<HttpRequestEnvelope<HttpCallContent>> for HttpRequest<SignedIngressContent> {
    type Error = HttpRequestError;

    fn try_from(envelope: HttpRequestEnvelope<HttpCallContent>) -> Result<Self, Self::Error> {
        let auth = Authentication::try_from(&envelope)?;
        match envelope.content {
            HttpCallContent::Call { update } => Ok(HttpRequest {
                content: SignedIngressContent::try_from(update)?,
                auth,
            }),
        }
    }
}

/// Errors returned by `HttpHandler` when processing ingress messages.
#[derive(Debug, Clone, Serialize)]
pub enum HttpRequestError {
    InvalidMessageId(String),
    InvalidIngressExpiry(String),
    InvalidDelegationExpiry(String),
    InvalidPrincipalId(String),
    MissingPubkeyOrSignature(String),
    InvalidEncoding(String),
}

impl From<serde_cbor::Error> for HttpRequestError {
    fn from(err: serde_cbor::Error) -> Self {
        HttpRequestError::InvalidEncoding(format!("{}", err))
    }
}

impl fmt::Display for HttpRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HttpRequestError::InvalidMessageId(msg) => write!(f, "invalid message ID: {}", msg),
            HttpRequestError::InvalidIngressExpiry(msg) => write!(f, "{}", msg),
            HttpRequestError::InvalidDelegationExpiry(msg) => write!(f, "{}", msg),
            HttpRequestError::InvalidPrincipalId(msg) => write!(f, "invalid princial id: {}", msg),
            HttpRequestError::MissingPubkeyOrSignature(msg) => {
                write!(f, "missing pubkey or signature: {}", msg)
            }
            HttpRequestError::InvalidEncoding(err) => write!(f, "Invalid CBOR encoding: {}", err),
        }
    }
}

impl Error for HttpRequestError {}

impl From<CanisterIdError> for HttpRequestError {
    fn from(err: CanisterIdError) -> Self {
        Self::InvalidPrincipalId(format!("Converting to canister id failed with {}", err))
    }
}

/// Describes a delegation map as defined in
/// https://sdk.dfinity.org/docs/interface-spec/index.html#certification-delegation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[cfg_attr(test, derive(Arbitrary))]
pub struct Delegation {
    pubkey: Blob,
    expiration: Time,
    targets: Option<Vec<Blob>>,
}

impl Delegation {
    pub fn new(pubkey: Vec<u8>, expiration: Time) -> Self {
        Self {
            pubkey: Blob(pubkey),
            expiration,
            targets: None,
        }
    }

    pub fn new_with_targets(pubkey: Vec<u8>, expiration: Time, targets: Vec<CanisterId>) -> Self {
        Self {
            pubkey: Blob(pubkey),
            expiration,
            targets: Some(targets.iter().map(|c| Blob(c.get().to_vec())).collect()),
        }
    }

    pub fn pubkey(&self) -> &Vec<u8> {
        &self.pubkey.0
    }

    pub fn expiration(&self) -> Time {
        self.expiration
    }

    pub fn targets(&self) -> Result<Option<BTreeSet<CanisterId>>, String> {
        match &self.targets {
            None => Ok(None),
            Some(targets) => {
                let mut target_canister_ids = BTreeSet::new();
                for target in targets {
                    target_canister_ids.insert(
                        CanisterId::new(
                            PrincipalId::try_from(target.0.as_slice())
                                .map_err(|e| format!("Error parsing canister ID: {}", e))?,
                        )
                        .map_err(|e| format!("Error parsing canister ID: {}", e))?,
                    );
                }
                Ok(Some(target_canister_ids))
            }
        }
    }
}

impl SignedBytesWithoutDomainSeparator for Delegation {
    fn as_signed_bytes_without_domain_separator(&self) -> Vec<u8> {
        use RawHttpRequestVal::*;

        let mut map = btreemap! {
            "pubkey" => Bytes(self.pubkey.0.clone()),
            "expiration" => U64(self.expiration.as_nanos_since_unix_epoch()),
        };
        if let Some(targets) = &self.targets {
            map.insert(
                "targets",
                Array(targets.iter().map(|t| Bytes(t.0.clone())).collect()),
            );
        }

        hash_of_map(&map).to_vec()
    }
}

/// Describes a delegation as defined in
/// https://sdk.dfinity.org/docs/interface-spec/index.html#certification-delegation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[cfg_attr(test, derive(Arbitrary))]
pub struct SignedDelegation {
    delegation: Delegation,
    signature: Blob,
}

impl SignedDelegation {
    pub fn new(delegation: Delegation, signature: Vec<u8>) -> Self {
        Self {
            delegation,
            signature: Blob(signature),
        }
    }

    pub fn delegation(&self) -> &Delegation {
        &self.delegation
    }

    pub fn take_delegation(self) -> Delegation {
        self.delegation
    }

    pub fn signature(&self) -> &Blob {
        &self.signature
    }
}

/// The different types of values supported in `RawHttpRequest`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RawHttpRequestVal {
    Bytes(#[serde(with = "serde_bytes")] Vec<u8>),
    String(String),
    U64(u64),
    Array(Vec<RawHttpRequestVal>),
}

/// The reply to an update call.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum HttpReply {
    CodeCall { arg: Blob },
    Empty {},
}

/// The response to `/api/v2/canister/_/{read_state|query}` with `request_type`
/// set to `query`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "status")]
pub enum HttpQueryResponse {
    Replied {
        reply: HttpQueryResponseReply,
    },
    Rejected {
        error_code: String,
        reject_code: u64,
        reject_message: String,
    },
}

/// The body of the `QueryResponse`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HttpQueryResponseReply {
    pub arg: Blob,
}

/// The response to a `read_state` request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HttpReadStateResponse {
    /// The CBOR-encoded `Certificate`.
    pub certificate: Blob,
}

/// A `Certificate` as defined in https://sdk.dfinity.org/docs/interface-spec/index.html#_certificate
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Certificate {
    pub tree: MixedHashTree,
    pub signature: Blob,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegation: Option<CertificateDelegation>,
}

/// A `CertificateDelegation` as defined in https://smartcontracts.org/docs/interface-spec/index.html#certification-delegation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CertificateDelegation {
    pub subnet_id: Blob,
    pub certificate: Blob,
}

/// Different stages required for the full initialization of the HttpHandler.
/// The fields are listed in order of execution.
#[derive(Debug, Display, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReplicaHealthStatus {
    Starting,
    WaitingForCertifiedState,
    WaitingForRootDelegation,
    CertifiedStateBehind,
    Healthy,
}

/// The response to `/api/v2/status`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct HttpStatusResponse {
    pub ic_api_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_key: Option<Blob>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub impl_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub impl_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replica_health_status: Option<ReplicaHealthStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certified_height: Option<Height>,
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{time::UNIX_EPOCH, AmountOf};
    use pretty_assertions::assert_eq;
    use serde::Serialize;
    use serde_cbor::Value;

    /// Makes sure that the serialized CBOR version of `obj` is the same as
    /// `Value`. Used when testing _outgoing_ messages from the HTTP
    /// Handler's point of view
    fn assert_cbor_ser_equal<T>(obj: &T, val: Value)
    where
        for<'de> T: Serialize,
    {
        assert_eq!(serde_cbor::value::to_value(obj).unwrap(), val)
    }

    fn text(text: &'static str) -> Value {
        Value::Text(text.to_string())
    }

    fn int(i: u64) -> Value {
        Value::Integer(i.into())
    }

    fn bytes(bs: &[u8]) -> Value {
        Value::Bytes(bs.to_vec())
    }

    #[test]
    fn encoding_read_query_response() {
        assert_cbor_ser_equal(
            &HttpQueryResponse::Replied {
                reply: HttpQueryResponseReply {
                    arg: Blob(b"some_bytes".to_vec()),
                },
            },
            Value::Map(btreemap! {
                text("status") => text("replied"),
                text("reply") => Value::Map(btreemap!{
                    text("arg") => bytes(b"some_bytes")
                })
            }),
        );
    }

    #[test]
    fn encoding_read_query_reject() {
        assert_cbor_ser_equal(
            &HttpQueryResponse::Rejected {
                reject_code: 1,
                reject_message: "system error".to_string(),
                error_code: "IC500".to_string(),
            },
            Value::Map(btreemap! {
                text("status") => text("rejected"),
                text("reject_code") => int(1),
                text("reject_message") => text("system error"),
                text("error_code") => text("IC500"),
            }),
        );
    }

    #[test]
    fn encoding_status_without_root_key() {
        assert_cbor_ser_equal(
            &HttpStatusResponse {
                ic_api_version: "foobar".to_string(),
                root_key: None,
                impl_version: Some("0.0".to_string()),
                impl_hash: None,
                replica_health_status: Some(ReplicaHealthStatus::Starting),
                certified_height: None,
            },
            Value::Map(btreemap! {
                text("ic_api_version") => text("foobar"),
                text("impl_version") => text("0.0"),
                text("replica_health_status") => text("starting"),
            }),
        );
    }

    #[test]
    fn encoding_status_with_root_key() {
        assert_cbor_ser_equal(
            &HttpStatusResponse {
                ic_api_version: "foobar".to_string(),
                root_key: Some(Blob(vec![1, 2, 3])),
                impl_version: Some("0.0".to_string()),
                impl_hash: None,
                replica_health_status: Some(ReplicaHealthStatus::Healthy),
                certified_height: None,
            },
            Value::Map(btreemap! {
                text("ic_api_version") => text("foobar"),
                text("root_key") => bytes(&[1, 2, 3]),
                text("impl_version") => text("0.0"),
                text("replica_health_status") => text("healthy"),
            }),
        );
    }

    #[test]
    fn encoding_status_without_health_status() {
        assert_cbor_ser_equal(
            &HttpStatusResponse {
                ic_api_version: "foobar".to_string(),
                root_key: Some(Blob(vec![1, 2, 3])),
                impl_version: Some("0.0".to_string()),
                impl_hash: None,
                replica_health_status: None,
                certified_height: None,
            },
            Value::Map(btreemap! {
                text("ic_api_version") => text("foobar"),
                text("root_key") => bytes(&[1, 2, 3]),
                text("impl_version") => text("0.0"),
            }),
        );
    }

    #[test]
    fn encoding_status_with_certified_height() {
        assert_cbor_ser_equal(
            &HttpStatusResponse {
                ic_api_version: "foobar".to_string(),
                root_key: Some(Blob(vec![1, 2, 3])),
                impl_version: Some("0.0".to_string()),
                impl_hash: None,
                replica_health_status: Some(ReplicaHealthStatus::Healthy),
                certified_height: Some(AmountOf::new(1)),
            },
            Value::Map(btreemap! {
                text("ic_api_version") => text("foobar"),
                text("root_key") => bytes(&[1, 2, 3]),
                text("impl_version") => text("0.0"),
                text("replica_health_status") => text("healthy"),
                text("certified_height") => serde_cbor::Value::Integer(1),
            }),
        );
    }

    #[test]
    fn encoding_delegation() {
        assert_cbor_ser_equal(
            &Delegation {
                pubkey: Blob(vec![1, 2, 3]),
                expiration: UNIX_EPOCH,
                targets: None,
            },
            Value::Map(btreemap! {
                text("pubkey") => bytes(&[1, 2, 3]),
                text("expiration") => int(0),
                text("targets") => Value::Null,
            }),
        );

        assert_cbor_ser_equal(
            &Delegation {
                pubkey: Blob(vec![1, 2, 3]),
                expiration: UNIX_EPOCH,
                targets: Some(vec![Blob(vec![4, 5, 6])]),
            },
            Value::Map(btreemap! {
                text("pubkey") => bytes(&[1, 2, 3]),
                text("expiration") => int(0),
                text("targets") => Value::Array(vec![bytes(&[4, 5, 6])]),
            }),
        );
    }

    #[test]
    fn encoding_signed_delegation() {
        assert_cbor_ser_equal(
            &SignedDelegation {
                delegation: Delegation {
                    pubkey: Blob(vec![1, 2, 3]),
                    expiration: UNIX_EPOCH,
                    targets: None,
                },
                signature: Blob(vec![4, 5, 6]),
            },
            Value::Map(btreemap! {
                text("delegation") => Value::Map(btreemap! {
                    text("pubkey") => bytes(&[1, 2, 3]),
                    text("expiration") => int(0),
                    text("targets") => Value::Null,
                }),
                text("signature") => bytes(&[4, 5, 6]),
            }),
        );
    }
}
