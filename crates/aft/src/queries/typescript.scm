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
  name: (type_identifier) @class.name) @class.def

;; method definitions inside classes
(class_declaration
  name: (type_identifier) @method.class_name
  body: (class_body
    (method_definition
      name: (property_identifier) @method.name) @method.def))

;; interface declarations
(interface_declaration
  name: (type_identifier) @interface.name) @interface.def

;; enum declarations
(enum_declaration
  name: (identifier) @enum.name) @enum.def

;; type alias declarations
(type_alias_declaration
  name: (type_identifier) @type_alias.name) @type_alias.def

;; top-level const/let variable declarations
(lexical_declaration
  (variable_declarator
    name: (identifier) @var.name)) @var.def

;; export statement wrappers (top-level only)
(export_statement) @export.stmt
