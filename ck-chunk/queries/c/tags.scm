; C chunk definitions

; Functions
(function_definition) @definition.function

; Structs, enums, and unions
(struct_specifier) @definition.struct
(enum_specifier) @definition.enum
(union_specifier) @definition.class



; Preprocessor macros
(preproc_function_def) @definition.function
(preproc_def) @definition.text
