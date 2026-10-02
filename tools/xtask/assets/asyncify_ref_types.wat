;; Asyncify must spill reference-typed locals (externref/funcref) that stay live
;; across a suspension point.

(module
  (import "env" "unwind" (func $unwind))
  (memory 1)

  ;; externref parameter, live across the suspension.
  (func (export "extern_param") (param $r externref) (result i32)
    (local $n i32)
    (call $unwind)
    (drop (ref.is_null (local.get $r)))
    (local.get $n)
  )

  ;; externref local, live across the suspension.
  (func (export "extern_local") (param $r externref) (result i32)
    (local $t externref)
    (local $n i32)
    (local.set $t (local.get $r))
    (local.set $n (i32.const 1))
    (call $unwind)
    (i32.add (ref.is_null (local.get $t)) (local.get $n))
  )

  ;; funcref local, live across the suspension.
  (func (export "funcref_local") (param $f funcref) (result i32)
    (local $t funcref)
    (local $n i32)
    (local.set $t (local.get $f))
    (local.set $n (i32.const 2))
    (call $unwind)
    (i32.add (ref.is_null (local.get $t)) (local.get $n))
  )
)
