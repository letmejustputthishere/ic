#![allow(clippy::unwrap_used)]
use ic_crypto::utils::TempCryptoComponent;
use ic_crypto::{
    threshold_sig_public_key_from_der, user_public_key_from_bytes, KeyBytesContentType,
};
use ic_crypto_internal_basic_sig_der_utils::subject_public_key_info_der;
use ic_crypto_internal_test_vectors::iccsa;
use ic_crypto_internal_types::sign::threshold_sig::public_key::bls12_381;
use ic_crypto_test_utils::canister_signatures::canister_sig_pub_key_to_bytes;
use ic_interfaces::crypto::CanisterSigVerifier;
use ic_protobuf::registry::crypto::v1::PublicKey as PublicKeyProto;
use ic_protobuf::types::v1::PrincipalId as PrincipalIdIdProto;
use ic_protobuf::types::v1::SubnetId as SubnetIdProto;
use ic_registry_client_fake::FakeRegistryClient;
use ic_registry_keys::{make_crypto_threshold_signing_pubkey_key, ROOT_SUBNET_ID_KEY};
use ic_registry_proto_data_provider::ProtoRegistryDataProvider;
use ic_types::crypto::threshold_sig::ThresholdSigPublicKey;
use ic_types::crypto::{AlgorithmId, CanisterSig, CanisterSigOf, CryptoError, UserPublicKey};
use ic_types::messages::Delegation;
use ic_types::time::{current_time, Time};
use ic_types::{CanisterId, RegistryVersion, SubnetId};
use ic_types_test_utils::ids::{node_test_id, SUBNET_1};
use simple_asn1::oid;
use std::str::FromStr;
use std::sync::Arc;

pub const REG_V1: RegistryVersion = RegistryVersion::new(5);
pub const ROOT_SUBNET_ID: SubnetId = SUBNET_1;

#[test]
fn should_correctly_parse_der_encoded_iccsa_pubkey() {
    let pubkey_bytes = canister_sig_pub_key_to_bytes(CanisterId::from_u64(42), b"seed");
    let pubkey_der =
        subject_public_key_info_der(oid!(1, 3, 6, 1, 4, 1, 56387, 1, 2), &pubkey_bytes).unwrap();

    let (parsed_pubkey, content_type) = user_public_key_from_bytes(&pubkey_der).unwrap();

    assert_eq!(parsed_pubkey.algorithm_id, AlgorithmId::IcCanisterSignature);
    assert_eq!(parsed_pubkey.key, pubkey_bytes);
    assert_eq!(
        content_type,
        KeyBytesContentType::IcCanisterSignatureAlgPublicKeyDer
    );
}

#[test]
fn should_verify_valid_canister_signature() {
    let (message, signature, user_pubkey, root_pubkey) = test_vec(iccsa::TestVectorId::STABILITY_1);
    let temp_crypto = temp_crypto_with_registry_with_root_pubkey(root_pubkey, REG_V1);

    let result = temp_crypto.verify_canister_sig(&signature, &message, &user_pubkey, REG_V1);

    assert!(result.is_ok());
}

#[test]
fn should_fail_to_verify_on_wrong_signature() {
    let (message, signature, user_pubkey, root_pubkey) = test_vec(iccsa::TestVectorId::STABILITY_1);
    let corrupted_sig_bytes = {
        let mut corrupted_sig = signature.clone().get().0;
        let len = corrupted_sig.len();
        corrupted_sig[len - 5] ^= 0xFF;
        corrupted_sig
    };
    let wrong_signature = CanisterSigOf::new(CanisterSig(corrupted_sig_bytes));
    assert_ne!(signature, wrong_signature);
    let temp_crypto = temp_crypto_with_registry_with_root_pubkey(root_pubkey, REG_V1);

    let result = temp_crypto.verify_canister_sig(&wrong_signature, &message, &user_pubkey, REG_V1);

    assert!(
        matches!(result, Err(CryptoError::SignatureVerification {  algorithm, public_key_bytes: _, sig_bytes: _, internal_error})
            if internal_error.contains("certificate verification failed")
            && algorithm == AlgorithmId::IcCanisterSignature
        )
    );
}

#[test]
fn should_fail_to_verify_on_wrong_message() {
    let (message, signature, user_pubkey, root_pubkey) = test_vec(iccsa::TestVectorId::STABILITY_1);
    let wrong_message = Delegation::new(b"delegated pubkey".to_vec(), current_time());
    assert_ne!(message, wrong_message);
    let temp_crypto = temp_crypto_with_registry_with_root_pubkey(root_pubkey, REG_V1);

    let result = temp_crypto.verify_canister_sig(&signature, &wrong_message, &user_pubkey, REG_V1);

    assert!(
        matches!(result, Err(CryptoError::SignatureVerification {  algorithm, public_key_bytes: _, sig_bytes: _, internal_error})
            if internal_error.contains("the signature tree doesn't contain sig")
            && algorithm == AlgorithmId::IcCanisterSignature
        )
    );
}

#[test]
fn should_fail_to_verify_on_wrong_public_key() {
    let (message, signature, user_pubkey, root_pubkey) = test_vec(iccsa::TestVectorId::STABILITY_1);
    let wrong_pubkey = {
        let mut wrong_pubkey = user_pubkey;
        wrong_pubkey.key.push(42);
        wrong_pubkey
    };
    let temp_crypto = temp_crypto_with_registry_with_root_pubkey(root_pubkey, REG_V1);

    let result = temp_crypto.verify_canister_sig(&signature, &message, &wrong_pubkey, REG_V1);

    assert!(
        matches!(result, Err(CryptoError::SignatureVerification {  algorithm, public_key_bytes: _, sig_bytes: _, internal_error})
            if internal_error.contains("the signature tree doesn't contain sig")
            && algorithm == AlgorithmId::IcCanisterSignature
        )
    );
}

#[test]
fn should_fail_to_verify_on_invalid_root_public_key() {
    let (message, signature, user_pubkey, root_pubkey) = test_vec(iccsa::TestVectorId::STABILITY_1);
    let invalid_root_pk = {
        ThresholdSigPublicKey::from(bls12_381::PublicKeyBytes(
            [42; bls12_381::PublicKeyBytes::SIZE],
        ))
    };
    assert_ne!(root_pubkey, invalid_root_pk);
    let temp_crypto = temp_crypto_with_registry_with_root_pubkey(invalid_root_pk, REG_V1);

    let result = temp_crypto.verify_canister_sig(&signature, &message, &user_pubkey, REG_V1);

    assert!(
        matches!(result, Err(CryptoError::SignatureVerification {  algorithm, public_key_bytes: _, sig_bytes: _, internal_error})
            if internal_error.contains("Invalid public key")
            && internal_error.contains("certificate verification failed")
            && algorithm == AlgorithmId::IcCanisterSignature
        )
    );
}

#[test]
fn should_fail_to_verify_on_wrong_root_public_key() {
    let (message, signature, user_pubkey, root_pubkey) = test_vec(iccsa::TestVectorId::STABILITY_1);
    // This is a valid public key different from root_pk. It was extracted using
    // `From<&NiDkgTranscript> for ThresholdSigPublicKey` from an `NiDkgTranscript`
    // in an integration test.
    let wrong_root_pubkey = {
        let wrong_root_pk_vec = hex::decode("91cf31d8a6ac701281d2e38d285a4141858f355e05102cedd280f98dfb277613a8b96ac32a5f463ebea2ae493f4eba8006e30b0f2f5c426323fb825a191fb7f639f61d33a0c07addcdd2791d2ac32ec8be354e8465b6a18da6b5685deb0e9245").unwrap();
        let mut wrong_root_pk_bytes = [0; 96];
        wrong_root_pk_bytes.copy_from_slice(&wrong_root_pk_vec);
        ThresholdSigPublicKey::from(bls12_381::PublicKeyBytes(wrong_root_pk_bytes))
    };
    assert_ne!(root_pubkey, wrong_root_pubkey);
    let temp_crypto = temp_crypto_with_registry_with_root_pubkey(wrong_root_pubkey, REG_V1);

    let result = temp_crypto.verify_canister_sig(&signature, &message, &user_pubkey, REG_V1);

    assert!(
        matches!(result, Err(CryptoError::SignatureVerification {  algorithm, public_key_bytes: _, sig_bytes: _, internal_error})
            if internal_error.contains("Invalid combined threshold signature")
            && internal_error.contains("certificate verification failed")
            && algorithm == AlgorithmId::IcCanisterSignature
        )
    );
}

#[test]
fn should_fail_to_verify_if_root_public_key_not_found_in_registry() {
    let (message, signature, user_pubkey, root_pubkey) = test_vec(iccsa::TestVectorId::STABILITY_1);
    let temp_crypto = temp_crypto_with_registry_with_root_pubkey(root_pubkey, REG_V1);

    let registry_version_where_root_pubkey_is_not_available_yet = REG_V1 - RegistryVersion::new(1);
    let result = temp_crypto.verify_canister_sig(
        &signature,
        &message,
        &user_pubkey,
        registry_version_where_root_pubkey_is_not_available_yet,
    );

    assert!(matches!(
        result,
        Err(CryptoError::RootSubnetPublicKeyNotFound { .. })
    ));
}

fn test_vec(
    testvec_id: iccsa::TestVectorId,
) -> (
    Delegation,
    CanisterSigOf<Delegation>,
    UserPublicKey,
    ThresholdSigPublicKey,
) {
    let test_vec = iccsa::test_vec(testvec_id);
    let message = Delegation::new(
        test_vec.delegation_pubkey,
        Time::from_nanos_since_unix_epoch(test_vec.delegation_exp),
    );
    let signature = CanisterSigOf::new(CanisterSig(test_vec.signature));
    let user_public_key = {
        let canister_id = CanisterId::from_str(&test_vec.canister_id).unwrap();
        let public_key_bytes = canister_sig_pub_key_to_bytes(canister_id, &test_vec.seed);

        UserPublicKey {
            key: public_key_bytes,
            algorithm_id: AlgorithmId::IcCanisterSignature,
        }
    };
    let root_pubkey = threshold_sig_public_key_from_der(&test_vec.root_pubkey_der).unwrap();
    (message, signature, user_public_key, root_pubkey)
}

fn temp_crypto_with_registry_with_root_pubkey(
    threshold_sig_pubkey: ThresholdSigPublicKey,
    registry_version: RegistryVersion,
) -> TempCryptoComponent {
    let registry_data = Arc::new(ProtoRegistryDataProvider::new());
    let registry = FakeRegistryClient::new(Arc::clone(&registry_data) as Arc<_>);
    let root_subnet_id = SubnetIdProto {
        principal_id: Some(PrincipalIdIdProto {
            raw: ROOT_SUBNET_ID.get_ref().to_vec(),
        }),
    };
    registry_data
        .add(ROOT_SUBNET_ID_KEY, registry_version, Some(root_subnet_id))
        .expect("failed to add root subnet ID to registry");

    let root_subnet_pubkey = PublicKeyProto::from(threshold_sig_pubkey);
    registry_data
        .add(
            &make_crypto_threshold_signing_pubkey_key(ROOT_SUBNET_ID),
            registry_version,
            Some(root_subnet_pubkey),
        )
        .expect("failed to add root subnet ID to registry");
    registry.update_to_latest_version();

    TempCryptoComponent::new(Arc::new(registry), node_test_id(42))
}
