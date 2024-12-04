use nyar_package_registry::*;

#[test]
fn seed_and_fingerprint_are_bidirectional() {
    let seed = [7u8; 32];
    let key = PublisherKey::from_seed(seed).expect("seed");
    let fingerprint = key.fingerprint().to_string();
    assert!(fingerprint.starts_with("ed25519:"));
    assert_eq!(fingerprint.len(), "ed25519:".len() + 64);

    let auth = key.to_auth_material();
    assert!(auth.starts_with("ed25519-seed:"));
    let reloaded = PublisherKey::parse(&auth).expect("reload seed");
    assert_eq!(reloaded.fingerprint(), key.fingerprint());
    assert!(reloaded.can_sign());

    let public_only = PublisherKey::parse(&fingerprint).expect("reload fingerprint");
    assert_eq!(public_only.fingerprint(), key.fingerprint());
    assert!(!public_only.can_sign());
}

#[test]
fn bare_hex_seed_normalizes_to_canonical_seed_form() {
    let seed_hex = "11".repeat(32);
    let key = PublisherKey::parse(&seed_hex).expect("bare hex");
    assert_eq!(key.to_canonical(), format!("ed25519-seed:{seed_hex}"));
    assert!(key.authorization_header(b"body").is_ok());
}

#[test]
fn package_name_canonicalization_is_bidirectional_enough() {
    assert_eq!(canonicalize_package_name("Org_Pkg-Name").unwrap(), "org.pkg.name");
    assert_eq!(canonicalize_package_name("org.pkg.name").unwrap(), "org.pkg.name");
}
