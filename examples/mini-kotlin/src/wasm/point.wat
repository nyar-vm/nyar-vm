(module
(rec
;; trait Any { }
;;	(type $AnyObject (sub (struct
;;		(field $vtable (ref null struct))
;;		(field $itable (ref null struct))
;;	)))
;;	(type $AnyObjectVTable (sub (struct
;;
;;	)))
;;	(type $AnyObjectITable (sub (struct
;;
;;	)))
;; trait Foo {
;;     get foo(self): i32 = 1
;; }
	(type $Foo (sub (struct
		(field $vtable (ref null struct))
		(field $itable (ref $FooITable))
	)))
	(type $FooITable (sub (struct
		(field $foo (ref $FooVTable))
	)))
	(type $FooVTable (sub (struct
		(field (ref $get_foo))
	)))
	(type $get_foo (sub (func
		(param $self (ref any))
		(result i32)
	)))
;; trait Bar {
;;     get bar(self): i32 = 2
;; }
	(type $Bar (sub (struct
		(field $vtable (ref null struct))
		(field $itable (ref $BarITable))
	)))
	(type $BarITable (sub (struct
		(field $foo (ref $BarVTable))
	)))
	(type $BarVTable (sub (struct
		(field (ref null $get_foo))
	)))
	(type $get_bar (sub (func (param i32 (ref $Bar)))))
;; class Base: Foo, Bar {
;;
;; }
	(type $Base (sub (struct
		(field $vtable (ref null struct))
		(field $itable (ref $BarITable))
	    (field $fields (ref $BaseFields))
	)))
	(type $BaseVTable (sub (struct

	)))
	(type $BaseITable (sub (struct
		(field $foo (ref $FooVTable))
		(field $bar (ref $BarVTable))
	)))
	(type $BaseFields (sub (struct

	)))
;; class Derived: Base {
;;    override get foo(self): i32 = self.foo
;;}
	(type $Derived (sub (struct
	    (field $itable (ref $DerivedVTable))
	    (field $vtable (ref $DerivedITable))
	    (field $fields (ref $DerivedFields))
	)))
	(type $DerivedVTable (sub (struct

	)))
	(type $DerivedITable (sub (struct
	    (field $foo (ref $FooVTable))
	    (field $bar (ref $BarVTable))
	)))
	(type $DerivedFields (sub (struct
	    (field $foo (mut i32))
	    (field $bar (mut i32))
	)))
)

;; let derived: Derived? = unknown? as? Derived
;; if derived != null {
;;
;; }
  (func (export "ref.cast") (param $self (ref null any)) (result (ref null $Derived))
    (ref.cast (ref $Derived) (local.get $self))
  )

  (func (export "unknown as Derived") (param $self (ref null any)) (result (ref null $Derived))
    (block $cast-fail (result (ref null any))
	    (br_on_cast_fail $cast-fail
	        (ref null any)
	        (ref $Derived)
		    (local.get $self)
	    )
	    return
    )
    ;; cast fail
    drop
    (return (ref.null $Derived))
  )

  (func $isFoo (param $self (ref null any)) (result i32)
    (block $cast-fail (result (ref null any))
	    (br_on_cast_fail $cast-fail
	        (ref null any)
	        (ref $Foo)
		    (local.get $self)
	    )
	    ;; Foo
	    struct.get $Foo $itable
		;; FooITable
	    struct.get $FooITable $foo
	    ref.test (ref $FooVTable)
	    return
    )
    (return (i32.const 0))
  )

    (func $unreachable
	    (;@18;) unreachable
    )
	(start $unreachable)

;;	(func $Derived.get_foo (param $self (ref any)) (result i32)
;;		local.get $self
;;		struct.get $Derived $fields
;;		call $DerivedFields.get_foo
;;	)
;;
;;	(func $DerivedFields.get_foo (param $self (ref $DerivedFields)) (result i32)
;;		local.get $self
;;		struct.get $DerivedFields $foo
;;	)
;;	(func $DerivedITable.new (result (ref $FooVTable))
;;	    ref.func $Derived.get_foo
;;	    struct.new $FooVTable
;;;;	    ref.func $Derived.get_foo
;;;;	    struct.new $FooVTable
;;;;	    struct.new $DerivedITable
;;	)
  (@custom "sourceMappingURL" (after data) "+kotlin-wasm-nodejs-example-wasm-js.wasm.map")
)
