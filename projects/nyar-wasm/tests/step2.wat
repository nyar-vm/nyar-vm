(module $control
    ;; [] -> []
    (type $func (func))
    ;; cont ([] -> [])
    (type $cont (cont $func)) 
    ;; [contref ([] -> [])] -> []
    (type $cont-func (func (param (ref $cont))))
    ;; cont ([contref ([] -> [])] -> [])
    (type $cont-cont (cont $cont-func))
    ;; control : [([contref ([ta*] -> [tr*])] -> [tr*])] -> [ta*]
    (tag $control (export "control") (param (ref $cont-func)))
    ;; prompt : [contref ([] -> [tr*])] -> [tr*]
    (func $prompt (export "prompt") (param $nextk (ref null $cont))
        (block $on_control (result (ref $cont-func) (ref $cont))
            (resume (tag $control $on_control)
                    (local.get $nextk))
            (return)
        )
        ;;   $on_control (param (ref $cont-func) (ref $cont))
        (let (local $h (ref $cont-func)) (local $k (ref $cont))
            (call_ref (local.get $k) (local.get $h))
        )
    )
)
(register "control")
