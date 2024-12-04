use nyar_package_manager::*;

#[test]
fn mit_is_compatible_and_not_restricted() {
    assert!(SecurityAudit::is_license_compatible("MIT"));
    assert!(!SecurityAudit::is_license_restricted("MIT"));
}

#[test]
fn offline_audit_skips_osv_but_checks_license() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut audit = SecurityAudit::with_cache_path(dir.path().join("cache.json")).expect("open").offline();
    let package = Package { name: "demo".into(), version: "1.0.0".into(), license: "MIT".into(), ..Package::default() };
    let result = audit.audit_package(&package, "npm").expect("audit");
    assert!(result.vulnerabilities.is_empty());
    assert_eq!(result.licenses.len(), 1);
    assert!(result.licenses[0].is_compatible);
}

#[test]
fn gpl_is_restricted() {
    assert!(SecurityAudit::is_license_restricted("GPL-3.0-only"));
    let package = Package { name: "demo".into(), version: "1.0.0".into(), license: "GPL-3.0-only".into(), ..Package::default() };
    let info = check_license(&package);
    assert!(!info.is_compatible);
    assert!(info.is_restricted);
}
