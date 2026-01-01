; Dart chunk definitions using tree-sitter queries

; Classes, Mixins, Enums
(class_definition) @definition.class
(mixin_declaration) @definition.class
(enum_declaration) @definition.class

; Functions, Methods, Constructors
(lambda_expression) @definition.function
(class_member_definition) @definition.method
; (constructor_signature) @definition.method 

; Top-level variables and constants (module-level)
(local_variable_declaration) @module.text
