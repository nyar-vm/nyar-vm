use nyar_package_registry::*;

#[test]
fn proxy_hint_reads_https_proxy() {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = LOCK.lock().expect("lock");
    let previous = std::env::var_os("HTTPS_PROXY");
    unsafe {
        std::env::set_var("HTTPS_PROXY", "http://127.0.0.1:7890");
    }
    let hint = proxy_env_hint().expect("hint");
    assert!(hint.contains("7890"));
    unsafe {
        match previous {
            Some(value) => std::env::set_var("HTTPS_PROXY", value),
            None => std::env::remove_var("HTTPS_PROXY"),
        }
    }
}
