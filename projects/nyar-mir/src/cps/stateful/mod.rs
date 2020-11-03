use std::collections::HashMap;

trait Continuation<A, B, S> {
    fn resume(value: A, state: S) -> B;
}

trait CPS<A,B,S> {
    fn apply(k: impl Continuation<A,B,S>,state:S) -> B;
}

trait Stateful<S> {
    fn export_state(&self) -> S;
    fn import_state(&mut self,state:S);
}

struct State<K,V> {
    data: HashMap<K,V>
}

struct Field<T> {

}

impl<K,V> Stateful<HashMap<K,V>> for State<K,V> {
    fn export_state(&self) -> HashMap<K, V> {
        self.data.clone()
    }

    fn import_state(&mut self, state: HashMap<K, V>) {
        self.data = state
    }
}
