; Elixir chunk definitions
; In Elixir's tree-sitter grammar, definitions like defmodule, def, defp
; are represented as function calls with specific target identifiers

; Module definition - defmodule ModuleName do ... end
(call
  target: (identifier) @_target
  (arguments
    (alias) @name)
  (#eq? @_target "defmodule")) @module

; Public function - def function_name(...) do ... end
(call
  target: (identifier) @_target
  (#eq? @_target "def")) @definition.function

; Private function - defp function_name(...) do ... end
(call
  target: (identifier) @_target
  (#eq? @_target "defp")) @definition.function

; Public macro - defmacro macro_name(...) do ... end
(call
  target: (identifier) @_target
  (#eq? @_target "defmacro")) @definition.function

; Private macro - defmacrop macro_name(...) do ... end
(call
  target: (identifier) @_target
  (#eq? @_target "defmacrop")) @definition.function

; Protocol definition - defprotocol ProtocolName do ... end
(call
  target: (identifier) @_target
  (#eq? @_target "defprotocol")) @module

; Protocol implementation - defimpl Protocol, for: Type do ... end
(call
  target: (identifier) @_target
  (#eq? @_target "defimpl")) @module

; Struct definition - defstruct [...] (inside a module)
(call
  target: (identifier) @_target
  (#eq? @_target "defstruct")) @definition.struct

; Exception definition - defexception [...] (inside a module)
(call
  target: (identifier) @_target
  (#eq? @_target "defexception")) @definition.struct

; Guard definitions
(call
  target: (identifier) @_target
  (#eq? @_target "defguard")) @definition.function

(call
  target: (identifier) @_target
  (#eq? @_target "defguardp")) @definition.function

; Delegate function - defdelegate function_name(args), to: Module
(call
  target: (identifier) @_target
  (#eq? @_target "defdelegate")) @definition.function

; Override callback - defoverridable [...]
(call
  target: (identifier) @_target
  (#eq? @_target "defoverridable")) @definition.text

; =============================================================================
; Module Attributes - @spec, @type, @callback, @behaviour
; =============================================================================
; Module attributes in Elixir are parsed as unary_operator with @ operator,
; containing a call node where the identifier is the attribute name.

; Type specification - @spec function_name(arg_types) :: return_type
(unary_operator
  operator: "@"
  operand: (call
    target: (identifier) @_attr
    (#eq? @_attr "spec"))) @definition.spec

; Type definitions - @type, @typep (private), @opaque
(unary_operator
  operator: "@"
  operand: (call
    target: (identifier) @_attr
    (#eq? @_attr "type"))) @definition.type

(unary_operator
  operator: "@"
  operand: (call
    target: (identifier) @_attr
    (#eq? @_attr "typep"))) @definition.type

(unary_operator
  operator: "@"
  operand: (call
    target: (identifier) @_attr
    (#eq? @_attr "opaque"))) @definition.type

; Callback definition - @callback function_name(arg_types) :: return_type
(unary_operator
  operator: "@"
  operand: (call
    target: (identifier) @_attr
    (#eq? @_attr "callback"))) @definition.callback

; Optional callback - @optional_callbacks [callback_name: arity, ...]
(unary_operator
  operator: "@"
  operand: (call
    target: (identifier) @_attr
    (#eq? @_attr "optional_callbacks"))) @definition.callback

; Macro callback - @macrocallback macro_name(arg_types) :: return_type
(unary_operator
  operator: "@"
  operand: (call
    target: (identifier) @_attr
    (#eq? @_attr "macrocallback"))) @definition.callback

; Behaviour declaration - @behaviour ModuleName
(unary_operator
  operator: "@"
  operand: (call
    target: (identifier) @_attr
    (#eq? @_attr "behaviour"))) @definition.behaviour

; British spelling variant - @behavior ModuleName
(unary_operator
  operator: "@"
  operand: (call
    target: (identifier) @_attr
    (#eq? @_attr "behavior"))) @definition.behaviour
