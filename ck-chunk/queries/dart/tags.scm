; Dart chunk definitions using tree-sitter queries
; Node names follow tree-sitter-dart 0.2.0's grammar.

; Classes, Mixins, Enums
(class_declaration) @definition.class
(mixin_declaration) @definition.class
(enum_declaration) @definition.class

; Functions, Methods, Constructors
(function_declaration) @definition.function
(method_declaration) @definition.method
(constructor_signature) @definition.method

; Top-level variables and constants (module-level)
(top_level_variable_declaration) @module.text
