use nyar_types::*;
#[test]
fn unknown_features_fail_closed() {
    let manifest = CoreSurfaceManifest::new("1", [CoreFeature::Functions]).unwrap();
    assert!(manifest.require(CoreFeature::Functions).is_ok());
    assert!(manifest.require(CoreFeature::Float64).is_err());
}
