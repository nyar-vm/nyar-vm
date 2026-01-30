use chomsky::uir::IKun;

pub struct BarrierElision;

impl<A: chomsky_uir::egraph::Analysis<IKun>> chomsky_rule_engine::RewriteRule<A> for BarrierElision {
    fn name(&self) -> &str {
        "barrier-elision"
    }

    fn apply(&self, egraph: &chomsky_uir::egraph::EGraph<IKun, A>) {
        let mut matches = Vec::new();
        for entry in egraph.classes.iter() {
            let (&id, eclass) = entry.pair();
            for node in &eclass.nodes {
                if let IKun::Extension(op, args) = node {
                    if op == "barrier" && args.len() == 1 {
                        let obj_id = egraph.union_find.find(args[0]);
                        if let Some(obj_class) = egraph.classes.get(&obj_id) {
                            for obj_node in &obj_class.nodes {
                                if let IKun::Extension(obj_op, _) = obj_node {
                                    if obj_op == "alloc" {
                                        matches.push(id);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        for id in matches {
            let nop_id = egraph.add(IKun::Symbol("nop".to_string()));
            egraph.union(id, nop_id);
        }
    }
}

pub struct AllocationSinking;

impl<A: chomsky_uir::egraph::Analysis<IKun>> chomsky_rule_engine::RewriteRule<A> for AllocationSinking {
    fn name(&self) -> &str {
        "allocation-sinking"
    }

    fn apply(&self, egraph: &chomsky_uir::egraph::EGraph<IKun, A>) {
        let mut matches = Vec::new();
        for entry in egraph.classes.iter() {
            let (&id, eclass) = entry.pair();
            for node in &eclass.nodes {
                if let IKun::Extension(op, _args) = node {
                    if op == "alloc" {
                        let mut escapes = false;
                        for other_entry in egraph.classes.iter() {
                            let other_class = other_entry.value();
                            for other_node in &other_class.nodes {
                                match other_node {
                                    IKun::Extension(other_op, other_args) => {
                                        if other_args.contains(&id)
                                            && other_op != "load_field"
                                            && other_op != "store_field"
                                            && other_op != "barrier"
                                            && other_op != "type_of"
                                        {
                                            escapes = true;
                                            break;
                                        }
                                    }
                                    IKun::Apply(_, other_args) => {
                                        if other_args.contains(&id) {
                                            escapes = true;
                                            break;
                                        }
                                    }
                                    IKun::Map(f, x) => {
                                        if *f == id || *x == id {
                                            escapes = true;
                                            break;
                                        }
                                    }
                                    IKun::StateUpdate(_k, v) => {
                                        if *v == id {
                                            escapes = true;
                                            break;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            if escapes {
                                break;
                            }
                        }
                        if !escapes {
                            matches.push(id);
                        }
                    }
                }
            }
        }

        for id in matches {
            let virtual_id = egraph.add(IKun::Extension("virtual_object".to_string(), vec![]));
            egraph.union(id, virtual_id);
        }
    }
}
