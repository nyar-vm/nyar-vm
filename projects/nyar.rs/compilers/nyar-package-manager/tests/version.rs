use nyar_package_manager::*;

#[test]
fn caret_and_tilde_constraints() {
    assert!(satisfies_version_constraint("1.2.3", "^1.0.0"));
    assert!(!satisfies_version_constraint("2.0.0", "^1.0.0"));
    assert!(satisfies_version_constraint("1.2.9", "~1.2.0"));
    assert!(!satisfies_version_constraint("1.3.0", "~1.2.0"));
}

#[test]
fn wildcard_and_minor_line_constraints() {
    assert!(satisfies_version_constraint("0.1.0.0", "0.1.*"));
    assert!(satisfies_version_constraint("0.1.3", "0.1"));
    assert!(!satisfies_version_constraint("0.2.0", "0.1.*"));
    assert!(satisfies_version_constraint("2025.4.1", "2025.*"));
}

#[test]
fn yearly_version_constraints() {
    assert!(satisfies_version_constraint("2020.0.0.0", "2020.*"));
    assert!(satisfies_version_constraint("2020.0.1.0", "2020.0.*"));
    assert!(satisfies_version_constraint("2020.0.0.2", "2020.0.0.*"));
    assert!(satisfies_version_constraint("2020.0.0.2", "2020.0"));
    assert!(!satisfies_version_constraint("2020.0.0.1", "2020.1.*"));
    assert!(!satisfies_version_constraint("2021.0.0.0", "2020.*"));
    assert_eq!(YearlyVersion::parse("2020.0.0.0").unwrap(), YearlyVersion { yearly: 2020, major: 0, minor: 0, patch: 0 });
}

#[test]
fn comparator_constraints_are_rejected() {
    assert!(!satisfies_version_constraint("0.1.0", ">=0.1.0"));
    assert!(!satisfies_version_constraint("0.1.0", "<2.0.0"));
}
