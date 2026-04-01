;; function declarations
(function_declaration
  name: (identifier) @fn.name) @fn.def

;; arrow functions assigned to const/let/var
(lexical_declaration
  (variable_declarator
    name: (identifier) @arrow.name
    value: (arrow_function) @arrow.body)) @arrow.def

;; class declarations
(class_declaration
  name: (identifier) @class.name) @class.def

;; method definitions inside classes
(class_declaration
  name: (identifier) @method.class_name
  body: (class_body
    (method_definition
      name: (property_identifier) @method.name) @method.def))

;; top-level const/let variable declarations
(lexical_declaration
  (variable_declarator
    name: (identifier) @var.name)) @var.def

;; export statement wrappers (top-level only)
(export_statement) @export.stmt
