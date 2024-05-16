use nyar_language::transitional::guest_scripts::powershell::PowerShellScript;

#[test]
fn parse_fixture_line() {
    let script = PowerShellScript::parse(r#"Write-Output "legend powershell fixture""#).expect("parse");
    assert_eq!(script.statements.len(), 1);
}
