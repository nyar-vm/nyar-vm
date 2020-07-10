
/// Continuation.
/// Cont ~ (Cont -> !)    We use `()` instead of `!` here since `!` not stable


impl Continuation<'_, V> {
    fn resume(&self, value: &Continuation<V>) -> V {
        value
    }

    fn call(&self, value: &Continuation<V>) {
        (self.0)(value); // Simple proxy. Note that it is dynamic dispatch.
    }
}

/// Equal to `{ let cc_ = getcc(); cc(cc_); }`
/// Apparently, `cc_` and `cc` is the same continuation.
fn with_cc<V>(cc: impl Fn(&Continuation<V>)) {
    cc(&Continuation(&cc)); // Call `cc` with `cc` itself (current continuation)
}

#[test]
fn puzzle() {
    with_cc(|yin| {
        print!("@");
        with_cc(|yang| {
            print!("*");
            yin.call(yang);
        });
    });
}