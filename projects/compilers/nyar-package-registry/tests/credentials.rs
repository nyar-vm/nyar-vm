use std::sync::{Mutex, OnceLock};

use nyar_package_registry::*;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn parses_auth_token_line() {
    let content = "//registry.npmjs.org/:_authToken=npm_test_token\n";
    assert_eq!(parse_npmrc_token(content, "//registry.npmjs.org/").as_deref(), Some("npm_test_token"));
}

#[test]
fn expands_env_placeholder() {
    // SAFETY: unit-test only.
    unsafe {
        std::env::set_var("REGISTRY_TEST_NPM_TOKEN", "from-env");
    }
    assert_eq!(expand_npmrc_value("${REGISTRY_TEST_NPM_TOKEN}"), "from-env");
    unsafe {
        std::env::remove_var("REGISTRY_TEST_NPM_TOKEN");
    }
}

#[test]
fn registry_prefix_from_endpoint() {
    assert_eq!(registry_auth_prefix("https://registry.npmjs.org"), Some("//registry.npmjs.org/".to_string()));
    assert_eq!(registry_auth_prefix("https://npm.pkg.github.com/"), Some("//npm.pkg.github.com/".to_string()));
}

#[test]
fn discovers_jsr_token_from_env() {
    let _guard = env_lock().lock().expect("lock");
    let previous = std::env::var_os("JSR_TOKEN");
    let previous_deno = std::env::var_os("DENO_AUTH_TOKENS");
    unsafe {
        std::env::set_var("JSR_TOKEN", "jsr-from-env");
        std::env::remove_var("DENO_AUTH_TOKENS");
    }
    let credential = discover_jsr_token().expect("token");
    assert_eq!(credential.token, "jsr-from-env");
    unsafe {
        match previous {
            Some(value) => std::env::set_var("JSR_TOKEN", value),
            None => std::env::remove_var("JSR_TOKEN"),
        }
        match previous_deno {
            Some(value) => std::env::set_var("DENO_AUTH_TOKENS", value),
            None => std::env::remove_var("DENO_AUTH_TOKENS"),
        }
    }
}

#[test]
fn discovers_jsr_token_from_deno_auth_tokens() {
    let _guard = env_lock().lock().expect("lock");
    let previous = std::env::var_os("JSR_TOKEN");
    let previous_deno = std::env::var_os("DENO_AUTH_TOKENS");
    unsafe {
        std::env::remove_var("JSR_TOKEN");
        std::env::set_var("DENO_AUTH_TOKENS", "npm@token1;jsr@jsr-from-deno;other@token3");
    }
    let credential = discover_jsr_token().expect("token");
    assert_eq!(credential.token, "jsr-from-deno");
    assert!(credential.source.contains("DENO_AUTH_TOKENS"));
    unsafe {
        match previous {
            Some(value) => std::env::set_var("JSR_TOKEN", value),
            None => std::env::remove_var("JSR_TOKEN"),
        }
        match previous_deno {
            Some(value) => std::env::set_var("DENO_AUTH_TOKENS", value),
            None => std::env::remove_var("DENO_AUTH_TOKENS"),
        }
    }
}

#[test]
fn extracts_deno_config_token_value() {
    let content = r#"{ "net.jsr": "token-from-deno-config" }"#;
    assert_eq!(extract_json_string_value(content, r#""net.jsr""#).as_deref(), Some("token-from-deno-config"));
    let alt = r#"{ "https://jsr.io": "alt-token" }"#;
    assert_eq!(extract_json_string_value(alt, r#""https://jsr.io""#).as_deref(), Some("alt-token"));
}

#[test]
fn sync_npm_token_writes_user_npmrc() {
    let _guard = env_lock().lock().expect("lock");
    let home = tempfile::tempdir().expect("temp home");
    let previous_home = std::env::var_os("HOME");
    let previous_profile = std::env::var_os("USERPROFILE");
    unsafe {
        std::env::set_var("HOME", home.path());
        std::env::set_var("USERPROFILE", home.path());
    }

    let path = sync_token_to_official_store("npm", Some("https://registry.npmjs.org"), "npm-sync-token").expect("sync").expect("path");
    let content = std::fs::read_to_string(&path).expect("read npmrc");
    assert!(content.contains("npm-sync-token"));

    unsafe {
        match previous_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
        match previous_profile {
            Some(value) => std::env::set_var("USERPROFILE", value),
            None => std::env::remove_var("USERPROFILE"),
        }
    }
}

#[test]
fn discovers_nuget_token_from_config() {
    let _guard = env_lock().lock().expect("lock");
    let home = tempfile::tempdir().expect("temp home");
    let nuget_dir = home.path().join("NuGet");
    std::fs::create_dir_all(&nuget_dir).expect("mkdir");
    std::fs::write(
        nuget_dir.join("NuGet.Config"),
        r#"<?xml version="1.0" encoding="utf-8"?>
<configuration>
  <apikeys>
    <add key="https://api.nuget.org/v3/index.json" value="nuget-from-config" />
  </apikeys>
</configuration>
"#,
    )
    .expect("write config");
    let previous = std::env::var_os("APPDATA");
    unsafe {
        std::env::set_var("APPDATA", home.path());
    }
    let credential = discover_nuget_token().expect("token");
    assert_eq!(credential.token, "nuget-from-config");
    unsafe {
        match previous {
            Some(value) => std::env::set_var("APPDATA", value),
            None => std::env::remove_var("APPDATA"),
        }
    }
}

#[test]
fn discovers_maven_token_from_settings_xml() {
    let _guard = env_lock().lock().expect("lock");
    let home = tempfile::tempdir().expect("temp home");
    let m2 = home.path().join(".m2");
    std::fs::create_dir_all(&m2).expect("mkdir");
    std::fs::write(
        m2.join("settings.xml"),
        r#"<settings>
  <servers>
    <server>
      <id>ossrh</id>
      <username>user</username>
      <password>pass</password>
    </server>
  </servers>
</settings>
"#,
    )
    .expect("write settings");
    let previous_home = std::env::var_os("HOME");
    let previous_profile = std::env::var_os("USERPROFILE");
    unsafe {
        std::env::set_var("HOME", home.path());
        std::env::set_var("USERPROFILE", home.path());
    }
    let credential = discover_maven_token().expect("token");
    use base64::{Engine, engine::general_purpose::STANDARD};
    assert_eq!(credential.token, STANDARD.encode("user:pass"));
    unsafe {
        match previous_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
        match previous_profile {
            Some(value) => std::env::set_var("USERPROFILE", value),
            None => std::env::remove_var("USERPROFILE"),
        }
    }
}

#[test]
fn discovers_conda_token_from_nucleus() {
    let _guard = env_lock().lock().expect("lock");
    let home = tempfile::tempdir().expect("temp home");
    let token_path = home.path().join(".anaconda").join("nucleus").join("user-1").join("tokens.json");
    std::fs::create_dir_all(token_path.parent().unwrap()).expect("mkdir");
    std::fs::write(token_path, r#"{"token":"conda-from-nucleus","login":"alice"}"#).expect("write token");
    let previous_home = std::env::var_os("HOME");
    let previous_profile = std::env::var_os("USERPROFILE");
    unsafe {
        std::env::set_var("HOME", home.path());
        std::env::set_var("USERPROFILE", home.path());
    }
    let credential = discover_conda_token().expect("token");
    assert_eq!(credential.token, "conda-from-nucleus");
    unsafe {
        match previous_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
        match previous_profile {
            Some(value) => std::env::set_var("USERPROFILE", value),
            None => std::env::remove_var("USERPROFILE"),
        }
    }
}

#[test]
fn sync_nuget_token_writes_config() {
    let _guard = env_lock().lock().expect("lock");
    let home = tempfile::tempdir().expect("temp home");
    let previous = std::env::var_os("APPDATA");
    unsafe {
        std::env::set_var("APPDATA", home.path());
    }
    let path = sync_token_to_official_store("nuget", Some("https://api.nuget.org/v3"), "nuget-sync-token").expect("sync").expect("path");
    let content = std::fs::read_to_string(path).expect("read config");
    assert!(content.contains("nuget-sync-token"));
    unsafe {
        match previous {
            Some(value) => std::env::set_var("APPDATA", value),
            None => std::env::remove_var("APPDATA"),
        }
    }
}

#[test]
fn uses_external_credential_store_classifies_third_party_registries() {
    assert!(uses_external_credential_store("npm"));
    assert!(uses_external_credential_store("nuget"));
    assert!(!uses_external_credential_store("valhalla"));
}
