//! Metrics exported by crypto

mod bls12_381_sig_cache;

use convert_case::{Case, Casing};
use core::fmt;
use ic_metrics::MetricsRegistry;
use prometheus::{Gauge, HistogramVec, IntCounterVec, IntGaugeVec};
use std::fmt::{Display, Formatter};
use std::time;
use std::time::Instant;
use strum::IntoEnumIterator;
use strum_macros::{EnumIter, IntoStaticStr};

/// Provides metrics for the crypto component.
///
/// This struct allows metrics being disabled and enabled.
pub struct CryptoMetrics {
    metrics: Option<Metrics>,
}

impl CryptoMetrics {
    /// Constructs CryptoMetrics that are disabled.
    pub fn none() -> Self {
        Self { metrics: None }
    }

    /// Constructs CryptoMetrics that are enabled if the metrics registry is
    /// some.
    pub fn new(registry: Option<&MetricsRegistry>) -> Self {
        Self {
            metrics: registry.map(Metrics::new),
        }
    }

    /// Returns `Instant::now()` iff metrics are enabled.
    ///
    /// This is a performance optimization to avoid calling `Instant::now()` if
    /// metrics are disabled. This may be relevant for very fast and frequent
    /// operations.
    pub fn now(&self) -> Option<Instant> {
        self.metrics.as_ref().map(|_| time::Instant::now())
    }

    /// Observes a lock acquisition duration. The `access` label is either
    /// 'read' or 'write'.
    ///
    /// This only observes the lock acquisition duration if metrics are enabled
    /// and `start_time` is `Some`.
    pub fn observe_lock_acquisition_duration_seconds(
        &self,
        name: &str,
        access: &str,
        start_time: Option<Instant>,
    ) {
        if let (Some(metrics), Some(start_time)) = (&self.metrics, start_time) {
            metrics
                .crypto_lock_acquisition_duration_seconds
                .with_label_values(&[name, access])
                .observe(start_time.elapsed().as_secs_f64());
        }
    }

    /// Observes a crypto method duration, measuring the the full duration,
    /// which includes actual cryptographic computation and the potential RPC overhead.
    /// `method_name` indicates the method's name, such as `BasicSignature::sign`.
    ///
    /// It observes the duration only if metrics are enabled, `start_time` is `Some`,
    /// and the metrics for `domain` are defined.
    pub fn observe_duration_seconds(
        &self,
        domain: MetricsDomain,
        scope: MetricsScope,
        method_name: &str,
        result: MetricsResult,
        start_time: Option<Instant>,
    ) {
        if let (Some(metrics), Some(start_time)) = (&self.metrics, start_time) {
            metrics
                .crypto_duration_seconds
                .with_label_values(&[
                    method_name,
                    &format!("{}", scope),
                    &format!("{}", domain),
                    &format!("{}", result),
                ])
                .observe(start_time.elapsed().as_secs_f64());
        }
    }

    /// Observes the key counts of a node. For more information about the types of keys contained
    /// in the `key_counts` parameter, see the [`KeyCounts`] documentation.
    pub fn observe_node_key_counts(&self, key_counts: KeyCounts) {
        if let Some(metrics) = &self.metrics {
            metrics
                .crypto_key_counts
                .with_label_values(&[&format!("{}", KeyType::PublicLocal)])
                .set(key_counts.get_pk_local() as i64);
            metrics
                .crypto_key_counts
                .with_label_values(&[&format!("{}", KeyType::PublicRegistry)])
                .set(key_counts.get_pk_registry() as i64);
            metrics
                .crypto_key_counts
                .with_label_values(&[&format!("{}", KeyType::SecretSKS)])
                .set(key_counts.get_sk_local() as i64);
            metrics
                .crypto_key_counts
                .with_label_values(&[&format!("{}", KeyType::IdkgDealingEncryptionLocal)])
                .set(key_counts.get_idkg_pks_local() as i64);
        }
    }

    /// Observes results of iDKG dealing encryption key operations.
    pub fn observe_key_rotation_result(&self, result: KeyRotationResult) {
        if let Some(metrics) = &self.metrics {
            metrics
                .crypto_key_rotation_results
                .with_label_values(&[&format!("{}", result)])
                .inc();
        }
    }

    /// Observes the results of operations returning a boolean.
    pub fn observe_boolean_result(&self, operation: BooleanOperation, result: BooleanResult) {
        if let Some(metrics) = &self.metrics {
            metrics
                .crypto_boolean_results
                .with_label_values(&[&format!("{}", operation), &format!("{}", result)])
                .inc();
        }
    }

    /// Observes the parameter size of selected input parameters for crypto operations.
    ///
    /// # Parameters
    /// * `domain` the domain of the operation
    /// * `method_name` the name of the method for the operation
    /// * `parameter_name` the name of the parameter that is being observed
    /// * `parameter_size` the size of the parameter being observed, in bytes
    /// * `result` the result of the crypto operation
    pub fn observe_parameter_size(
        &self,
        domain: MetricsDomain,
        method_name: &str,
        parameter_name: &str,
        parameter_size: usize,
        result: MetricsResult,
    ) {
        if let Some(metrics) = &self.metrics {
            metrics
                .crypto_parameter_byte_sizes
                .with_label_values(&[
                    method_name,
                    parameter_name,
                    &format!("{}", domain),
                    &format!("{}", result),
                ])
                .observe(parameter_size as f64);
        }
    }

    pub fn observe_vault_message_size(
        &self,
        service_type: ServiceType,
        message_type: MessageType,
        domain: MetricsDomain,
        method_name: &str,
        size: usize,
    ) {
        if let Some(metrics) = &self.metrics {
            metrics
                .crypto_vault_message_sizes
                .with_label_values(&[
                    &format!("{}", service_type),
                    &format!("{}", message_type),
                    &format!("{}", domain),
                    method_name,
                ])
                .observe(size as f64);
        }
    }

    /// Observes the cache statistics for the verification of threshold BLS12-381 signatures.
    pub fn observe_bls12_381_sig_cache_stats(&self, size: usize, hits: u64, misses: u64) {
        if let Some(metrics) = &self.metrics {
            let m = &metrics.crypto_bls12_381_sig_cache_metrics;
            m.cache_size.set(size as i64);

            let prev_hits = m.cache_hits.get();
            debug_assert!(prev_hits <= hits);
            m.cache_hits.inc_by(hits - prev_hits);

            let prev_misses = m.cache_misses.get();
            debug_assert!(prev_misses <= misses);
            m.cache_misses.inc_by(misses - prev_misses);
        }
    }

    /// Observes the minimum registry version in active iDKG transcripts.
    pub fn observe_minimum_registry_version_in_active_idkg_transcripts(
        &self,
        registry_version: u64,
    ) {
        if let Some(metrics) = &self.metrics {
            metrics
                .observe_minimum_registry_version_in_active_idkg_transcripts
                .set(registry_version as f64);
        }
    }
}

#[derive(Copy, Clone, Debug, EnumIter, Eq, IntoStaticStr, PartialOrd, Ord, PartialEq)]
pub enum KeyType {
    PublicRegistry,
    PublicLocal,
    SecretSKS,
    IdkgDealingEncryptionLocal,
}

#[derive(Copy, Clone, Debug, EnumIter, Eq, IntoStaticStr, PartialOrd, Ord, PartialEq)]
pub enum MetricsDomain {
    BasicSignature,
    MultiSignature,
    ThresholdSignature,
    NiDkgAlgorithm,
    TlsHandshake,
    IdkgProtocol,
    ThresholdEcdsa,
    IcCanisterSignature,
    PublicSeed,
    KeyManagement,
}

#[derive(Copy, Clone, Debug, EnumIter, Eq, IntoStaticStr, PartialOrd, Ord, PartialEq)]
pub enum MetricsScope {
    Full,
    Local,
}

#[derive(Copy, Clone, Debug, EnumIter, Eq, IntoStaticStr, PartialOrd, Ord, PartialEq)]
pub enum MetricsResult {
    Ok,
    Err,
}

impl<T, E> From<&Result<T, E>> for MetricsResult {
    fn from(original: &Result<T, E>) -> Self {
        match original {
            Ok(_) => MetricsResult::Ok,
            Err(_) => MetricsResult::Err,
        }
    }
}

#[derive(Copy, Clone, Debug, EnumIter, Eq, IntoStaticStr, PartialOrd, Ord, PartialEq)]
pub enum KeyRotationResult {
    KeyRotated,
    LatestLocalRotationTooRecent,
    KeyGenerationError,
    RegistryError,
    KeyRotationNotEnabled,
    KeyNotRotated,
    RegistryKeyBadOrMissing,
    TransientInternalError,
}

#[derive(Copy, Clone, Debug, EnumIter, Eq, IntoStaticStr, PartialOrd, Ord, PartialEq)]
pub enum ServiceType {
    Client,
    Server,
}

#[derive(Copy, Clone, Debug, EnumIter, Eq, IntoStaticStr, PartialOrd, Ord, PartialEq)]
pub enum MessageType {
    Request,
    Response,
}

/// Keeps track of the number of node keys. This information is collected and provided to the
/// metrics component. The type of keys for which the key counts are tracked are the following:
///  - `pk_registry`: The number of node public keys (and TLS x.509 certificates) stored
///    in the registry
///  - `pk_local`: The number of node public keys (and TLS x.509 certificates) stored
///    in the local public key store
///  - `sk_local`: The number of node secret keys stored in the local secret key store
///  - `idkg_pks_local`: The number of iDKG dealing encryption public keys stored in the local
///    public key store
pub struct KeyCounts {
    pk_registry: u8,
    pk_local: u8,
    sk_local: u8,
    idkg_pks_local: u8,
}

impl KeyCounts {
    pub fn new(pk_registry: u8, pk_local: u8, sk_local: u8, idkg_pks_local: u8) -> Self {
        KeyCounts {
            pk_registry,
            pk_local,
            sk_local,
            idkg_pks_local,
        }
    }

    pub fn get_pk_registry(&self) -> u8 {
        self.pk_registry
    }

    pub fn get_pk_local(&self) -> u8 {
        self.pk_local
    }

    pub fn get_sk_local(&self) -> u8 {
        self.sk_local
    }

    pub fn get_idkg_pks_local(&self) -> u8 {
        self.idkg_pks_local
    }
}

/// A result for operations returning booleans. Using an enum allows adding errors, and using
/// macros for deriving the string representation needed for the dashboards.
#[derive(EnumIter, IntoStaticStr)]
pub enum BooleanResult {
    True,
    False,
}

impl Display for BooleanResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let value: &'static str = self.into();
        write!(f, "{}", value.to_case(Case::Snake))
    }
}

#[derive(EnumIter, IntoStaticStr)]
pub enum BooleanOperation {
    KeyInRegistryMissingLocally,
    LatestLocalIdkgKeyExistsInRegistry,
}

impl Display for BooleanOperation {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let value: &'static str = self.into();
        write!(f, "{}", value.to_case(Case::Snake))
    }
}

struct Metrics {
    /// Histogram of crypto lock acquisition times. The 'access' label is either
    /// 'read' or 'write'.
    pub crypto_lock_acquisition_duration_seconds: HistogramVec,

    /// Histograms of crypto method call times of various functionalities, measuring the full
    /// duration of the call, i.e. both the local crypto computation, and the
    /// potential RPC overhead.
    /// The 'method_name' label indicates the functionality, such as `sign`.
    /// The 'scope' label indicates the scope of the call, either `Full` or `Local`.
    /// The 'domain' label indicates the domain, e.g., `MetricsDomain::BasicSignature`.
    pub crypto_duration_seconds: HistogramVec,

    /// A gauge vector for the different types of keys and certificates of a node. The keys and
    /// certificates that are kept track of are:
    ///  - Node signing keys
    ///  - Committee signing keys
    ///  - NI-DKG keys
    ///  - iDKG keys
    ///  - TLS certificates and secret keys
    /// The above keys are not kept track of separately, merely a total number of stored keys.
    /// The counters keep track of which locations these keys are stored in:
    ///  - Registry
    ///  - Local public key store
    ///  - Local secret key store (SKS)
    /// Additionally, the number of iDKG dealing encryption public keys that are stored locally are
    /// also kept track of in the gauge vector.
    pub crypto_key_counts: IntGaugeVec,

    /// An counter vector for keeping track of key rotation results. Each time a key rotation is
    /// performed, the outcome of the operation is tracked in this counter vector.
    pub crypto_key_rotation_results: IntCounterVec,

    /// Counter vector for crypto results that can be expressed as booleans. An additional label
    /// is used to identify the type of operation.
    pub crypto_boolean_results: IntCounterVec,

    /// Histograms of crypto method parameter sizes.
    /// The 'method_name' label indicates the functionality, such as `sign`.
    /// The 'domain' label indicates the domain, e.g., `MetricsDomain::BasicSignature`.
    /// The 'parameter_name' indicates the name of the parameter, e.g., `message`.
    /// The 'parameter_size' indicates the size of the parameter in bytes.
    pub crypto_parameter_byte_sizes: HistogramVec,

    /// Histograms of messages' sizes sent between the CSP vault client and server via the RPC socket.
    /// The observed value is the size of the message in bytes.
    /// The 'method_name' label indicates the functionality, such as `sign` or `idkg_retain_active_keys`.
    /// The 'service_type' label indicates whether the observation is made by the `client` or `server`
    /// The 'message_type' label indicates whether the message is a request or a response.
    pub crypto_vault_message_sizes: HistogramVec,

    /// Metrics for the cache of successfully verified BLS12-381 threshold signatures.
    pub crypto_bls12_381_sig_cache_metrics: bls12_381_sig_cache::Metrics,

    /// Gauge for the minimum registry version in active iDKG transcripts.
    observe_minimum_registry_version_in_active_idkg_transcripts: Gauge,
}

impl Display for MetricsDomain {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let value: &'static str = self.into();
        write!(f, "{}", value.to_case(Case::Snake))
    }
}

impl Display for MetricsScope {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let value: &'static str = self.into();
        write!(f, "{}", value.to_case(Case::Snake))
    }
}

impl Display for MetricsResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let value: &'static str = self.into();
        write!(f, "{}", value.to_case(Case::Snake))
    }
}

impl Display for KeyRotationResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let value: &'static str = self.into();
        write!(f, "{}", value.to_case(Case::Snake))
    }
}

impl Display for KeyType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let value: &'static str = self.into();
        write!(f, "{}", value.to_case(Case::Snake))
    }
}

impl Display for ServiceType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let value: &'static str = self.into();
        write!(f, "{}", value.to_case(Case::Snake))
    }
}

impl Display for MessageType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let value: &'static str = self.into();
        write!(f, "{}", value.to_case(Case::Snake))
    }
}

impl Metrics {
    pub fn new(r: &MetricsRegistry) -> Self {
        let durations = r.histogram_vec(
            "crypto_duration_seconds",
            "Histogram of method call durations in seconds",
            ic_metrics::buckets::decimal_buckets(-4, 1),
            &["method_name", "scope", "domain", "result"],
        );
        let key_counts = r.int_gauge_vec(
            "crypto_key_counts",
            "Number of crypto keys stored locally and in the registry",
            &["key_type"],
        );
        for key_type in KeyType::iter() {
            key_counts.with_label_values(&[&format!("{}", key_type)]);
        }
        let boolean_results = r.int_counter_vec(
            "crypto_boolean_results",
            "Boolean results from crypto operations",
            &["operation", "result"],
        );
        for operation in BooleanOperation::iter() {
            for result in BooleanResult::iter() {
                boolean_results
                    .with_label_values(&[&format!("{}", operation), &format!("{}", result)]);
            }
        }
        let rotation_results = r.int_counter_vec(
            "crypto_key_rotation_results",
            "Result from iDKG dealing encryption key rotations",
            &["result"],
        );
        for result in KeyRotationResult::iter() {
            rotation_results.with_label_values(&[&format!("{}", result)]);
        }
        Self {
            crypto_lock_acquisition_duration_seconds: r.histogram_vec(
                "crypto_lock_acquisition_duration_seconds",
                "Histogram of crypto lock acquisition times",
                vec![0.00001, 0.0001, 0.001, 0.01, 0.1, 1.0, 10.0],
                &["name", "access"],
            ),
            crypto_duration_seconds: durations,
            crypto_key_counts: key_counts,
            crypto_key_rotation_results: rotation_results,
            crypto_boolean_results: boolean_results,
            crypto_parameter_byte_sizes: r.histogram_vec(
                "crypto_parameter_byte_sizes",
                "Byte sizes of crypto operation parameters",
                vec![
                    1000.0, 10000.0, 100000.0, 1000000.0, 2000000.0, 4000000.0, 8000000.0,
                    16000000.0, 20000000.0, 24000000.0, 28000000.0, 30000000.0,
                ],
                &["method_name", "parameter_name", "domain", "result"],
            ),
            crypto_vault_message_sizes: r.histogram_vec(
                "crypto_vault_message_sizes",
                "Byte sizes of crypto vault messages",
                vec![
                    1000.0, 10000.0, 100000.0, 1000000.0, 2000000.0, 4000000.0, 8000000.0,
                    16000000.0, 20000000.0, 24000000.0, 28000000.0, 30000000.0,
                ],
                &["service_type", "message_type", "domain", "method_name"],
            ),
            crypto_bls12_381_sig_cache_metrics: bls12_381_sig_cache::Metrics {
                cache_size: r.int_gauge(
                    "crypto_bls12_381_sig_cache_size",
                    "Size of cache for successfully verified BLS12-381 threshold signatures",
                ),
                cache_hits: r.int_counter(
                    "crypto_bls12_381_sig_cache_hits",
                "Number of cache hits for successfully verified BLS12-381 threshold signatures"),
                cache_misses: r.int_counter(
                    "crypto_bls12_381_sig_cache_misses",
                "Number of cache misses for successfully verified BLS12-381 threshold signatures"), 
            },
            observe_minimum_registry_version_in_active_idkg_transcripts: r.gauge(
                "crypto_minimum_registry_version_in_active_idkg_transcripts",
                "Minimum registry version in active iDKG transcripts"
            )
        }
    }
}
